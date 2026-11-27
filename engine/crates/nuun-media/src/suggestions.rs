//! Suggestions when a disk is connected (docs/features/suite-disks-flows.md §3): only what has something new to
//! do, each with a stable key so the person can silence it for that disk and flow. A read: it never copies,
//! plans or deletes anything. «Not now» is the app's, per mount event; «don't ask again» is stored here.
use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::conversations;
use crate::suite_volume;
use crate::volumes::{self, Backend};
use crate::work_copy;
use nu9ve_json::{Error, Result, require};
use serde_json::{Value, json};
use std::path::Path;

const DISMISSED: &str = "suggestions-dismissed";
const LISTED: usize = 20;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn dismissed(catalog: &Catalog) -> Result<Value> {
    Ok(catalog.get(DISMISSED)?.unwrap_or_else(|| json!({})))
}

/// `suggestions_dismiss`: `{key, dismissed: true}` silences a suggestion for good; `false` brings it back.
pub fn dismiss(home: &Path, params: &Value) -> Result<Value> {
    let key = params["key"]
        .as_str()
        .filter(|k| !k.is_empty() && k.len() <= 512)
        .ok_or_else(|| Error::new("invalid_params", "'key' is a suggestion's key"))?;
    require(
        params["dismissed"].is_boolean(),
        "invalid_params",
        "'dismissed' is true or false",
    )?;
    let _lock = writer_lock(home)?;
    let mut all = dismissed(&Catalog::open(home)?)?;
    if params["dismissed"] == true {
        all[key] = json!({"at":nu9ve_time::now_utc()});
    } else if let Some(map) = all.as_object_mut() {
        map.remove(key);
    }
    WritableCatalog::open(home)?.set(DISMISSED, &all)?;
    Ok(json!({"dismissed":all}))
}

struct Disk {
    path: std::path::PathBuf,
    root: crate::tree::Root,
    marker: Value,
}

/// The work disks mounted here whose marker this device may use.
fn work_disks(home: &Path, backend: &dyn Backend) -> Vec<Disk> {
    backend
        .mounted_roots()
        .into_iter()
        .filter_map(|path| {
            let (root, _, marker, _) = suite_volume::usable(home, backend, &path).ok()?;
            ["work", "other"]
                .contains(&text(&marker, "role"))
                .then_some(Disk { path, root, marker })
        })
        .collect()
}

/// The receipts on `to` of copies that came from `from`, as the source paths they carried.
fn copied_from(to: &Disk, from: &str) -> Vec<String> {
    let Some(value) = work_copy::volume_apps_copies(&to.root).ok() else {
        return Vec::new();
    };
    value
        .iter()
        .filter(|r| r["source"]["volume_id"] == from)
        .flat_map(|r| r["source"]["paths"].as_array().cloned().unwrap_or_default())
        .filter_map(|p| p.as_str().map(str::to_owned))
        .collect()
}

fn within(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/"))
}

/// Two work disks connected: the roots other apps declared on one that are not on the other.
fn work_copies(disks: &[Disk]) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for from in disks {
        let from_id = text(&from.marker, "volume_id");
        let apps = work_copy::app_indexes_of(&from.root, from_id)?;
        for to in disks {
            let to_id = text(&to.marker, "volume_id");
            if to_id == from_id {
                continue;
            }
            let carried = copied_from(to, from_id);
            let mut missing = Vec::new();
            for app in &apps {
                if app["index"] != "ok" || app["app"] == "nuun" {
                    continue;
                }
                for root in app["roots"].as_array().into_iter().flatten() {
                    let path = text(root, "volume_path");
                    if path.is_empty()
                        || to.path.join(path).exists()
                        || carried.iter().any(|c| within(path, c))
                    {
                        continue;
                    }
                    missing.push(
                        json!({"app":app["app"],"path":path,"projects":app["root_projects"][path]}),
                    );
                }
            }
            if !missing.is_empty() {
                let count = missing.len();
                missing.truncate(LISTED);
                out.push(json!({"key":format!("work_copy:{from_id}:{to_id}"),"flow":"work_copy",
                    "source":{"volume_id":from_id,"label":from.marker["label"],"root":volumes::normalize(&from.path)},
                    "destination":{"volume_id":to_id,"label":to.marker["label"],"root":volumes::normalize(&to.path)},
                    "count":count,"paths":missing}));
            }
        }
    }
    Ok(out)
}

/// Two modification times that a copy keeps: equal, within what the coarsest filesystem keeps (HFS+ keeps
/// seconds, FAT two of them).
fn same_time(a: &Value, b: &Value) -> bool {
    match (a.as_i64(), b.as_i64()) {
        (Some(a), Some(b)) => (a - b).abs() < 2_000_000_000,
        _ => false,
    }
}

/// Copied before between two connected work disks and changed since on one of them: what was processed there
/// (new files under the copied folders, or files that changed). Known from the receipts on the disk, by size and
/// date only, so it works without the other app publishing an index. Only copies without `into`.
fn changed_since_copy(disks: &[Disk]) -> Vec<Value> {
    let mut out = Vec::new();
    for from in disks {
        let from_id = text(&from.marker, "volume_id");
        let receipts = work_copy::full_receipts(&from.root);
        for to in disks {
            let to_id = text(&to.marker, "volume_id");
            if to_id == from_id {
                continue;
            }
            let between: Vec<&Value> = receipts
                .iter()
                .filter(|r| {
                    r["destination"]["into"]
                        .as_str()
                        .unwrap_or_default()
                        .is_empty()
                })
                .filter(|r| {
                    let (s, d) = (&r["source"]["volume_id"], &r["destination"]["volume_id"]);
                    (s == from_id && d == to_id) || (s == to_id && d == from_id)
                })
                .collect();
            if between.is_empty() {
                continue;
            }
            // What each file of `from` was at its last copy, in either direction (receipts are oldest first).
            let mut known: std::collections::HashMap<String, (u64, Value)> = Default::default();
            let mut roots: Vec<String> = Vec::new();
            for receipt in &between {
                let here = if receipt["destination"]["volume_id"] == from_id {
                    "path"
                } else {
                    "source_path"
                };
                for file in receipt["files"].as_array().into_iter().flatten() {
                    known.insert(
                        text(file, here).to_owned(),
                        (
                            file["size"].as_u64().unwrap_or_default(),
                            file["mtime_ns"].clone(),
                        ),
                    );
                }
                for path in receipt["source"]["paths"].as_array().into_iter().flatten() {
                    let path = path.as_str().unwrap_or_default().to_owned();
                    if !roots.iter().any(|r| within(&path, r)) {
                        roots.retain(|r| !within(r, &path));
                        roots.push(path);
                    }
                }
            }
            roots.sort();
            let mut changed = Vec::new();
            let (mut files, mut bytes) = (0usize, 0u64);
            for root in roots {
                let Ok(found) = work_copy::files_under(&from.root, &root) else {
                    continue;
                };
                let (mut n, mut b, mut new) = (0usize, 0u64, 0usize);
                for (path, stamp) in found {
                    let mtime = crate::tree::stamp_json(&stamp)["mtime_ns"].clone();
                    match known.get(&path) {
                        Some((size, at)) if *size == stamp.size && same_time(at, &mtime) => {
                            continue;
                        }
                        _ => {}
                    }
                    // Already on the other disk as it is here: copied back the other way, or written to both by
                    // another flow (the conversations export). Proposing it again made the two disks ping-pong
                    // (2026-11-26). Same size and date, like the receipts; copies keep the source's dates.
                    let there = to.root.stat(&path).ok().flatten();
                    if there.as_ref().is_some_and(|there| {
                        there.size == stamp.size
                            && same_time(&crate::tree::stamp_json(there)["mtime_ns"], &mtime)
                    }) {
                        continue;
                    }
                    if !known.contains_key(&path) {
                        new += 1;
                    }
                    n += 1;
                    b += stamp.size;
                }
                if n > 0 {
                    files += n;
                    bytes += b;
                    changed.push(json!({"path":root,"files":n,"new":new,"bytes":b}));
                }
            }
            if !changed.is_empty() {
                changed.truncate(LISTED);
                out.push(json!({"key":format!("work_copy_changed:{from_id}:{to_id}"),"flow":"work_copy_changed",
                    "source":{"volume_id":from_id,"label":from.marker["label"],"root":volumes::normalize(&from.path)},
                    "destination":{"volume_id":to_id,"label":to.marker["label"],"root":volumes::normalize(&to.path)},
                    "count":files,"bytes":bytes,"paths":changed,"update":true}));
            }
        }
    }
    out
}

/// The decided conversations that are due, with their archive connected; and a working copy left behind.
fn conversations_due(
    home: &Path,
    backend: &dyn Backend,
    user_home: &Path,
    portable: &[Disk],
) -> Result<Vec<Value>> {
    let status = conversations::status_with_details(home, user_home, false)?;
    let settings = &status["settings"];
    let mut out = Vec::new();
    if settings["schedule"]["mode"] == "manual" {
        return Ok(out);
    }
    let catalog = Catalog::open(home)?;
    let Some(record) = settings["archive"]
        .as_str()
        .and_then(|name| catalog.volume(name).ok())
    else {
        return Ok(out);
    };
    let online = volumes::online(backend, &record).is_some();
    let decided: Vec<&Value> = status["collections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| !c["decided_at"].is_null() && c["present"] == true)
        .collect();
    let due: Vec<Value> = decided
        .iter()
        .filter(|c| c["due"] == true)
        .map(|c| json!({"id":c["id"],"title":c["title"],"last":c["backup"]["updated_at"]}))
        .collect();
    if online && !due.is_empty() {
        out.push(
            json!({"key":format!("conversations_backup:{}", text(&record, "id")),
            "flow":"conversations_backup","archive":{"name":record["name"],"root":record["root"]},
            "every_days":settings["schedule"]["every_days"],"collections":due}),
        );
    }
    // A working copy that missed a backup (its disk was not connected then).
    let working = &settings["working"];
    let target = Path::new(text(working, "root"));
    // Portable SSDs use source changes instead of the date of the last archive backup.
    if !working["decided_at"].is_null()
        && target.is_dir()
        && !portable.iter().any(|disk| disk.path == target)
    {
        let behind: Vec<Value> = decided
            .iter()
            .filter_map(|c| {
                let backup = c["backup"]["updated_at"].as_str()?;
                let folder = c["backup"]["folder"].as_str()?;
                let manifest = crate::tree::read_bytes(
                    target,
                    &format!("{folder}/_nuun/working.json"),
                    64 << 20,
                )
                .ok()
                .flatten()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
                let updated = manifest
                    .as_ref()
                    .map(|m| text(m, "updated_at"))
                    .unwrap_or_default();
                (updated < backup).then(|| json!({"id":c["id"],"title":c["title"],"folder":folder}))
            })
            .collect();
        if !behind.is_empty() {
            out.push(
                json!({"key":format!("conversations_working:{}", target.display()),
                "flow":"conversations_working","root":target,"collections":behind}),
            );
        }
    }
    Ok(out)
}

/// `suite_suggestions`: what is worth suggesting now, without the ones the person silenced.
pub fn suggestions(
    home: &Path,
    backend: &dyn Backend,
    user_home: &Path,
    device: &Value,
    conversations_only: bool,
) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let silenced = dismissed(&catalog)?;
    let disks = work_disks(home, backend);
    let portable = if device["device_id"].is_string() {
        disks.as_slice()
    } else {
        &[]
    };
    let mut all = Vec::new();
    if !conversations_only {
        all.extend(work_copies(&disks)?);
        all.extend(changed_since_copy(&disks));
        all.extend(conversations_due(home, backend, user_home, portable)?);
    }
    if let Some(id) = device["device_id"].as_str() {
        let targets = portable.iter().filter(|disk| {
            silenced
                .get(format!(
                    "conversations_update:{}",
                    text(&disk.marker, "volume_id")
                ))
                .is_none()
        });
        let updates = conversations::working_updates(
            &catalog,
            user_home,
            id,
            targets.map(|d| d.path.as_path()),
        )?;
        for disk in portable {
            if let Some(collections) = updates.get(disk.path.as_path()) {
                let volume = &disk.marker["volume_id"];
                all.push(json!({"key":format!("conversations_update:{}",text(&disk.marker,"volume_id")),
                    "flow":"conversations_update","destination":{"root":disk.path,"volume_id":volume,
                    "label":disk.marker["label"]},"collections":collections}));
            }
        }
    }
    let (kept, hidden): (Vec<Value>, Vec<Value>) = all
        .into_iter()
        .partition(|s| silenced.get(text(s, "key")).is_none());
    Ok(json!({"suggestions":kept,"dismissed":hidden.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Root;
    use crate::volumes::test_backend::{Fixture, setup};

    fn suggestions(home: &Path, backend: &dyn Backend, user: &Path) -> Result<Value> {
        super::suggestions(home, backend, user, &Value::Null, false)
    }

    fn marked(backend: &mut Fixture, path: &Path, id: &str, serial: &str) {
        std::fs::create_dir_all(path).unwrap();
        Root::open(path)
            .unwrap()
            .write_json(
                ".nu9ve/volume.json",
                &json!({"contract":"nu9ve-volume/1","volume_id":id,"role":"work",
                    "label":path.file_name().unwrap().to_str().unwrap(),"registered_at":"2026-11-22T00:00:00Z",
                    "registered_by":{"app":"nove","device_id":id},
                    "fs":{"os":crate::suite_device::OS,"kind":"exfat","serial":serial}}),
            )
            .unwrap();
        backend.roots.push(path.to_path_buf());
        let value = if crate::suite_device::OS == "windows" {
            json!({"device": u32::from_str_radix(&serial.replace('-', ""), 16).unwrap()})
        } else {
            json!({"volume_uuid": serial})
        };
        backend.overrides.insert(volumes::normalize(path), value);
    }

    #[test]
    fn a_root_of_another_app_missing_on_the_other_disk_is_suggested_until_copied_or_silenced() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let (x, e, home) = (base.join("VIAJE"), base.join("EDICION"), base.join("state"));
        let xid = "1b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e01";
        let mut backend = Fixture::default();
        marked(&mut backend, &x, xid, "AAAA-0001");
        marked(
            &mut backend,
            &e,
            "2b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e02",
            "AAAA-0002",
        );
        let xr = Root::open(&x).unwrap();
        xr.write_bytes("atlas-derived/potholes/a.geojson", b"{}", false)
            .unwrap();
        xr.write_json(".nu9ve/apps/nove/index.json", &json!({"contract":"nu9ve-data/1","app":"nove",
            "generated_at":"2026-11-23T00:00:00Z","projects":[],"datasets":[],"next_cursor":null,
            "roots":[{"id":"root-1","label":"atlas","state":"ok","volume_id":xid,"volume_path":"atlas-derived"}]}))
            .unwrap();
        let user = base.join("user");
        std::fs::create_dir_all(&user).unwrap();
        let found = suggestions(&home, &backend, &user).unwrap();
        let list = found["suggestions"].as_array().unwrap();
        assert_eq!(list.len(), 1, "{found}");
        assert_eq!(list[0]["flow"], "work_copy");
        assert_eq!(list[0]["paths"][0]["path"], "atlas-derived");
        let key = list[0]["key"].as_str().unwrap().to_owned();
        // Silenced for this pair of disks and flow; brought back on request.
        dismiss(&home, &json!({"key":key,"dismissed":true})).unwrap();
        let silent = suggestions(&home, &backend, &user).unwrap();
        assert_eq!(silent["suggestions"], json!([]));
        assert_eq!(silent["dismissed"], 1);
        dismiss(&home, &json!({"key":key,"dismissed":false})).unwrap();
        // Once copied (receipt on the destination), nothing new to suggest.
        let plan = work_copy::plan(
            &home,
            &backend,
            true,
            &json!({"source":x,"destination":e,"paths":["atlas-derived"]}),
        )
        .unwrap();
        work_copy::apply(
            &home,
            &backend,
            &json!({"plan_id":plan["id"],"digest":plan["digest"]}),
        )
        .unwrap();
        assert_eq!(
            suggestions(&home, &backend, &user).unwrap()["suggestions"],
            json!([])
        );
    }

    #[test]
    fn decided_conversations_are_suggested_when_due_and_their_disk_is_here() {
        let s = setup();
        let user = s.card.clone();
        Root::open(&user)
            .unwrap()
            .write_bytes(".claude/history.jsonl", b"one\n", false)
            .unwrap();
        let none = suggestions(&s.home, &s.backend, &user).unwrap();
        assert_eq!(
            none["suggestions"],
            json!([]),
            "nothing decided, nothing suggested"
        );
        conversations::settings(
            &s.home,
            &json!({"archive":"MEDIA","collections":{"claude-code":true},
            "confirm_personal":true}),
        )
        .unwrap();
        let due = suggestions(&s.home, &s.backend, &user).unwrap();
        assert_eq!(due["suggestions"][0]["flow"], "conversations_backup");
        conversations::run(
            &s.home,
            &s.backend,
            true,
            &user,
            Ok("6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f".into()),
            &json!({"when":"now"}),
        )
        .unwrap();
        assert_eq!(
            suggestions(&s.home, &s.backend, &user).unwrap()["suggestions"],
            json!([])
        );
        // The working copy chosen after that backup is behind it until the next one.
        conversations::settings(
            &s.home,
            &json!({"working":{"root":s.work},"confirm_personal":true}),
        )
        .unwrap();
        let behind = suggestions(&s.home, &s.backend, &user).unwrap();
        assert_eq!(behind["suggestions"][0]["flow"], "conversations_working");
        // Manual mode never suggests.
        conversations::settings(
            &s.home,
            &json!({"schedule":{"mode":"manual","every_days":7}}),
        )
        .unwrap();
        assert_eq!(
            suggestions(&s.home, &s.backend, &user).unwrap()["suggestions"],
            json!([])
        );
    }

    #[test]
    fn home_suggests_only_the_previous_conversation_export_on_a_work_disk_without_media() {
        let mut s = setup();
        let id = "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f";
        let disk_id = "5d0cb4e8-605d-4682-a95a-adee7139d823";
        s.backend.roots.retain(|p| p != &s.work);
        marked(&mut s.backend, &s.work, disk_id, "AABB-1234");
        assert!(suite_volume::usable(&s.home, &s.backend, &s.work).is_ok());
        s.backend.offline.insert(volumes::normalize(&s.archive));
        Root::open(&s.card)
            .unwrap()
            .write_bytes(".claude/history.jsonl", b"new\n", false)
            .unwrap();
        let device = json!({"device_id":id});
        let check = || super::suggestions(&s.home, &s.backend, &s.card, &device, true).unwrap();
        assert_eq!(check()["suggestions"], json!([]));
        Root::open(&s.work)
            .unwrap()
            .write_json(
                &format!("004_datos/conversaciones/claude-code/{id}/_nuun/working.json"),
                &json!({"contract":"nuun-conversations-working/1","collection":"claude-code",
                "device":device,"files":{}}),
            )
            .unwrap();
        let found = check();
        assert_eq!(found["suggestions"].as_array().unwrap().len(), 1);
        let item = &found["suggestions"][0];
        assert_eq!(item["flow"], "conversations_update");
        assert_eq!(item["destination"]["volume_id"], disk_id);
        assert_eq!(
            item["collections"],
            json!([{"id":"claude-code","title":"Claude Code","count":1}])
        );
        let replaced = conversations::export_working(
            &s.home,
            &s.backend,
            true,
            &s.card,
            &json!({"destination":s.work,"collections":["claude-code"],
                "volume_id":id,"confirm_personal":true}),
        )
        .unwrap_err();
        assert_eq!(replaced.code, "volume_replaced");
        dismiss(&s.home, &json!({"key":item["key"],"dismissed":true})).unwrap();
        assert_eq!(check()["suggestions"], json!([]));
    }
}
