//! What `apply` does after a verified ingest, as `Service._apply_ingest`: the consumer's map of drone units
//! (`missions.unit_entries`, `write_units_map`, Atlas A12) and a consistent snapshot of the catalog on the archive
//! (`metadata_backup.backup_metadata`).

use crate::catalog::WritableCatalog;
use crate::planner::destination_path;
use crate::snapshot::kind_of;
use crate::transfer::{Target, Transfer};
use crate::tree::Root;
use crate::volumes::Backend;
use nu9ve_json::{Error, Result, canonical, require};
use nu9ve_time::DateTime;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const UNIT_SCHEMA: &str = "nu9ve-drone-unit/1";

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
        && value != &json!("")
        && value != &json!({})
        && value != &json!([])
}

/// Python `missions._utc`: UTC of a camera wall time, from its EXIF offset or else the host zone (said so).
fn utc(local: &str, offset: Option<&str>) -> Result<(String, &'static str)> {
    let value = format!("{local}{}", offset.unwrap_or_default());
    let when = DateTime::fromisoformat(&value).ok_or_else(|| {
        Error::new(
            "operation_failed",
            format!("Invalid isoformat string: '{value}'"),
        )
    })?;
    let basis = if offset.is_some_and(|o| !o.is_empty()) {
        "exif_offset"
    } else {
        "host_zone"
    };
    Ok((when.to_utc().isoformat().replace("+00:00", "Z"), basis))
}

/// Python `missions.unit_entries`: one line per still unit of a mission flow, keyed by asset id.
pub fn unit_entries(catalog: &WritableCatalog, plan: &Value, map_rel: &str) -> Result<Vec<Value>> {
    let archive = catalog.volume(text(plan, "archive_id"))?;
    let work_id = [
        &plan["work_id"],
        &plan["expected_work_id"],
        &plan["source_work_id"],
    ]
    .into_iter()
    .find(|v| truthy(v))
    .and_then(Value::as_str);
    let work = work_id.map(|w| catalog.volume(w)).transpose()?;
    // The consumer's root: the parent of `_nuun/`.
    let base = map_rel
        .rsplitn(3, '/')
        .nth(2)
        .unwrap_or_default()
        .to_owned();
    let files: BTreeMap<&str, &Value> = plan["files"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| (text(f, "path"), f))
        .collect();
    let mut entries = Vec::new();
    for unit in plan["units"].as_array().into_iter().flatten() {
        let mission = &unit["mission"];
        if !truthy(mission) {
            continue;
        }
        let mut member: BTreeMap<&str, Value> = BTreeMap::new();
        for path in unit["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let label = match kind_of(path) {
                Some("image") => "jpg",
                Some("raw_image") => "dng",
                _ => continue,
            };
            let sha: Option<String> = catalog
                .db()
                .query_row(
                    "SELECT sha256 FROM members WHERE job_id=? AND relative_path=?",
                    [text(plan, "id"), path],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
            // A still that was already in the work root keeps that location instead of a second copy.
            let work_rel = match unit["work_existing"][path]
                .as_str()
                .filter(|s| !s.is_empty())
            {
                Some(existing) => Some(existing.to_owned()),
                None => match &work {
                    Some(w) => destination_path(w, plan, unit, path, true)?,
                    None => None,
                },
            };
            let shown = work_rel.map(|w| match w.strip_prefix(&format!("{base}/")) {
                Some(inside) => inside.to_owned(),
                None => w,
            });
            member.insert(
                label,
                json!({"path": path, "sha256": sha,
                "archive": destination_path(&archive, plan, unit, path, true)?, "work": shown}),
            );
        }
        let Some(first) = member.get("jpg").or_else(|| member.get("dng")).cloned() else {
            continue;
        };
        let first_path = text(&first, "path");
        let photo = files
            .get(first_path)
            .map(|f| f["photo"].clone())
            .filter(truthy)
            .unwrap_or(json!({}));
        let (at_utc, basis) = match photo["captured_at"].as_str().filter(|c| !c.is_empty()) {
            Some(captured) => {
                let (u, b) = utc(captured, photo["offset"].as_str())?;
                (json!(u), json!(b))
            }
            None => (Value::Null, Value::Null),
        };
        let device = &unit["device"];
        let pick = |label: &str, role: &str| {
            member
                .get(label)
                .map(|m| m[role].clone())
                .unwrap_or(Value::Null)
        };
        let parent = first_path
            .rsplit_once('/')
            .map(|(p, _)| p.rsplit('/').next().unwrap_or(p))
            .unwrap_or("");
        let dcim = if parent.is_empty() {
            Path::new(text(plan, "source"))
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            parent.to_owned()
        };
        let matched = mission["state"] == "matched";
        entries.push(json!({
            "schema": UNIT_SCHEMA, "asset_id": unit["id"], "job_id": plan["id"], "project": unit["project"],
            "archive": {"volume": archive["name"], "jpg": pick("jpg", "archive"), "dng": pick("dng", "archive")},
            "work": {"volume": work.as_ref().map(|w| w["name"].clone()).unwrap_or(Value::Null),
                     "jpg": pick("jpg", "work"), "dng": pick("dng", "work")},
            "original_name": first_path.rsplit('/').next().unwrap_or(first_path),
            "dcim_folder": dcim,
            // A folder on the work disk is not a card: no card identity to report.
            "card_uuid": if truthy(&plan["source_work_id"]) { Value::Null } else { plan["source_identity"]["volume_uuid"].clone() },
            "captured_at": photo["captured_at"], "captured_at_utc": at_utc, "utc_basis": basis,
            "make": device["make"], "model": device["model"], "serial": device["serial"],
            "sha256": {"jpg": pick("jpg", "sha256"), "dng": pick("dng", "sha256")},
            "gps": photo["gps"], "flight": photo["flight"],
            "mission": if matched { json!({"planID": mission["planID"], "sessionID": mission["sessionID"], "captureID": mission["captureID"]}) } else { Value::Null },
            "mission_state": mission["state"], "plan_name": mission["plan_name"], "updated_at": nu9ve_time::now_utc(),
        }));
    }
    Ok(entries)
}

/// Python `missions.write_units_map`: upsert this job's drone units into the consumer's map on the work volume;
/// `pending` when it is offline (remembered for later), `skipped` without missions or a work volume.
pub fn write_units_map(home: &Path, backend: &dyn Backend, plan: &Value) -> Result<String> {
    let catalog = WritableCatalog::open(home)?;
    let recipe = nuun_recipe::effective(catalog.get("recipe")?.as_ref());
    let flows: BTreeMap<&str, &Value> = recipe["missions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| (text(f, "id"), f))
        .collect();
    let used: BTreeSet<String> = plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|u| truthy(&u["mission"]))
        .map(|u| text(&u["mission"], "flow").to_owned())
        .collect();
    let work_id = [
        &plan["work_id"],
        &plan["expected_work_id"],
        &plan["source_work_id"],
    ]
    .into_iter()
    .find(|v| truthy(v))
    .and_then(Value::as_str);
    let (false, Some(work_id)) = (used.is_empty(), work_id) else {
        return Ok("skipped".into());
    };
    let id = text(plan, "id");
    let record = catalog.volume(work_id)?;
    if let Err(e) = crate::volumes::check(backend, &record) {
        if e.code == "volume_offline" || e.code == "volume_replaced" {
            catalog.set(
                &format!("units-map-pending:{id}"),
                &json!({"job_id": id, "since": nu9ve_time::now_utc()}),
            )?;
            return Ok("pending".into());
        }
        return Err(e);
    }
    let _lock = crate::catalog::writer_lock(home)?;
    let tree = Root::open(Path::new(text(&record, "root")))?;
    let flow_of: BTreeMap<&str, &str> = plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|u| truthy(&u["mission"]))
        .map(|u| (text(u, "id"), text(&u["mission"], "flow")))
        .collect();
    for flow_id in &used {
        let flow = flows
            .get(flow_id.as_str())
            .ok_or_else(|| Error::new("operation_failed", format!("'{flow_id}'")))?;
        let rel = format!(
            "{}/{}",
            text(&record, "prefix").trim_matches('/'),
            text(flow, "units_map").trim_matches('/')
        );
        let mut current: BTreeMap<String, Value> = BTreeMap::new();
        let existing =
            crate::tree::read_bytes(Path::new(text(&record, "root")), &rel, 64 * 1024 * 1024)?
                .unwrap_or_default();
        for line in String::from_utf8_lossy(&existing)
            .lines()
            .filter(|l| !l.trim().is_empty())
        {
            let value: Value = serde_json::from_str(line)
                .map_err(|e| Error::new("operation_failed", e.to_string()))?;
            current.insert(text(&value, "asset_id").to_owned(), value);
        }
        for entry in unit_entries(&catalog, plan, &rel)? {
            if flow_of.get(text(&entry, "asset_id")) == Some(&flow_id.as_str()) {
                current.insert(text(&entry, "asset_id").to_owned(), entry);
            }
        }
        let mut ordered: Vec<Value> = current.into_values().collect();
        ordered.sort_by(|a, b| {
            (text(a, "captured_at"), text(a, "asset_id"))
                .cmp(&(text(b, "captured_at"), text(b, "asset_id")))
        });
        let mut data = Vec::new();
        for entry in &ordered {
            data.extend(canonical(entry));
            data.push(b'\n');
        }
        tree.write_bytes(&rel, &data, true)?;
        let back =
            crate::tree::read_bytes(Path::new(text(&record, "root")), &rel, 64 * 1024 * 1024)?;
        require(
            back.as_deref() == Some(&data[..]),
            "checksum_mismatch",
            "units map did not read back identical",
        )?;
    }
    catalog.set(&format!("units-map-pending:{id}"), &Value::Null)?;
    Ok("written".into())
}

/// Python `metadata_backup.backup_metadata`: a consistent snapshot of the whole catalog (every table, projects,
/// aliases, profiles, tags, settings, journals), verified and copied to `.ncreator/catalog-backups/` on the archive.
pub fn backup_metadata(home: &Path, backend: &dyn Backend, archive: &str) -> Result<Value> {
    let _lock = crate::catalog::writer_lock(home)?;
    let catalog = WritableCatalog::open(home)?;
    backup_metadata_locked(home, backend, archive, &catalog)
}

pub(crate) fn backup_metadata_locked(
    home: &Path,
    backend: &dyn Backend,
    archive: &str,
    catalog: &WritableCatalog,
) -> Result<Value> {
    let record = catalog.volume(archive)?;
    require(
        record["role"] == "archive",
        "invalid_role",
        "Metadata backup requires an archive disk",
    )?;
    crate::volumes::check(backend, &record)?;
    let name = format!(
        "{}-{}.sqlite3",
        nu9ve_time::now_utc().replace(':', "-"),
        nu9ve_json::uid()?
    );
    let temporary = tempfile_in(home)?;
    let result = (|| -> Result<Value> {
        let path = temporary.join("catalog.sqlite3");
        // SQLite's own consistent copy (Python uses the backup API; both are a transactionally consistent image).
        catalog
            .db()
            .execute("VACUUM INTO ?", [path.to_string_lossy().as_ref()])
            .map_err(|e| Error::new("invalid_backup", e.to_string()))?;
        let check: String = rusqlite::Connection::open(&path)
            .and_then(|c| c.query_row("PRAGMA integrity_check", [], |r| r.get(0)))
            .map_err(|e| Error::new("invalid_backup", e.to_string()))?;
        require(
            check == "ok",
            "invalid_backup",
            "Catalog snapshot failed integrity check",
        )?;
        let source = Root::open(&temporary)?;
        let target = Root::open(Path::new(text(&record, "root")))?;
        let mut file = nu9ve_volumes::fs::open_read(&path)?
            .ok_or_else(|| Error::new("invalid_backup", "Snapshot vanished"))?;
        let sha = {
            use sha2::Digest;
            use std::io::Read;
            let mut hasher = sha2::Sha256::new();
            let mut buffer = vec![0u8; 1 << 20];
            loop {
                let n = file
                    .read(&mut buffer)
                    .map_err(|e| nu9ve_volumes::fs::io_error(e, &path))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            nu9ve_json::hex(&hasher.finalize())
        };
        drop(file);
        let rel = format!(".ncreator/catalog-backups/{name}");
        let no_hook = |_: &str, _: &Value| Ok(());
        let mut transfer = Transfer::new(home, catalog, backend, &no_hook);
        transfer.copy_many(
            &source,
            "catalog.sqlite3",
            &[Target {
                root: &target,
                rel: rel.clone(),
                record: &record,
            }],
            Some(&sha),
            None,
        )?;
        let receipt = json!({"schema": "nu9ve-catalog-backup/1", "path": rel, "sha256": sha, "created_at": nu9ve_time::now_utc(),
            "includes": ["all SQLite tables", "projects", "aliases", "profiles", "capture tags", "integration settings", "release journals"]});
        target.write_json(&format!("{rel}.json"), &receipt)?;
        catalog.set("last-metadata-backup", &receipt)?;
        Ok(receipt)
    })();
    let _ = std::fs::remove_dir_all(&temporary);
    result
}

/// A private temporary folder inside the state folder (Python `TemporaryDirectory(prefix=…, dir=catalog.home)`).
fn tempfile_in(home: &Path) -> Result<std::path::PathBuf> {
    let path = home.join(format!("catalog-backup-{}", nu9ve_json::uid()?));
    std::fs::create_dir(&path).map_err(|e| nu9ve_volumes::fs::io_error(e, &path))?;
    Ok(path)
}
