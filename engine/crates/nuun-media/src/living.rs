//! Incremental, verified backups that preserve all archived versions.

use crate::catalog::{Catalog, WritableCatalog, sql, writer_lock};
use crate::delivery::ActionProgress;
use crate::release::checked_hash;
use crate::rules::relative;
use crate::transfer::{Hook, Target, Transfer};
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend, truthy};
use nu9ve_json::{Error, NAMESPACE_URL, Result, digest, require, uid, uuid5};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "nu9ve-living-plan/1";
const IDENTITY: [&str; 4] = ["mode", "volume_uuid", "disk_uuid", "root_inode"];

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

pub(crate) fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
}

pub(crate) fn source(path: &str) -> Result<PathBuf> {
    let path = crate::planner::absolute_source(path)?;
    require(
        path.is_dir() && !path.is_symlink(),
        "invalid_request",
        format!("Source folder not found: {}", path.display()),
    )?;
    Ok(path)
}

pub(crate) fn identity(info: &Value) -> Value {
    Value::Object(
        IDENTITY
            .into_iter()
            .map(|key| (key.into(), info[key].clone()))
            .collect(),
    )
}

pub(crate) fn validate_source(
    backend: &dyn Backend,
    root: &Path,
    expected: &Value,
) -> Result<Value> {
    let actual = backend.describe(root, false)?;
    require(
        identity(&actual) == *expected,
        "source_changed",
        "Source folder identity changed",
    )?;
    Ok(actual)
}

pub(crate) fn stored(catalog: &Catalog, key: &str, params: &Value) -> Result<Value> {
    let digest = required(params, "digest")?;
    catalog
        .get(key)?
        .filter(|plan| plan["digest"] == digest)
        .ok_or_else(|| Error::new("plan_changed", "Use the recorded reviewed plan"))
}

fn folder_ok(folder: &str) -> Result<()> {
    relative(folder)?;
    require(
        folder.split('/').next() != Some(".ncreator")
            && !folder.split('/').any(|part| part == "_nuun"),
        "invalid_request",
        "Reserved archive folder",
    )
}

fn check_folder(catalog: &Catalog, cid: &str, archive: &str, folder: &str) -> Result<()> {
    for other in crate::reads::living_collections(catalog)?
        .as_array()
        .into_iter()
        .flatten()
    {
        let old = text(other, "folder");
        require(
            other["id"] == cid
                || other["archive_id"] != archive
                || !(Path::new(old).starts_with(folder) || Path::new(folder).starts_with(old)),
            "overlapping_roots",
            format!("Another collection already uses this archive folder: {old}"),
        )?;
    }
    Ok(())
}

fn index(catalog: &Catalog, cid: &str) -> Result<BTreeMap<String, Value>> {
    let query = "SELECT json_object('path',path,'size',size,'mtime_ns',mtime_ns,'sha256',sha256,'archive_path',archive_path)
        FROM living_files WHERE collection_id=?";
    match catalog.json_rows(query, [cid]) {
        Ok(rows) => Ok(rows
            .into_iter()
            .map(|row| (text(&row, "path").to_owned(), row))
            .collect()),
        Err(error) if error.message.contains("no such table") => Ok(BTreeMap::new()),
        Err(error) => Err(error),
    }
}

fn external(root: &Path, collections: &Value, cid: &str) -> Result<Value> {
    let mut refs = BTreeMap::<String, BTreeSet<String>>::new();
    let (sets, unreadable) = crate::ableton::references(root, true, |set, target| {
        if !Path::new(target).starts_with(root) {
            refs.entry(target.into())
                .or_default()
                .insert(set.split('/').next().unwrap_or(set).into());
        }
    })?;
    let mut missing = 0;
    let mut uncovered = Vec::new();
    let mut bytes = 0;
    let count = refs.len();
    for (path, users) in refs {
        let Ok(stat) = std::fs::metadata(&path) else {
            missing += 1;
            continue;
        };
        if !stat.is_file() {
            missing += 1;
            continue;
        }
        if collections.as_array().into_iter().flatten().any(|c| {
            c["id"] != cid
                && Path::new(&path)
                    .strip_prefix(text(c, "source"))
                    .is_ok_and(|rel| !rel.as_os_str().is_empty())
        }) {
            continue;
        }
        bytes += stat.len();
        uncovered.push(json!({"path":path,"exists":true,"covered_by":null,
            "used_by":users.into_iter().take(5).collect::<Vec<_>>(),"bytes":stat.len()}));
    }
    Ok(
        json!({"sets":sets,"unreadable":unreadable,"references":count,"missing":missing,
        "not_covered":uncovered,"not_covered_bytes":bytes}),
    )
}

pub fn plan(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    require(
        params.as_object().is_some_and(|p| {
            p.keys().all(|k| {
                ["source", "archive", "folder", "name", "progress_id"].contains(&k.as_str())
            })
        }),
        "invalid_request",
        "Unknown parameters",
    )?;
    let root = source(required(params, "source")?)?;
    let folder = required(params, "folder")?;
    folder_ok(folder)?;
    let catalog = Catalog::open(home)?;
    let record = catalog.volume(required(params, "archive")?)?;
    require(
        record["role"] == "archive",
        "invalid_role",
        "Archive required",
    )?;
    let info = volumes::check(backend, &record)?;
    let archive = Path::new(text(&record, "root"));
    require(
        !root.starts_with(archive) && !archive.starts_with(&root),
        "overlapping_roots",
        "Source overlaps the archive",
    )?;
    let original = backend.describe(&root, false)?;
    require(
        volumes::independent(&original, &info),
        "same_physical_disk",
        "Archive and source must be independent disks",
    )?;
    let archive_id = text(&record, "id");
    let cid = uuid5(
        &NAMESPACE_URL,
        &format!("nu9ve-living:{}|{archive_id}|{folder}", root.display()),
    );
    check_folder(&catalog, &cid, archive_id, folder)?;
    let name = params["name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            root.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
        });
    let writer = WritableCatalog::open(home)?;
    let mut progress = ActionProgress::new(&writer, params["progress_id"].as_str(), "preparing");
    progress.report()?;
    let known = index(&catalog, &cid)?;
    let origin = Root::open(&root)?;
    let target = Root::open(archive)?;
    let (mut new, mut changed, mut present) = (Vec::new(), Vec::new(), BTreeSet::new());
    let (mut same, mut same_bytes, mut bytes) = (0, 0, 0);
    crate::coverage::walk(&origin, "", |path, size| {
        require(
            size.is_some(),
            "unsafe_file",
            format!("Links/special files are not followed: {path}"),
        )?;
        require(
            !path.starts_with("_nuun/"),
            "invalid_request",
            "The source has a reserved _nuun folder",
        )?;
        let stamp = origin
            .stat(&path)?
            .ok_or_else(|| Error::new("source_changed", format!("Source changed: {path}")))?;
        let stamp = stamp_json(&stamp);
        present.insert(path.clone());
        let previous = known.get(&path);
        if previous.is_some_and(|row| {
            row["size"] == stamp["size"]
                && row["mtime_ns"] == stamp["mtime_ns"]
                && target
                    .stat(text(row, "archive_path"))
                    .ok()
                    .flatten()
                    .is_some_and(|s| row["size"] == s.size)
        }) {
            same += 1;
            same_bytes += size.unwrap();
        } else {
            let dest = format!("{folder}/{path}");
            let item = json!({"path":path,"stamp":stamp,"archive_path":dest});
            if previous.is_some() {
                changed.push(item);
            } else {
                new.push(item);
            }
            bytes += size.unwrap();
        }
        if present.len().is_multiple_of(2000) {
            progress.stage = "listing_source";
            progress.meter.files_done = present.len();
            progress.report()?;
        }
        Ok(())
    })?;
    let gone: Vec<_> = known
        .keys()
        .filter(|path| !present.contains(*path))
        .collect();
    require(
        info["free"].as_u64().is_none_or(|free| bytes <= free),
        "insufficient_space",
        format!(
            "Not enough free space on {} for {bytes} bytes",
            record["name"].as_str().unwrap_or_default()
        ),
    )?;
    let id = uid()?;
    let mut value = json!({"schema":SCHEMA,"id":id,"created_at":nu9ve_time::now_utc(),"collection_id":cid,
        "name":name,"source":root,"source_identity":identity(&original),"archive_id":archive_id,"folder":folder,
        "new":new,"changed":changed,"unchanged":{"files":same,"bytes":same_bytes},
        "gone_from_source":gone.iter().take(1000).collect::<Vec<_>>(),"gone_count":gone.len(),"bytes":bytes,
        "free":info["free"],"ableton":external(&root,&crate::reads::living_collections(&catalog)?,&cid)?,
        "state":"review","done":{}});
    value["digest"] = json!(digest(&value));
    let _lock = writer_lock(home)?;
    check_folder(&catalog, &cid, archive_id, folder)?;
    writer.set(&format!("living-plan:{id}"), &value)?;
    Ok(value)
}

pub fn apply(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    apply_with(home, backend, params, &|_, _| Ok(()))
}

fn apply_with(home: &Path, backend: &dyn Backend, params: &Value, hook: Hook<'_>) -> Result<Value> {
    let id = required(params, "plan_id")?;
    let key = format!("living-plan:{id}");
    let _lock = writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let mut value = stored(&catalog, &key, params)?;
    if value["state"] == "completed" {
        return Ok(value);
    }
    require(
        value["schema"] == SCHEMA && value["id"] == id,
        "invalid_plan",
        "Invalid living collection plan",
    )?;
    let source = PathBuf::from(required(&value, "source")?);
    let record = catalog.volume(required(&value, "archive_id")?)?;
    let target = Root::open(Path::new(text(&record, "root")))?;
    let info = volumes::check(backend, &record)?;
    let original = validate_source(backend, &source, &value["source_identity"])?;
    require(
        record["role"] == "archive",
        "invalid_role",
        "Archive required",
    )?;
    require(
        !source.starts_with(&target.path) && !target.path.starts_with(&source),
        "overlapping_roots",
        "Source overlaps the archive",
    )?;
    require(
        volumes::independent(&original, &info),
        "same_physical_disk",
        "Archive and source must be independent disks",
    )?;
    let folder = required(&value, "folder")?.to_owned();
    folder_ok(&folder)?;
    let cid = required(&value, "collection_id")?.to_owned();
    check_folder(&catalog, &cid, text(&record, "id"), &folder)?;
    let collection_key = format!("living-collection:{cid}");
    let writer = WritableCatalog::open(home)?;
    writer.db().execute_batch("CREATE TABLE IF NOT EXISTS living_files(collection_id TEXT, path TEXT, size INTEGER,
        mtime_ns INTEGER, sha256 TEXT, archive_path TEXT, verified_at TEXT, PRIMARY KEY(collection_id,path))").map_err(sql)?;
    let origin = Root::open(&source)?;
    let now = nu9ve_time::now_utc();
    let run_key = format!("living-run:{id}");
    let run = writer
        .get(&run_key)?
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| {
            format!(
                "{}-{}",
                now[..19].replace(['-', ':'], "").replace('T', "-"),
                id.chars().take(8).collect::<String>()
            )
        });
    relative(&run)?;
    let mut collection = json!({"id":cid,"name":value["name"],"source":source,"archive_id":record["id"],
        "folder":folder,"last_plan":id,"updated_at":now});
    // Only one in-progress backup for this collection may write its live paths.
    if let Some(previous) = writer.get(&collection_key)? {
        let last = text(&previous, "last_plan");
        require(
            last == id
                || writer
                    .get(&format!("living-plan:{last}"))?
                    .is_none_or(|plan| plan["state"] != "applying"),
            "plan_changed",
            "Resume the in-progress collection backup first",
        )?;
    }
    value["state"] = json!("applying");
    let tx = writer.db().unchecked_transaction().map_err(sql)?;
    writer.set(&run_key, &json!(run))?;
    writer.set(&key, &value)?;
    writer.set(&collection_key, &collection)?;
    tx.commit().map_err(sql)?;

    let items: Vec<_> = ["new", "changed"]
        .into_iter()
        .flat_map(|key| value[key].as_array().into_iter().flatten().cloned())
        .collect();
    let mut progress = ActionProgress::new(&writer, Some(id), "copying");
    progress.meter.total = value["bytes"].as_u64().unwrap_or_default();
    progress.meter.files_total = items.len();
    let mut transfer = Transfer::new(home, &writer, backend, &|_, _| Ok(()));
    let mut processed = 0u64;
    let mut versions = Vec::new();
    let mut totals = BTreeMap::<String, u64>::new();
    for item in &items {
        let path = relative(required(item, "path")?)?;
        let dest = format!("{folder}/{path}");
        require(
            item["archive_path"] == dest && path.split('/').next() != Some("_nuun"),
            "invalid_plan",
            "Invalid living collection member",
        )?;
        let version_key = format!("living-version:{id}:{path}");
        let aside = format!("{folder}/_nuun/versions/{run}/{path}");
        let prior_version = writer.get(&version_key)?;
        let size = item["stamp"]["size"].as_u64().unwrap_or_default();
        if !truthy(&value["done"][path]) {
            progress
                .meter
                .holding(path, text(&value, "name"), text(&record, "name"), "copy");
            progress.report()?;
            hook("before_copy", item)?;
            backend.fast_check(&source, &original)?;
            if origin
                .stat(path)?
                .is_none_or(|stamp| stamp_json(&stamp) != item["stamp"])
            {
                value["done"][path] = json!("changed_since_review");
                writer.set(&key, &value)?;
            } else {
                let copy = |transfer: &mut Transfer<'_>| {
                    transfer.copy_many(
                        &origin,
                        path,
                        &[Target {
                            root: &target,
                            rel: dest.clone(),
                            record: &record,
                        }],
                        None,
                        Some(&item["stamp"]),
                    )
                };
                let existed = target.stat(&dest)?.is_some();
                let mut versioned = false;
                if let Some(prior) = &prior_version
                    && let Some((sha, stamp)) = checked_hash(&target, &aside)?
                {
                    require(
                        prior["sha256"] == sha && prior["stamp"]["inode"] == stamp["inode"],
                        "checksum_mismatch",
                        "Archived prior version changed",
                    )?;
                    versioned = true;
                }
                let result = match copy(&mut transfer) {
                    Err(error) if error.code == "destination_conflict" && !versioned => {
                        let (sha, stamp) = checked_hash(&target, &dest)?.ok_or_else(|| {
                            Error::new("source_changed", "Archive changed before versioning")
                        })?;
                        if let Some(prior) = &prior_version {
                            require(
                                prior["sha256"] == sha && prior["stamp"] == stamp,
                                "source_changed",
                                "Archive changed before versioning",
                            )?;
                        }
                        writer.set(&version_key, &json!({"sha256":sha,"stamp":stamp}))?;
                        target.parent(&aside, true)?;
                        require(
                            target
                                .stat(&dest)?
                                .is_some_and(|now| stamp_json(&now) == stamp),
                            "source_changed",
                            "Archive changed before versioning",
                        )?;

                        target.rename_new(&dest, &aside)?;
                        hook("after_version", item)?;
                        versioned = true;
                        copy(&mut transfer)?
                    }
                    other => other?,
                };
                let state = if versioned {
                    "versioned"
                } else if existed {
                    "adopted"
                } else {
                    "copied"
                };
                value["done"][path] = json!(state);
                // If a transfer is interrupted before the journal is committed, retry adopts verified bytes.
                hook("after_copy", item)?;
                let tx = writer.db().unchecked_transaction().map_err(sql)?;
                tx.execute(
                    "INSERT OR REPLACE INTO living_files VALUES(?,?,?,?,?,?,?)",
                    (
                        &cid,
                        path,
                        size,
                        item["stamp"]["mtime_ns"].as_i64().unwrap_or_default(),
                        &result[0].0,
                        &dest,
                        nu9ve_time::now_utc(),
                    ),
                )
                .map_err(sql)?;
                if versioned {
                    writer.event(
                        &cid,
                        "living_versioned",
                        &json!({"path":path,"aside":aside,"plan_id":id}),
                    )?;
                }
                writer.set(&key, &value)?;
                tx.commit().map_err(sql)?;
            }
        }
        let state = text(&value["done"], path);
        *totals.entry(state.to_owned()).or_default() += 1;
        if state == "versioned" {
            versions.push(json!({"path":path,"aside":aside}));
        }
        if state != "changed_since_review" {
            processed += size;
        }
        progress.meter.reach(processed);
        progress.meter.finished(state, path, text(&record, "name"));
    }
    volumes::check(backend, &record)?;
    let files = index(&catalog,&cid)?.into_iter().map(|(path,row)| (path,
        json!({"size":row["size"],"sha256":row["sha256"],"archive_path":row["archive_path"]}))).collect::<serde_json::Map<_,_>>();
    target.write_json(&format!("{folder}/_nuun/manifest.json"),&json!({"schema":"nu9ve-living-manifest/1",
        "collection_id":cid,"name":value["name"],"source":source,"updated_at":nu9ve_time::now_utc(),"files":files}))?;
    let now = nu9ve_time::now_utc();
    collection["updated_at"] = json!(now);
    value["state"] = json!("completed");
    value["completed_at"] = json!(now);
    value["result"] = json!({"copied":totals.get("copied").copied().unwrap_or_default(),
        "adopted":totals.get("adopted").copied().unwrap_or_default(),"versioned":versions.len(),
        "changed_since_review":totals.get("changed_since_review").copied().unwrap_or_default(),
        "versions":versions.iter().take(1000).collect::<Vec<_>>()});
    let mut payload = value["result"].clone();
    payload["plan_id"] = json!(id);
    payload["bytes"] = json!(processed);
    payload["versions"] = json!(versions.len());
    let tx = writer.db().unchecked_transaction().map_err(sql)?;
    writer.set(&key, &value)?;
    writer.set(&collection_key, &collection)?;
    writer.event(&cid, "living_backup_completed", &payload)?;
    tx.commit().map_err(sql)?;
    progress.stage = "complete";
    progress.meter.reach(progress.meter.total);
    progress.meter.current.clear();
    progress.report()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_resume_after_rename_and_deleted_sources_stay_archived() {
        let s = crate::volumes::test_backend::setup();
        let source = Root::open(&s.card).unwrap();
        source
            .write_bytes("tracks/one.wav", b"original", false)
            .unwrap();
        source
            .write_bytes("tracks/gone.wav", b"keep", false)
            .unwrap();
        let review = || {
            plan(
                &s.home,
                &s.backend,
                &json!({"source":s.card,"archive":"MEDIA","folder":"Music/backup"}),
            )
            .unwrap()
        };
        let args = |plan: &Value| json!({"plan_id":plan["id"],"digest":plan["digest"]});
        let first = apply(&s.home, &s.backend, &args(&review())).unwrap();
        assert_eq!(first["result"]["copied"], 2);
        assert_eq!(review()["unchanged"]["files"], 2);
        source
            .write_bytes("tracks/one.wav", b"new revision", true)
            .unwrap();
        source.unlink("tracks/gone.wav", None).unwrap();
        let second = review();
        assert_eq!(second["gone_from_source"], json!(["tracks/gone.wav"]));
        let args = args(&second);
        let error = apply_with(&s.home, &s.backend, &args, &|stage, _| {
            require(stage != "after_version", "interrupted", "fixture")
        })
        .unwrap_err();
        assert_eq!(error.code, "interrupted");
        let result = apply(&s.home, &s.backend, &args).unwrap();
        assert_eq!(result["result"]["versioned"], 1);
        let aside = text(&result["result"]["versions"][0], "aside");
        assert_eq!(std::fs::read(s.archive.join(aside)).unwrap(), b"original");
        assert_eq!(
            std::fs::read(s.archive.join("Music/backup/tracks/one.wav")).unwrap(),
            b"new revision"
        );
        assert_eq!(
            std::fs::read(s.archive.join("Music/backup/tracks/gone.wav")).unwrap(),
            b"keep"
        );
        assert_eq!(review()["unchanged"]["files"], 1);
        assert_eq!(apply(&s.home, &s.backend, &args).unwrap(), result);
    }
}
