//! Backups of the person's agent conversations, one folder per device (suite-devices §6.5, data §2.1 and §6):
//! Claude Code, Codex and Claude (app), only their history, in `<base>/<collection>/<device_id>/…` of an archive
//! disk, with a receipt per run, and optionally a verified working copy in the same shape on a work disk or an
//! analysis folder, for nove to read in place. Nothing leaves the machine without a recorded decision of the
//! person for that collection (and, apart, for the working copy). docs/features/suite-disks-flows.md §2.
use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::living;
use crate::transfer::hash_file;
use crate::tree::{Root, stamp_json};
use crate::volumes::Backend;
use nu9ve_json::{Error, Result, require, sha256_hex};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const SETTINGS: &str = "conversations-settings";
pub const BASE: &str = "004_datos/conversaciones";
pub const RECEIPT: &str = "nuun-conversations-backup/1";
pub const WORKING: &str = "nuun-conversations-working/1";
const DAY: i64 = 86_400;

/// A known collection: its id (the folder name), title, root under the home folder and history entries. Only the
/// `history` class of `storage_collections`: never credentials, settings, caches or logs.
pub struct Known {
    pub id: &'static str,
    pub title: &'static str,
    pub root: &'static [&'static str],
    pub include: &'static [&'static str],
}

pub const KNOWN: [Known; 3] = [
    Known {
        id: "claude-code",
        title: "Claude Code",
        root: &[".claude"],
        include: &[
            "projects",
            "history.jsonl",
            "todos",
            "plans",
            "transcripts",
            "file-history",
            "sessions",
        ],
    },
    Known {
        id: "codex",
        title: "Codex",
        root: &[".codex"],
        // Not its live SQLite databases (thread_history*.sqlite): Codex writes them while it runs (decision 2).
        include: &[
            "sessions",
            "archived_sessions",
            "history.jsonl",
            "session_index.jsonl*",
            "attachments",
            "generated_images",
            "visualizations",
            "dictation-history",
            "transcription-history.jsonl",
        ],
    },
    Known {
        id: "claude-desktop",
        title: "Claude (app)",
        root: &["Library", "Application Support", "Claude"],
        include: &[
            "local-agent-mode-sessions",
            "claude-code-sessions",
            "side-session-notes",
        ],
    },
];

fn known(id: &str) -> Result<&'static Known> {
    KNOWN
        .iter()
        .find(|k| k.id == id)
        .ok_or_else(|| Error::new("invalid_params", format!("Unknown collection: {id}")))
}

fn source_of(user_home: &Path, k: &Known) -> PathBuf {
    k.root
        .iter()
        .fold(user_home.to_path_buf(), |p, part| p.join(part))
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn settings_of(catalog: &Catalog) -> Result<Value> {
    let mut value = catalog.get(SETTINGS)?.unwrap_or_else(|| json!({}));
    if value["base"].as_str().is_none() {
        value["base"] = json!(BASE);
    }
    if !value["schedule"].is_object() {
        value["schedule"] = json!({"mode":"suggest","every_days":7});
    }
    if !value["collections"].is_object() {
        value["collections"] = json!({});
    }
    Ok(value)
}

/// `conversations_settings`: where and when to back up, and which collections. Turning a collection on, or the
/// working copy, needs `confirm_personal: true` and records when: that is the person's decision (data §6).
/// Turning them off needs nothing. Without parameters it only answers the settings.
pub fn settings(home: &Path, params: &Value) -> Result<Value> {
    let allowed = [
        "archive",
        "base",
        "schedule",
        "collections",
        "working",
        "confirm_personal",
    ];
    let given = params
        .as_object()
        .filter(|p| p.keys().all(|k| allowed.contains(&k.as_str())))
        .ok_or_else(|| Error::new("invalid_params", "Unknown parameters"))?;
    if given.is_empty() {
        return settings_of(&Catalog::open(home)?);
    }
    let confirmed = params["confirm_personal"] == true;
    let _lock = writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let mut value = settings_of(&catalog)?;
    let now = nu9ve_time::now_utc();
    if let Some(archive) = given.get("archive") {
        if archive.is_null() {
            value["archive"] = Value::Null;
        } else {
            let name = archive
                .as_str()
                .ok_or_else(|| Error::new("invalid_params", "'archive' is a disk name"))?;
            let record = catalog.volume(name)?;
            require(
                record["role"] == "archive",
                "invalid_role",
                "Back up conversations on an archive disk",
            )?;
            if value["archive"] != record["name"] {
                // Another disk is another decision: every collection asks again.
                require(
                    confirmed || value["collections"].as_object().is_none_or(Map::is_empty),
                    "confirmation_required",
                    "Confirm that your conversations may be kept on this disk",
                )?;
                for (_, entry) in value["collections"].as_object_mut().unwrap().iter_mut() {
                    entry["decided_at"] = json!(now);
                }
            }
            value["archive"] = record["name"].clone();
        }
    }
    if let Some(base) = given.get("base") {
        let base = base
            .as_str()
            .ok_or_else(|| Error::new("invalid_params", "'base' is a folder"))?;
        crate::rules::relative(base)?;
        value["base"] = json!(base);
    }
    if let Some(schedule) = given.get("schedule") {
        let mode = schedule["mode"].as_str().unwrap_or_default();
        let days = schedule["every_days"].as_u64().unwrap_or(7);
        require(
            ["manual", "suggest", "auto"].contains(&mode) && (1..=365).contains(&days),
            "invalid_params",
            "Use mode manual, suggest or auto and every_days 1–365",
        )?;
        value["schedule"] = json!({"mode":mode,"every_days":days});
    }
    if let Some(chosen) = given.get("collections") {
        let chosen = chosen.as_object().ok_or_else(|| {
            Error::new("invalid_params", "'collections' maps ids to true or false")
        })?;
        for (id, on) in chosen {
            known(id)?;
            match on.as_bool() {
                Some(true) => {
                    if value["collections"][id]["decided_at"].is_null() {
                        require(
                            confirmed,
                            "confirmation_required",
                            "Confirm that these conversations may leave this Mac for the chosen disk",
                        )?;
                        value["collections"][id] = json!({"decided_at":now});
                    }
                }
                Some(false) => {
                    value["collections"].as_object_mut().unwrap().remove(id);
                }
                None => {
                    return Err(Error::new(
                        "invalid_params",
                        "'collections' maps ids to true or false",
                    ));
                }
            }
        }
    }
    if let Some(working) = given.get("working") {
        if working.is_null() {
            value["working"] = Value::Null;
        } else {
            let root = PathBuf::from(text(working, "root"));
            require(
                root.is_absolute() && root.is_dir(),
                "invalid_params",
                "The working copy goes to an existing absolute folder",
            )?;
            let changed = value["working"]["root"] != json!(root);
            require(
                confirmed || !changed,
                "confirmation_required",
                "Confirm that a working copy of your conversations may be kept there",
            )?;
            let decided = if changed {
                json!(now)
            } else {
                value["working"]["decided_at"].clone()
            };
            value["working"] = json!({"root":root,"decided_at":decided});
        }
    }
    value["updated_at"] = json!(now);
    WritableCatalog::open(home)?.set(SETTINGS, &value)?;
    Ok(value)
}

/// Seconds since 1970 of an RFC 3339 UTC time (`YYYY-MM-DDTHH:MM:SS…Z`), by Howard Hinnant's `days_from_civil`.
fn seconds(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || !iso.ends_with('Z') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| iso.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * DAY + h * 3600 + mi * 60 + s)
}

/// The last completed backup of a collection to this archive, from the catalog's living collections.
fn last_backup(catalog: &Catalog, source: &Path, archive_id: &str) -> Result<Option<Value>> {
    let source = source.display().to_string();
    Ok(crate::reads::living_collections(catalog)?
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["source"] == source && c["archive_id"] == archive_id)
        .max_by(|a, b| text(a, "updated_at").cmp(text(b, "updated_at")))
        .cloned())
}

/// The history entries of a collection present here, their files and bytes (only `lstat`: nothing is opened).
fn measure(source: &Path, k: &Known) -> Value {
    let mut entries = Vec::new();
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut stack: Vec<PathBuf> = Vec::new();
    if let Ok(read) = std::fs::read_dir(source) {
        for entry in read.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if k.include
                .iter()
                .any(|p| crate::agent_storage::glob(p, &name))
            {
                entries.push(name);
                stack.push(entry.path());
            }
        }
    }
    while let Some(path) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            if let Ok(read) = std::fs::read_dir(&path) {
                stack.extend(read.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if meta.is_file() {
            files += 1;
            bytes += meta.len();
        }
    }
    entries.sort();
    json!({"entries":entries,"files":files,"bytes":bytes})
}

/// `conversations_status`: what is decided, where, and what is due. A read.
pub fn status(home: &Path, user_home: &Path) -> Result<Value> {
    status_with_details(home, user_home, true)
}

pub(crate) fn status_with_details(home: &Path, user_home: &Path, details: bool) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let settings = settings_of(&catalog)?;
    let archive = settings["archive"]
        .as_str()
        .and_then(|name| catalog.volume(name).ok());
    let every = settings["schedule"]["every_days"].as_i64().unwrap_or(7) * DAY;
    let now = seconds(&nu9ve_time::now_utc()).unwrap_or_default();
    let mut collections = Vec::new();
    for k in &KNOWN {
        let source = source_of(user_home, k);
        let decided = &settings["collections"][k.id]["decided_at"];
        let mut entry = json!({"id":k.id,"title":k.title,"source":source,"present":source.is_dir(),
            "include":k.include,"decided_at":decided,"backup":Value::Null,"due":false});
        if details && source.is_dir() {
            entry["history"] = measure(&source, k);
        }
        if let Some(archive) = &archive {
            let last = last_backup(&catalog, &source, text(archive, "id"))?;
            let due = !decided.is_null()
                && source.is_dir()
                && last
                    .as_ref()
                    .and_then(|l| seconds(text(l, "updated_at")))
                    .is_none_or(|at| now - at >= every);
            entry["due"] = json!(due);
            entry["backup"] = last.map_or(Value::Null, |l| {
                json!({"updated_at":l["updated_at"],"folder":l["folder"],"files":l["files"],"bytes":l["bytes"]})
            });
        }
        collections.push(entry);
    }
    let working = settings["working"].clone();
    Ok(
        json!({"settings":settings,"archive":archive.map(|a| json!({"name":a["name"],"id":a["id"],"root":a["root"]})),
        "collections":collections,"working":working,
        "personal":"Only the collections with decided_at leave this Mac, to the chosen archive; the working copy only when working.decided_at is set"}),
    )
}

/// The living-collection parameters of a decided collection on this archive.
fn living_params(settings: &Value, k: &Known, user_home: &Path) -> Value {
    json!({"source":source_of(user_home, k),"archive":settings["archive"],
        "folder":format!("{}/{}", text(settings, "base"), k.id),"include":k.include,"name":k.title})
}

/// `conversations_backup_plan`: one reviewed living-collection plan per decided collection present here (or the
/// ones in `collections`), each in its device folder. The device is created if this machine has none.
pub fn backup_plan(
    home: &Path,
    backend: &dyn Backend,
    user_home: &Path,
    device: std::result::Result<String, Error>,
    params: &Value,
) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let settings = settings_of(&catalog)?;
    require(
        settings["archive"].is_string(),
        "not_configured",
        "Choose the archive disk for your conversations first",
    )?;
    let wanted: Option<Vec<&str>> = params["collections"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect());
    let mut plans = Vec::new();
    let mut skipped = Vec::new();
    for k in &KNOWN {
        if wanted.as_ref().is_some_and(|w| !w.contains(&k.id)) {
            continue;
        }
        if settings["collections"][k.id]["decided_at"].is_null() {
            if wanted.is_some() {
                return Err(Error::new(
                    "confirmation_required",
                    format!("{} is not decided for backup", k.title),
                ));
            }
            continue;
        }
        if !source_of(user_home, k).is_dir() {
            skipped.push(json!({"id":k.id,"reason":"absent"}));
            continue;
        }
        let params = living::device_folder(
            home,
            &living_params(&settings, k, user_home),
            device.clone(),
        )?;
        let mut plan = living::plan(home, backend, &params)?;
        plan["collection"] = json!(k.id);
        plans.push(plan);
    }
    Ok(json!({"plans":plans,"skipped":skipped}))
}

fn hash_at(root: &Root, rel: &str) -> Result<Option<String>> {
    let Some(file) = root.open_read(rel)? else {
        return Ok(None);
    };
    Ok(Some(hash_file(
        &mut nu9ve_volumes::fs::verify_read(file),
        |_| {},
    )?))
}

pub fn device_of(isolated: bool) -> Value {
    crate::suite_device::current(isolated, &json!({"peek":true}))
        .ok()
        .filter(|d| d["state"] == "ok")
        .map_or(
            Value::Null,
            |d| json!({"device_id":d["device"]["device_id"],"name":d["device"]["name"]}),
        )
}

/// Explicitly export selected histories to a portable work disk, independently of archive settings.
pub fn export_working(
    home: &Path,
    backend: &dyn Backend,
    isolated: bool,
    user_home: &Path,
    params: &Value,
) -> Result<Value> {
    require(
        params["confirm_personal"] == true,
        "confirmation_required",
        "Confirm which history may be copied to this disk",
    )?;
    let target = Path::new(text(params, "destination"));
    let selected = params["collections"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= KNOWN.len())
        .ok_or_else(|| Error::new("invalid_params", "Choose the histories to export"))?;
    let mut collections = BTreeMap::new();
    for id in selected {
        let collection = known(id.as_str().unwrap_or_default())?;
        let source = source_of(user_home, collection);
        require(
            source.is_dir(),
            "source_missing",
            format!("{} is not on this Mac", collection.title),
        )?;
        collections.insert(collection.id, (collection, source));
    }
    let _lock = writer_lock(home)?;
    let (root, _, marker, _) = crate::suite_volume::usable(home, backend, target)?;
    require(
        matches!(text(&marker, "role"), "work" | "other"),
        "invalid_role",
        "Choose a work disk to export conversations",
    )?;
    require(
        params
            .get("volume_id")
            .is_none_or(|id| id == &marker["volume_id"]),
        "volume_replaced",
        "The work disk changed after the suggestion was shown",
    )?;
    let device = device_of(isolated);
    require(
        !device.is_null(),
        "device_unavailable",
        "Register this Mac in the suite first",
    )?;
    let catalog = Catalog::open(home)?;
    let settings = settings_of(&catalog)?;
    let mut results = Vec::new();
    for (collection, source) in collections.values() {
        root.check()?;
        let folder = format!(
            "{}/{}/{}",
            text(&settings, "base"),
            collection.id,
            text(&device, "device_id")
        );
        let mut result = working_copy(
            backend, source, collection, &root.path, &folder, &device, false,
        )?;
        result["collection"] = json!(collection.id);
        results.push(result);
    }
    Ok(json!({"results":results}))
}

/// U1 de nove (`nuun-conversations-compat.md`): la misma copia de trabajo que `conversations_export_working`,
/// pedida desde la suite, sólo con lo que la persona ya decidió en Ajustes → Conversaciones: el disco de la copia
/// de trabajo y las colecciones con `decided_at`. Otro destino u otra colección es `confirmation_required`.
pub fn export_decided(
    home: &Path,
    backend: &dyn Backend,
    isolated: bool,
    user_home: &Path,
    params: &Value,
) -> Result<Value> {
    let decided = export_decided_check(home, params)?;
    export_working(home, backend, isolated, user_home, &decided)
}

/// The parameters of `export_working` that the person's decision allows, or `confirmation_required`.
pub fn export_decided_check(home: &Path, params: &Value) -> Result<Value> {
    let given = params
        .as_object()
        .filter(|p| {
            p.keys()
                .all(|k| ["destination", "collections", "confirm_personal"].contains(&k.as_str()))
        })
        .ok_or_else(|| {
            Error::new(
                "invalid_params",
                "Only destination and collections are accepted",
            )
        })?;
    let settings = settings_of(&Catalog::open(home)?)?;
    let root = settings["working"]["root"].as_str().unwrap_or_default();
    require(
        !root.is_empty() && !settings["working"]["decided_at"].is_null(),
        "confirmation_required",
        "Decide the working copy in nüün first (Settings → Conversations)",
    )?;
    if let Some(destination) = given.get("destination") {
        require(
            destination.as_str() == Some(root),
            "confirmation_required",
            "Only the destination decided in nüün",
        )?;
    }
    let decided: Vec<String> = settings["collections"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, c)| !c["decided_at"].is_null())
        .map(|(id, _)| id.clone())
        .collect();
    let collections: Vec<String> = match given.get("collections") {
        None => decided.clone(),
        Some(list) => list
            .as_array()
            .ok_or_else(|| Error::new("invalid_params", "collections is a list of ids"))?
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_owned())
            .collect(),
    };
    require(
        !collections.is_empty() && collections.iter().all(|c| decided.contains(c)),
        "confirmation_required",
        "Only collections decided in nüün",
    )?;
    Ok(json!({"destination": root, "collections": collections, "confirm_personal": true}))
}

/// `conversations_backup_apply`: each reviewed plan copied and verified (`living_apply`), its receipt published
/// next to the collection's manifest, then the working copy when it is decided.
pub fn backup_apply(
    home: &Path,
    backend: &dyn Backend,
    isolated: bool,
    user_home: &Path,
    params: &Value,
) -> Result<Value> {
    let reviewed = params["plans"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| Error::new("invalid_params", "'plans' lists {plan_id, digest}"))?;
    let device = device_of(isolated);
    let mut results = Vec::new();
    for item in reviewed {
        let done = living::apply(home, backend, item)?;
        let catalog = Catalog::open(home)?;
        let collection = KNOWN
            .iter()
            .find(|k| done["source"] == json!(source_of(user_home, k)))
            .ok_or_else(|| Error::new("invalid_plan", "Not a conversations collection"))?;
        let record = catalog.volume(text(&done, "archive_id"))?;
        crate::volumes::check(backend, &record)?;
        let archive = Root::open(Path::new(text(&record, "root")))?;
        let folder = text(&done, "folder").to_owned();
        let manifest = hash_at(&archive, &format!("{folder}/_nuun/manifest.json"))?;
        let receipt = json!({"contract":RECEIPT,"collection":collection.id,"title":collection.title,
            "device":device,"plan_id":done["id"],"plan_digest":done["digest"],"completed_at":done["completed_at"],
            "archive":{"name":record["name"],"folder":folder},"include":collection.include,
            "result":done["result"],"bytes":done["bytes"],"unchanged":done["unchanged"],
            "manifest":{"path":format!("{folder}/_nuun/manifest.json"),"sha256":manifest}});
        let rel = format!("{folder}/_nuun/receipts/{}.json", text(&done, "id"));
        let bytes = nu9ve_json::canonical(&receipt);
        if archive.stat(&rel)?.is_none() {
            archive.write_bytes(&rel, &bytes, false)?;
        }
        let mut entry = json!({"collection":collection.id,"plan_id":done["id"],"result":done["result"],
            "receipt":{"path":rel,"sha256":sha256_hex(&bytes)}});
        let settings = settings_of(&catalog)?;
        if params["working"] != false && !settings["working"]["decided_at"].is_null() {
            let target = PathBuf::from(text(&settings["working"], "root"));
            let device_folder = folder
                .strip_prefix(&format!("{}/", text(&settings, "base")))
                .unwrap_or(&folder)
                .to_owned();
            entry["working"] = match writer_lock(home).and_then(|_lock| {
                working_copy(
                    backend,
                    &source_of(user_home, collection),
                    collection,
                    &target,
                    &format!("{}/{device_folder}", text(&settings, "base")),
                    &device,
                    true,
                )
            }) {
                Ok(done) => done,
                // The backup stands; the working copy says why it did not happen (its disk is not here…).
                Err(error) => json!({"state":"failed","code":error.code,"message":error.message}),
            };
        }
        results.push(entry);
    }
    Ok(json!({"results":results}))
}

/// `conversations_run`: plan and apply in one call. `when: "due"` (the app, on its schedule) does it only for the
/// collections that are due and only when the person chose `auto`; `when: "now"` is the person's click.
pub fn run(
    home: &Path,
    backend: &dyn Backend,
    isolated: bool,
    user_home: &Path,
    device: std::result::Result<String, Error>,
    params: &Value,
) -> Result<Value> {
    let state = status(home, user_home)?;
    let collections: Vec<Value> = match params["when"].as_str() {
        Some("now") => state["collections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| !c["decided_at"].is_null() && c["present"] == true)
            .map(|c| c["id"].clone())
            .collect(),
        Some("due") => {
            if state["settings"]["schedule"]["mode"] != "auto" {
                return Ok(json!({"results":[],"reason":"not_automatic"}));
            }
            state["collections"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["due"] == true)
                .map(|c| c["id"].clone())
                .collect()
        }
        _ => {
            return Err(Error::new("invalid_params", "'when' is \"now\" or \"due\""));
        }
    };
    if collections.is_empty() {
        return Ok(json!({"results":[],"reason":"nothing_due"}));
    }
    let plans = backup_plan(
        home,
        backend,
        user_home,
        device,
        &json!({"collections":collections}),
    )?;
    let reviewed: Vec<Value> = plans["plans"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| json!({"plan_id":p["id"],"digest":p["digest"]}))
        .collect();
    if reviewed.is_empty() {
        return Ok(json!({"results":[],"skipped":plans["skipped"]}));
    }
    backup_apply(
        home,
        backend,
        isolated,
        user_home,
        &json!({"plans":reviewed}),
    )
}

/// The working copy's folder is never on an archive disk (that one keeps the backup), nor inside the source.
fn working_target(backend: &dyn Backend, target: &Path, source: &Path) -> Result<()> {
    require(
        !target.starts_with(source) && !source.starts_with(target),
        "overlapping_roots",
        "The working copy cannot be inside the conversations it copies",
    )?;
    if let Ok(info) = backend.describe(target, false)
        && let Some(mount) = info["mount"].as_str()
        && let Ok(Some(bytes)) =
            crate::tree::read_bytes(Path::new(mount), ".nu9ve/volume.json", 16 * 1024)
        && let Ok(marker) = serde_json::from_slice::<Value>(&bytes)
    {
        require(
            marker["role"] != "archive",
            "invalid_role",
            "The working copy goes to a work disk or folder, not to an archive",
        )?;
    }
    Ok(())
}

/// Copy one file to `rel` on `target`: a partial, flushed with the source's times, read back and hashed, then
/// renamed over the previous working copy only once it is verified. Returns its SHA-256, or `None` when the
/// source changed while it was read (the next run takes it).
fn mirror_file(source: &Root, rel_src: &str, target: &Root, rel: &str) -> Result<Option<Value>> {
    let Some(before) = source.stat(rel_src)? else {
        return Ok(None);
    };
    let Some(input) = source.open_read(rel_src)? else {
        return Ok(None);
    };
    let from = source.path.join(rel_src);
    let (dir, name) = target.parent(rel, true)?;
    let partial = dir.join(format!(".{name}.ncpartial"));
    if partial.exists() {
        std::fs::remove_file(&partial).map_err(|e| nu9ve_volumes::fs::io_error(e, &partial))?;
    }
    let mut input = input.take(before.size);
    let mut output = nu9ve_volumes::fs::create_new(&partial)?;
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    let mut block = vec![0u8; 1 << 20];
    loop {
        let n = input
            .read(&mut block)
            .map_err(|e| nu9ve_volumes::fs::io_error(e, &from))?;
        if n == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &block[..n]);
        output
            .write_all(&block[..n])
            .map_err(|e| nu9ve_volumes::fs::io_error(e, &partial))?;
    }
    let meta = input
        .get_ref()
        .metadata()
        .map_err(|e| nu9ve_volumes::fs::io_error(e, &from))?;
    let mut times = std::fs::FileTimes::new();
    if let Ok(modified) = meta.modified() {
        times = times.set_modified(modified);
    }
    output
        .set_times(times)
        .map_err(|e| nu9ve_volumes::fs::io_error(e, &partial))?;
    nu9ve_volumes::fs::full_sync(&output)?;
    drop(output);
    let sha = nu9ve_json::hex(&sha2::Digest::finalize(hasher));
    let unchanged = source
        .stat(rel_src)?
        .is_some_and(|after| stamp_json(&after) == stamp_json(&before));
    if !unchanged || input.limit() != 0 {
        std::fs::remove_file(&partial).map_err(|e| nu9ve_volumes::fs::io_error(e, &partial))?;
        return Ok(None);
    }
    let mut check = nu9ve_volumes::fs::verify_read(
        std::fs::File::open(&partial).map_err(|e| nu9ve_volumes::fs::io_error(e, &partial))?,
    );
    require(
        hash_file(&mut check, |_| {})? == sha,
        "checksum_mismatch",
        format!("Working copy verification failed: {rel}"),
    )?;
    nu9ve_volumes::fs::replace(&partial, &dir.join(&name))?;
    target.sync_parent(rel)?;
    Ok(Some(json!({"size":before.size,"mtime_ns":before.mtime_ns,
        "ctime_ns":before.ctime_ns,"inode":before.inode,"sha256":sha})))
}

fn history_files(
    origin: &Root,
    k: &Known,
    mut visit: impl FnMut(String, Option<nu9ve_volumes::Stamp>) -> Result<()>,
) -> Result<()> {
    let mut entries: Vec<String> = std::fs::read_dir(&origin.path)
        .map_err(|e| nu9ve_volumes::fs::io_error(e, &origin.path))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|name| {
            k.include
                .iter()
                .any(|p| crate::agent_storage::glob(p, name))
        })
        .collect();
    entries.sort();
    for name in entries {
        let stamp = nu9ve_volumes::stamp(&origin.path.join(&name))?;
        match stamp.kind {
            nu9ve_volumes::EntryKind::Dir => {
                crate::coverage::walk_stamped(origin, &name, &mut visit)?
            }
            nu9ve_volumes::EntryKind::File => visit(name, Some(stamp))?,
            _ => visit(name, None)?,
        }
    }
    Ok(())
}

fn same_source(entry: &Value, stamp: &nu9ve_volumes::Stamp) -> bool {
    entry["size"].as_u64() == Some(stamp.size)
        && entry["mtime_ns"] == json!(stamp.mtime_ns)
        && entry["sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64)
        && entry
            .get("ctime_ns")
            .is_none_or(|n| *n == json!(stamp.ctime_ns))
        && entry.get("inode").is_none_or(|n| *n == json!(stamp.inode))
}

fn working_manifest(target: &Path, rel: &str) -> Result<Option<Value>> {
    Ok(crate::tree::read_bytes(target, rel, 64 << 20)?
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()))
}

/// Check source metadata once per collection, comparing with only the manifests on the eligible SSDs.
pub(crate) fn working_updates<'a>(
    catalog: &Catalog,
    user_home: &Path,
    device: &str,
    targets: impl IntoIterator<Item = &'a Path>,
) -> Result<BTreeMap<&'a Path, Vec<Value>>> {
    let settings = settings_of(catalog)?;
    let base = text(&settings, "base");
    let targets: Vec<&Path> = targets.into_iter().collect();
    let mut result = BTreeMap::<_, Vec<Value>>::new();
    for k in &KNOWN {
        let source = source_of(user_home, k);
        if !source.is_dir() {
            continue;
        }
        let folder = format!("{base}/{}/{device}", k.id);
        let rel = format!("{folder}/_nuun/working.json");
        let mut candidates: Vec<_> = targets
            .iter()
            .filter_map(|&target| {
                let saved = working_manifest(target, &rel).ok()?;
                let decided = settings["working"]["root"].as_str() == target.to_str()
                    && !settings["working"]["decided_at"].is_null()
                    && !settings["collections"][k.id]["decided_at"].is_null();
                let manifest = saved.filter(|m| {
                    m["contract"] == WORKING
                        && m["collection"] == k.id
                        && m["files"].is_object()
                        && (m["device"].is_null() || m["device"]["device_id"] == device)
                });
                (decided || manifest.is_some()).then_some((target, manifest, 0usize))
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let scanned = Root::open(&source).and_then(|origin| {
            history_files(&origin, k, |path, stamp| {
                if let Some(stamp) = stamp {
                    for (_, manifest, count) in &mut candidates {
                        if !manifest
                            .as_ref()
                            .is_some_and(|m| same_source(&m["files"][&path], &stamp))
                        {
                            *count += 1;
                        }
                    }
                }
                Ok(())
            })
        });
        if scanned.is_err() {
            // An agent can rotate its history during the walk; retry at the next refresh.
            continue;
        }
        for (target, _, count) in candidates {
            if count > 0 {
                result
                    .entry(target)
                    .or_default()
                    .push(json!({"id":k.id,"title":k.title,"count":count}));
            }
        }
    }
    Ok(result)
}

/// The verified mirror of a collection's history in `<target>/<folder>/` and its manifest
/// (`_nuun/working.json`, `nuun-conversations-working/1`). Files no longer in the source stay.
fn working_copy(
    backend: &dyn Backend,
    source: &Path,
    k: &Known,
    target: &Path,
    folder: &str,
    device: &Value,
    from_archive_backup: bool,
) -> Result<Value> {
    require(
        target.is_dir(),
        "volume_offline",
        "The working copy's folder is not here",
    )?;
    working_target(backend, target, source)?;
    let root = Root::open(target)?;
    let origin = Root::open(source)?;
    let manifest_rel = format!("{folder}/_nuun/working.json");
    let manifest = working_manifest(target, &manifest_rel)?;
    let previous = manifest.as_ref().and_then(|m| m["files"].as_object());
    let mut files = Map::new();
    let (mut copied, mut same, mut changed_during, mut links) = (0usize, 0usize, 0usize, 0usize);
    let mut bytes = 0u64;
    history_files(&origin, k, |path, stamp| {
        let Some(stamp) = stamp else {
            links += 1;
            return Ok(());
        };
        let rel = format!("{folder}/{path}");
        let kept = previous.and_then(|p| p.get(&path)).filter(|p| {
            same_source(p, &stamp)
                && root
                    .stat(&rel)
                    .ok()
                    .flatten()
                    .is_some_and(|s| json!(s.size) == p["size"])
        });
        if let Some(kept) = kept {
            same += 1;
            files.insert(path, kept.clone());
            return Ok(());
        }
        match mirror_file(&origin, &path, &root, &rel)? {
            Some(verified) => {
                copied += 1;
                bytes += verified["size"].as_u64().unwrap_or_default();
                files.insert(path, verified);
            }
            None => changed_during += 1,
        }
        Ok(())
    })?;
    let gone = previous
        .into_iter()
        .flat_map(|p| p.keys())
        .filter(|p| !files.contains_key(*p))
        .count();
    if from_archive_backup
        || manifest.as_ref().is_none_or(|m| {
            m["contract"] != WORKING
                || m["device"] != *device
                || m["source"] != json!(source)
                || m["collection"] != k.id
                || previous != Some(&files)
        })
    {
        let manifest = json!({"contract":WORKING,"collection":k.id,"title":k.title,"device":device,
            "source":source,"updated_at":nu9ve_time::now_utc(),"include":k.include,"files":files});
        root.write_json(&manifest_rel, &manifest)?;
    }
    Ok(
        json!({"state":"completed","root":target,"folder":folder,"copied":copied,"unchanged":same,
        "changed_during_copy":changed_during,"skipped_links":links,"no_longer_in_source":gone,"bytes":bytes,
        "manifest":manifest_rel}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_suite_may_only_ask_for_the_decided_export() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let home = root.join("media");
        crate::catalog::fixture::create(&home);
        let disk = root.join("EDICION");
        std::fs::create_dir_all(&disk).unwrap();
        // Nothing decided: nothing leaves.
        assert_eq!(
            export_decided_check(&home, &json!({})).unwrap_err().code,
            "confirmation_required"
        );
        settings(
            &home,
            &json!({"working": {"root": disk}, "collections": {"claude-code": true}, "confirm_personal": true}),
        )
        .unwrap();
        let decided = export_decided_check(&home, &json!({})).unwrap();
        assert_eq!(decided["destination"], json!(disk));
        assert_eq!(decided["collections"], json!(["claude-code"]));
        for refused in [
            json!({"destination": "/Volumes/OTRO"}),
            json!({"collections": ["codex"]}),
            json!({"collections": []}),
        ] {
            assert_eq!(
                export_decided_check(&home, &refused).unwrap_err().code,
                "confirmation_required",
                "{refused}"
            );
        }
        assert_eq!(
            export_decided_check(&home, &json!({"other": 1}))
                .unwrap_err()
                .code,
            "invalid_params"
        );
    }

    const M1: &str = "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f";

    fn agent_home(user: &Path) {
        let h = Root::open(user).unwrap();
        h.write_bytes(".claude/projects/repo/s1.jsonl", b"{\"turn\":1}\n", false)
            .unwrap();
        h.write_bytes(".claude/history.jsonl", b"one\n", false)
            .unwrap();
        h.write_bytes(".claude/.credentials.json", b"secret", false)
            .unwrap();
        h.write_bytes(".claude/settings.json", b"{}", false)
            .unwrap();
        h.write_bytes(".claude/cache/blob", b"cache", false)
            .unwrap();
        h.write_bytes(".codex/sessions/2026/a.jsonl", b"codex\n", false)
            .unwrap();
        h.write_bytes(".codex/thread_history_1.sqlite", b"live db", false)
            .unwrap();
        h.write_bytes(".codex/auth.json", b"secret", false).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hosts", user.join(".claude/projects/repo/link")).unwrap();
    }

    #[test]
    fn only_decided_history_leaves_in_one_folder_per_device_with_a_receipt_and_a_working_copy() {
        let s = crate::volumes::test_backend::setup();
        let user = s.card.clone();
        agent_home(&user);
        let decide = json!({"archive":"MEDIA","collections":{"claude-code":true,"codex":true}});
        assert_eq!(
            settings(&s.home, &decide).unwrap_err().code,
            "confirmation_required"
        );
        let mut confirmed = decide.clone();
        confirmed["confirm_personal"] = json!(true);
        let chosen = settings(&s.home, &confirmed).unwrap();
        assert!(chosen["collections"]["claude-code"]["decided_at"].is_string());
        assert!(chosen["collections"]["claude-desktop"].is_null());
        let before = status(&s.home, &user).unwrap();
        assert_eq!(before["collections"][0]["due"], true);
        assert_eq!(before["collections"][2]["decided_at"], Value::Null);

        let plans = backup_plan(&s.home, &s.backend, &user, Ok(M1.into()), &json!({})).unwrap();
        assert_eq!(plans["plans"].as_array().unwrap().len(), 2);
        let folder = format!("{BASE}/claude-code/{M1}");
        assert_eq!(plans["plans"][0]["folder"], folder);
        assert_eq!(plans["plans"][0]["skipped_links"], 1);
        let reviewed: Vec<Value> = plans["plans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| json!({"plan_id":p["id"],"digest":p["digest"]}))
            .collect();
        let done =
            backup_apply(&s.home, &s.backend, true, &user, &json!({"plans":reviewed})).unwrap();
        let a = &s.archive;
        assert!(a.join(format!("{folder}/projects/repo/s1.jsonl")).is_file());
        assert!(a.join(format!("{folder}/history.jsonl")).is_file());
        for never in [
            ".credentials.json",
            "settings.json",
            "cache/blob",
            "projects/repo/link",
        ] {
            assert!(!a.join(format!("{folder}/{never}")).exists(), "{never}");
        }
        let codex = format!("{BASE}/codex/{M1}");
        assert!(a.join(format!("{codex}/sessions/2026/a.jsonl")).is_file());
        assert!(!a.join(format!("{codex}/thread_history_1.sqlite")).exists());
        assert!(!a.join(format!("{codex}/auth.json")).exists());
        let receipt_rel = text(&done["results"][0]["receipt"], "path");
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(a.join(receipt_rel)).unwrap()).unwrap();
        assert_eq!(receipt["contract"], RECEIPT);
        assert_eq!(receipt["collection"], "claude-code");
        assert_eq!(receipt["result"]["copied"], 2);
        assert_eq!(receipt["manifest"]["sha256"].as_str().unwrap().len(), 64);
        assert!(done["results"][0].get("working").is_none());
        let after = status(&s.home, &user).unwrap();
        assert_eq!(after["collections"][0]["due"], false);
        assert_eq!(after["collections"][0]["backup"]["folder"], folder);

        // The working copy is a decision of its own, then follows every backup in the same shape.
        let working = json!({"working":{"root":s.work}});
        assert_eq!(
            settings(&s.home, &working).unwrap_err().code,
            "confirmation_required"
        );
        let mut working = working;
        working["confirm_personal"] = json!(true);
        settings(&s.home, &working).unwrap();
        Root::open(&user)
            .unwrap()
            .write_bytes(".claude/history.jsonl", b"one\ntwo\n", true)
            .unwrap();
        let ran = run(
            &s.home,
            &s.backend,
            true,
            &user,
            Ok(M1.into()),
            &json!({"when":"now"}),
        )
        .unwrap();
        let first = &ran["results"][0];
        assert_eq!(first["result"]["versioned"], 1);
        assert_eq!(first["working"]["state"], "completed", "{first}");
        assert_eq!(
            std::fs::read(s.work.join(format!("{folder}/history.jsonl"))).unwrap(),
            b"one\ntwo\n"
        );
        assert!(!s.work.join(format!("{folder}/.credentials.json")).exists());
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(s.work.join(format!("{folder}/_nuun/working.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["contract"], WORKING);
        assert_eq!(
            manifest["files"]["history.jsonl"]["sha256"],
            json!(sha256_hex(b"one\ntwo\n"))
        );
        // Again with nothing new: the working copy keeps what it verified.
        let again = run(
            &s.home,
            &s.backend,
            true,
            &user,
            Ok(M1.into()),
            &json!({"when":"now"}),
        )
        .unwrap();
        assert_eq!(again["results"][0]["working"]["copied"], 0);
        // On its own schedule, only when the person chose automatic.
        let due = run(
            &s.home,
            &s.backend,
            true,
            &user,
            Ok(M1.into()),
            &json!({"when":"due"}),
        )
        .unwrap();
        assert_eq!(due["reason"], "not_automatic");
    }

    #[test]
    fn a_working_copy_never_goes_to_an_archive() {
        let s = crate::volumes::test_backend::setup();
        let user = s.card.clone();
        agent_home(&user);
        Root::open(&s.archive)
            .unwrap()
            .write_json(".nu9ve/volume.json", &json!({"role":"archive"}))
            .unwrap();
        settings(
            &s.home,
            &json!({"archive":"MEDIA","collections":{"claude-code":true},
            "working":{"root":s.archive},"confirm_personal":true}),
        )
        .unwrap();
        let ran = run(
            &s.home,
            &s.backend,
            true,
            &user,
            Ok(M1.into()),
            &json!({"when":"now"}),
        )
        .unwrap();
        assert_eq!(ran["results"][0]["working"]["code"], "invalid_role");
        assert_eq!(seconds("1970-01-02T00:00:01Z"), Some(DAY + 1));
        assert_eq!(seconds("2026-11-23T00:00:00.123Z"), Some(1_795_392_000));
    }

    #[test]
    fn working_updates_are_per_device_and_repeat_without_rewriting_the_manifest() {
        let mut s = crate::volumes::test_backend::setup();
        s.backend
            .offline
            .insert(crate::volumes::normalize(&s.archive));
        let user = &s.card;
        let source = user.join(".claude");
        agent_home(user);
        let target = &s.work;
        let k = known("claude-code").unwrap();
        let device = json!({"device_id":M1,"name":"M1"});
        let folder = format!("{BASE}/claude-code/{M1}");
        let catalog = Catalog::open(&s.home).unwrap();
        let updates = |device| working_updates(&catalog, user, device, [target.as_path()]).unwrap();
        let copy =
            || working_copy(&s.backend, &source, k, target, &folder, &device, false).unwrap();
        assert!(updates(M1).is_empty(), "a disk alone is not a destination");
        assert_eq!(copy()["copied"], 2);
        let manifest = target.join(&folder).join("_nuun/working.json");
        let initial = nu9ve_volumes::stamp(&manifest).unwrap();
        assert_eq!(copy()["copied"], 0);
        assert_eq!(
            stamp_json(&nu9ve_volumes::stamp(&manifest).unwrap()),
            stamp_json(&initial)
        );
        assert!(updates(M1).is_empty());

        let old = source.join("projects/repo/s1.jsonl");
        let time = std::fs::metadata(&old).unwrap().modified().unwrap();
        let replacement = source.join("replacement");
        std::fs::write(&replacement, b"{\"turn\":2}\n").unwrap();
        std::fs::File::open(&replacement)
            .unwrap()
            .set_modified(time)
            .unwrap();
        std::fs::rename(&replacement, &old).unwrap();
        Root::open(&source)
            .unwrap()
            .write_bytes("projects/repo/extra.jsonl", b"x", false)
            .unwrap();
        assert_eq!(updates(M1)[target.as_path()][0]["count"], 2);
        assert!(updates("b56b874c-f743-493f-9b12-75a509c68829").is_empty());
        assert_eq!(copy()["copied"], 2);
        assert!(updates(M1).is_empty());
        assert_eq!(copy()["copied"], 0);
    }
}
