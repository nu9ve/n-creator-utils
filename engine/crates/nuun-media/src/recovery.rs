//! Recover jobs from verified portable archive receipts.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::planner::validate_plan;
use crate::transfer::{Transfer, hash_file};
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, Result, canonical, digest, require};
use nu9ve_volumes::fs;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn malformed() -> Error {
    Error::new("invalid_receipt", "Malformed transfer receipt")
}

fn json_text(value: &Value) -> String {
    String::from_utf8(canonical(value)).unwrap_or_default()
}

pub fn recover(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let volume = params
        .get("volume")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("operation_failed", "'volume'"))?;
    let _lock = writer_lock(home)?;
    let record = Catalog::open(home)?.volume(volume)?;
    require(
        record["role"] == "archive",
        "invalid_role",
        "Recover from an archive volume",
    )?;
    volumes::check(backend, &record)?;
    let tree = Root::open(Path::new(text(&record, "root")))?;
    let (directory, _) = tree.parent(".ncreator/receipts/entry.json", false)?;
    let mut names = std::fs::read_dir(&directory)
        .map_err(|e| fs::io_error(e, &directory))?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| fs::io_error(e, &directory))?;
    names.sort();
    let catalog = WritableCatalog::open(home)?;
    let db = catalog.db();
    let mut restored = Vec::new();
    let mut publications = Vec::new();
    const LIMIT: u64 = 16 * 1024 * 1024;
    for name in names {
        let Some(name) = name
            .to_str()
            .filter(|name| name.ends_with(".json") && !name.starts_with('.'))
        else {
            continue;
        };
        let path = format!(".ncreator/receipts/{name}");
        let file = tree.open_read(&path)?.ok_or_else(malformed)?;
        let mut raw = Vec::new();
        file.take(LIMIT + 1)
            .read_to_end(&mut raw)
            .map_err(|e| fs::io_error(e, &directory.join(name)))?;
        require(
            raw.len() as u64 <= LIMIT,
            "receipt_too_large",
            "Receipt exceeds input limit",
        )?;
        let receipt: Value = serde_json::from_slice(&raw)
            .map_err(|e| Error::new("operation_failed", e.to_string()))?;
        let schema = text(&receipt, "schema");
        if !schema.starts_with("nu9ve-transfer-receipt/") {
            continue;
        }
        require(
            matches!(
                schema,
                "nu9ve-transfer-receipt/1" | "nu9ve-transfer-receipt/2"
            ),
            "unsupported_receipt",
            "Unsupported transfer receipt version",
        )?;
        require(
            receipt["plan"].is_object(),
            "unsupported_receipt",
            "Receipt has no recoverable plan",
        )?;
        let original = &receipt["plan"];
        validate_plan(original)?;
        require(
            original["id"] == receipt["job_id"] && original["digest"] == receipt["plan_digest"],
            "invalid_receipt",
            "Plan/receipt mismatch",
        )?;
        let expected: BTreeMap<_, _> = original["files"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|file| (text(file, "path"), file))
            .collect();
        let copies = receipt["files"].as_array().ok_or_else(malformed)?;
        let received: BTreeSet<_> = copies.iter().map(|copy| text(copy, "path")).collect();
        require(
            copies.len() == expected.len() && received.iter().copied().eq(expected.keys().copied()),
            "invalid_receipt",
            "Receipt membership mismatch",
        )?;
        let mut verified = Vec::new();
        for copy in copies {
            let rel = crate::rules::relative(text(copy, "destination"))?;
            let sha = text(copy, "sha256");
            require(
                sha.len() == 64
                    && sha
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "invalid_receipt",
                "Invalid receipt hash",
            )?;
            require(
                copy["bytes"] == expected[text(copy, "path")]["stamp"]["size"],
                "invalid_receipt",
                "Receipt size mismatch",
            )?;
            let before = tree.stat(rel)?.ok_or_else(|| {
                Error::new("operation_failed", format!("[FileNotFoundError] {rel}"))
            })?;
            let file = tree.open_read(rel)?.ok_or_else(|| {
                Error::new("operation_failed", format!("[FileNotFoundError] {rel}"))
            })?;
            require(
                json!(before.size) == copy["bytes"]
                    && hash_file(&mut fs::verify_read(file), |_| {})? == sha,
                "checksum_mismatch",
                "Archive member does not match receipt",
            )?;
            require(
                tree.stat(rel)?.as_ref() == Some(&before),
                "source_changed",
                "Archive member changed while hashing",
            )?;
            let mut copy = copy.clone();
            copy["volume_id"] = record["id"].clone();
            copy["receipt_id"] = receipt["receipt_id"].clone();
            copy["stamp"] = stamp_json(&before);
            copy["verified_at"] = json!(nu9ve_time::now_utc());
            verified.push(copy);
        }
        volumes::check(backend, &record)?;
        let units = receipt["units"].as_array().ok_or_else(malformed)?;
        let id = text(original, "id");
        let planned_units: BTreeMap<_, _> = original["units"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|unit| (text(unit, "id"), unit))
            .collect();
        let received_units: BTreeSet<_> = units.iter().map(|unit| text(unit, "id")).collect();
        require(
            units.len() == planned_units.len()
                && received_units
                    .iter()
                    .copied()
                    .eq(planned_units.keys().copied()),
            "invalid_receipt",
            "Receipt unit membership differs from the reviewed plan",
        )?;
        let tx = db.unchecked_transaction().map_err(sql)?;
        for unit in units {
            let support = crate::volumes::truthy(&unit["support"]);
            let planned = planned_units[text(unit, "id")];
            require(
                crate::volumes::truthy(&planned["support"]) == support
                    && planned["profile"] == unit["profile"]
                    && planned["project"] == unit["project"],
                "invalid_receipt",
                "Receipt units differ from the reviewed plan",
            )?;
            if support {
                require(
                    receipt["schema"] == "nu9ve-transfer-receipt/2" && unit["project_id"].is_null(),
                    "invalid_receipt",
                    "Camera support needs receipt v2 without an editorial project",
                )?;
                continue;
            }
            let project =
                crate::captures::python_uuid(text(unit, "project_id")).ok_or_else(malformed)?;
            let profile = text(unit, "profile");
            let alias = text(unit, "project");
            let current: Option<String> = tx
                .query_row(
                    "SELECT project_id FROM project_aliases WHERE profile=? AND alias=?",
                    [profile, alias],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sql)?;
            require(
                current.as_deref().is_none_or(|id| id == project),
                "project_conflict",
                "Resolve conflicting project identity before recovery",
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO projects VALUES(?,?,?)",
                [&project, profile, alias],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT OR IGNORE INTO project_aliases VALUES(?,?,?)",
                [profile, alias, &project],
            )
            .map_err(sql)?;
        }
        let old: Option<String> = tx
            .query_row("SELECT plan FROM jobs WHERE id=?", [id], |row| row.get(0))
            .optional()
            .map_err(sql)?;
        let plan = if let Some(raw) = old {
            let plan: Value = serde_json::from_str(&raw)
                .map_err(|e| Error::new("catalog_corrupt", e.to_string()))?;
            require(
                plan["files"] == original["files"] && plan["units"] == original["units"],
                "job_conflict",
                "Receipt conflicts with the cataloged job",
            )?;
            plan
        } else {
            let mut plan = original.clone();
            plan["archive_id"] = record["id"].clone();
            plan["work_id"] = Value::Null;
            plan["cleanup"] = json!(false);
            plan["archive_only_confirmed"] = json!(true);
            plan["work_delivery_status"] = json!(if !original["expected_work_id"].is_null()
                && original["work_delivery_status"] != "skipped"
            {
                "pending"
            } else {
                "skipped"
            });
            plan["destinations"] = json!({text(&record, "id"): {"root": record["root"], "prefix": record["prefix"], "role": record["role"]}});
            plan.as_object_mut().unwrap().remove("digest");
            plan["digest"] = json!(digest(&plan));
            let now = nu9ve_time::now_utc();
            tx.execute(
                "INSERT INTO jobs VALUES(?,?,?,?,?,?,?)",
                (
                    id,
                    text(&plan, "digest"),
                    json_text(&plan),
                    "recovered",
                    None::<String>,
                    &now,
                    &now,
                ),
            )
            .map_err(sql)?;
            plan
        };
        for copy in &verified {
            tx.execute(
                "INSERT OR IGNORE INTO members(job_id,relative_path,sha256) VALUES(?,?,?)",
                [id, text(copy, "path"), text(copy, "sha256")],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT OR REPLACE INTO copies VALUES(?,?,?,?,?)",
                (
                    id,
                    text(&record, "id"),
                    text(copy, "path"),
                    text(copy, "sha256"),
                    json_text(copy),
                ),
            )
            .map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        if crate::volumes::truthy(&receipt["revision_publication"]) {
            publications.push((receipt["revision_publication"].clone(), plan));
        } else {
            Transfer::new(home, &catalog, backend, &|_, _| Ok(())).index(&plan)?;
        }
        catalog.event(id, "catalog_recovered", &json!({"volume_id": record["id"]}))?;
        restored.push(id.to_owned());
    }
    recover_publications(home, backend, &catalog, &publications)?;
    Ok(json!({"recovered_jobs": restored, "verified": true}))
}

/// A receipt filename is a UUID, so dependency order must come from the predecessor revisions.
fn recover_publications(
    home: &Path,
    backend: &dyn Backend,
    catalog: &WritableCatalog,
    publications: &[(Value, Value)],
) -> Result<()> {
    if publications.is_empty() {
        return Ok(());
    }
    let db = catalog.db();
    let revision = |asset: &Value| -> Result<Option<Value>> {
        let raw: Option<String> = db
            .query_row(
                "SELECT payload FROM asset_revisions WHERE asset_id=? AND revision=?",
                [text(asset, "id"), text(asset, "revision")],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql)?;
        raw.map(|raw| {
            serde_json::from_str(&raw).map_err(|e| Error::new("catalog_corrupt", e.to_string()))
        })
        .transpose()
    };
    let mut pending: Vec<_> = publications
        .iter()
        .map(|(publication, plan)| (publication, plan))
        .collect();
    while !pending.is_empty() {
        let mut progress = false;
        let mut waiting = Vec::new();
        for (publication, plan) in pending {
            let old = &publication["previous_asset"];
            let new = &publication["asset"];
            let Some(predecessor) = revision(old)? else {
                waiting.push((publication, plan));
                continue;
            };
            require(
                predecessor["job_id"] == old["job_id"],
                "invalid_receipt",
                "Predecessor job differs",
            )?;
            require(
                new["id"] == old["id"]
                    && new["job_id"] == plan["id"]
                    && ["project_id", "profile", "session_id"]
                        .iter()
                        .all(|key| new[*key] == old[*key]),
                "invalid_receipt",
                "Publication identity differs",
            )?;
            Transfer::new(home, catalog, backend, &|_, _| Ok(())).index(plan)?;
            require(
                revision(new)?.is_some_and(|indexed| indexed["job_id"] == new["job_id"]),
                "invalid_receipt",
                "Publication revision differs from verified members",
            )?;
            progress = true;
        }
        require(
            progress,
            "missing_receipt",
            "Recover predecessor receipts before publishing revision history",
        )?;
        pending = waiting;
    }
    let predecessors: BTreeSet<_> = publications
        .iter()
        .map(|(publication, _)| {
            let old = &publication["previous_asset"];
            (text(old, "id"), text(old, "revision"))
        })
        .collect();
    let tx = db.unchecked_transaction().map_err(sql)?;
    for (publication, _) in publications {
        let asset = &publication["asset"];
        let id = text(asset, "id");
        if predecessors.contains(&(id, text(asset, "revision"))) {
            continue;
        }
        let current: String = tx
            .query_row("SELECT payload FROM assets WHERE id=?", [id], |row| {
                row.get(0)
            })
            .map_err(sql)?;
        let current: Value = serde_json::from_str(&current)
            .map_err(|e| Error::new("catalog_corrupt", e.to_string()))?;
        if predecessors.contains(&(id, text(&current, "revision"))) {
            tx.execute(
                "UPDATE assets SET job_id=?,payload=? WHERE id=?",
                (text(asset, "job_id"), json_text(asset), id),
            )
            .map_err(sql)?;
            tx.execute(
                "UPDATE work SET state='pending',target=NULL,updated_at=? WHERE asset_id=?",
                (nu9ve_time::now_utc(), id),
            )
            .map_err(sql)?;
        }
    }
    tx.commit().map_err(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_revisions_recover_in_predecessor_order() {
        let s = crate::volumes::test_backend::setup();
        std::fs::write(s.card.join("clip.MP4"), b"first").unwrap();
        let plan = crate::planner::build_plan(
            &s.home,
            &s.backend,
            &json!({
                "source": s.card, "archive": "MEDIA", "work": "ESSD", "project": "intro"
            }),
        )
        .unwrap();
        crate::transfer::apply(
            &s.home,
            &s.backend,
            &plan,
            text(&plan, "digest"),
            &|_, _| Ok(()),
        )
        .unwrap();
        let archive = Root::open(&s.archive).unwrap();
        let original = format!(".ncreator/receipts/{}.json", text(&plan, "id"));
        let mut receipt: Value =
            serde_json::from_slice(&std::fs::read(s.archive.join(&original)).unwrap()).unwrap();
        std::fs::rename(
            s.archive.join(&original),
            s.archive.join(".ncreator/receipts/z-original.json"),
        )
        .unwrap();
        let original = Catalog::open(&s.home)
            .unwrap()
            .json_rows("SELECT payload FROM assets", [])
            .unwrap()
            .remove(0);
        let asset_id = text(&original, "id");
        let mut previous = original.clone();
        for (filename, id, bytes) in [
            (
                "b-second",
                "c8291e8f-0ddd-4f03-b745-b8bbd15832d9",
                b"second".as_slice(),
            ),
            (
                "a-third",
                "d9391e8f-0ddd-4f03-b745-b8bbd15832d9",
                b"third".as_slice(),
            ),
        ] {
            let path = format!("001_raw/revisions/{id}/clip.MP4");
            archive.write_bytes(&path, bytes, false).unwrap();
            let stamp = stamp_json(&archive.stat(&path).unwrap().unwrap());
            let hash = hash_file(&mut archive.open_read(&path).unwrap().unwrap(), |_| {}).unwrap();
            let revision = digest(&json!([{"path": "clip.MP4", "sha256": hash}]));
            let mut asset = previous.clone();
            asset["revision"] = json!(revision);
            asset["job_id"] = json!(id);
            let mut plan = plan.clone();
            plan["id"] = json!(id);
            plan["files"][0]["stamp"] = stamp.clone();
            plan["layout"]["clip.MP4"]["archive"] = json!(path.strip_prefix("001_raw/").unwrap());
            plan.as_object_mut().unwrap().remove("digest");
            plan["digest"] = json!(digest(&plan));
            receipt["job_id"] = json!(id);
            receipt["plan_digest"] = plan["digest"].clone();
            receipt["plan"] = plan;
            receipt["files"][0]["job_id"] = json!(id);
            receipt["files"][0]["destination"] = json!(path);
            receipt["files"][0]["stamp"] = stamp;
            receipt["files"][0]["bytes"] = json!(bytes.len());
            receipt["files"][0]["sha256"] = json!(hash);
            receipt["revision_publication"] =
                json!({"previous_asset": previous, "asset": asset, "operation_id": id});
            archive
                .write_json(&format!(".ncreator/receipts/{filename}.json"), &receipt)
                .unwrap();
            previous = asset;
        }
        let home = s.home.with_file_name("recovered");
        let database = crate::catalog::fixture::create(&home);
        crate::catalog::fixture::register(
            &database,
            "MEDIA",
            "archive",
            &s.archive,
            &s.backend.describe(&s.archive, false).unwrap(),
            "001_raw",
        );
        drop(database);
        let args = json!({"volume": "MEDIA"});
        assert_eq!(
            recover(&home, &s.backend, &args).unwrap()["recovered_jobs"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        recover(&home, &s.backend, &args).unwrap();
        let reader = Catalog::open(&home).unwrap();
        assert_eq!(
            reader
                .json_rows(
                    "SELECT payload FROM asset_revisions WHERE asset_id=?",
                    [asset_id]
                )
                .unwrap()
                .len(),
            3
        );
        let current = reader
            .json_rows("SELECT payload FROM assets WHERE id=?", [asset_id])
            .unwrap()
            .remove(0);
        assert_eq!(current["revision"], previous["revision"]);
        assert_eq!(current["job_id"], previous["job_id"]);
    }

    #[test]
    fn recover_and_restore_verified_bytes_without_the_source() {
        let s = crate::volumes::test_backend::setup();
        std::fs::write(s.card.join("clip.MP4"), b"original media").unwrap();
        let plan = crate::planner::build_plan(
            &s.home,
            &s.backend,
            &json!({
                "source": s.card, "archive": "MEDIA", "work": "ESSD", "project": "intro"
            }),
        )
        .unwrap();
        crate::transfer::apply(
            &s.home,
            &s.backend,
            &plan,
            text(&plan, "digest"),
            &|_, _| Ok(()),
        )
        .unwrap();
        let receipt = Catalog::open(&s.home)
            .unwrap()
            .json_rows("SELECT payload FROM copies WHERE volume_id='id-MEDIA'", [])
            .unwrap()
            .remove(0);
        let archived = s.archive.join(text(&receipt, "destination"));
        std::fs::remove_dir_all(&s.card).unwrap();
        let home = s.home.with_file_name("recovered");
        let database = crate::catalog::fixture::create(&home);
        crate::catalog::fixture::register(
            &database,
            "MEDIA",
            "archive",
            &s.archive,
            &s.backend.describe(&s.archive, false).unwrap(),
            "001_raw",
        );
        drop(database);
        let params = json!({"volume": "MEDIA"});
        std::fs::write(&archived, b"corrupted data").unwrap();
        assert_eq!(
            recover(&home, &s.backend, &params).unwrap_err().code,
            "checksum_mismatch"
        );
        assert!(
            crate::reads::jobs(&Catalog::open(&home).unwrap())
                .unwrap()
                .is_empty()
        );
        std::fs::write(&archived, b"original media").unwrap();
        let recovered = recover(&home, &s.backend, &params).unwrap();
        assert_eq!(recovered["recovered_jobs"], json!([plan["id"]]));
        assert_eq!(recover(&home, &s.backend, &params).unwrap(), recovered);
        let job = crate::reads::job(
            &Catalog::open(&home).unwrap(),
            &json!({"job_id": plan["id"]}),
        )
        .unwrap();
        assert_eq!(job["state"], "recovered");
        assert_eq!(job["plan"]["cleanup"], false);
        let asset_id = &plan["units"][0]["id"];
        crate::catalog::fixture::register(
            &rusqlite::Connection::open(home.join("catalog.sqlite3")).unwrap(),
            "ESSD",
            "work",
            &s.work,
            &s.backend.describe(&s.work, false).unwrap(),
            "001_proyectos",
        );
        let delivery = json!({"asset_ids": [asset_id], "work": "ESSD", "progress_id": "delivery"});
        let mut offline = s.backend;
        offline.offline.insert(s.work.display().to_string());
        assert_eq!(
            crate::delivery::deliver(&home, &offline, &delivery)
                .unwrap_err()
                .code,
            "volume_offline"
        );
        offline.offline.clear();
        let delivered = crate::delivery::deliver(&home, &offline, &delivery).unwrap();
        assert_eq!(
            delivered,
            json!({"delivered": [asset_id], "volume_id": "id-ESSD"})
        );
        assert_eq!(
            crate::delivery::deliver(&home, &offline, &delivery).unwrap(),
            delivered
        );
        let reader = Catalog::open(&home).unwrap();
        assert!(reader.get("action-progress:delivery").unwrap().is_none());
        let work = reader
            .json_rows("SELECT payload FROM copies WHERE volume_id='id-ESSD'", [])
            .unwrap();
        assert_eq!(
            std::fs::read(s.work.join(text(&work[0], "destination"))).unwrap(),
            b"original media"
        );
        let args = json!({"asset_id": asset_id, "destination": s.work});
        let restored = crate::restore::restore(&home, &offline, &args).unwrap();
        let target = Path::new(text(&restored, "path")).join("clip.MP4");
        assert_eq!(std::fs::read(&target).unwrap(), b"original media");
        assert_eq!(
            crate::restore::restore(&home, &offline, &args).unwrap(),
            restored
        );
        std::fs::write(&target, b"foreign bytes").unwrap();
        assert_eq!(
            crate::restore::restore(&home, &offline, &args)
                .unwrap_err()
                .code,
            "destination_conflict"
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"foreign bytes");
    }
}
