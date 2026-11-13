//! Restore a cataloged asset from its verified archive copies.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::transfer::{Target, Transfer};
use crate::tree::Root;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, NAMESPACE_URL, Result, canonical, require, uuid5};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

pub fn restore(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let required = |key| {
        params
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
    };
    let asset_id = required("asset_id")?;
    let _lock = writer_lock(home)?;
    let reader = Catalog::open(home)?;
    let asset = reader
        .json_rows("SELECT payload FROM assets WHERE id=?", [asset_id])?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("unknown_asset", "Asset not found"))?;
    let job = crate::reads::job(&reader, &json!({"job_id": asset["job_id"]}))?;
    let plan = &job["plan"];
    let archive = reader.volume(plan["archive_id"].as_str().unwrap_or_default())?;
    volumes::check(backend, &archive)?;
    let root = crate::planner::absolute_source(required("destination")?)?;
    let info = backend.describe(&root, false)?;
    let source_root = Path::new(archive["root"].as_str().unwrap_or_default());
    require(
        !root.starts_with(source_root) && !source_root.starts_with(&root),
        "overlapping_roots",
        "Restore outside the archive volume root",
    )?;
    let record = json!({"id": uuid5(&NAMESPACE_URL, &root.to_string_lossy()), "name": "restore",
        "role": "work", "root": root, "identity": info});
    let files: Vec<_> = plan["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|file| file["unit_id"] == asset_id)
        .collect();
    let needed = files
        .iter()
        .filter_map(|file| file["stamp"]["size"].as_u64())
        .sum::<u64>();
    require(
        info["free"].as_u64().unwrap_or_default() >= needed.saturating_add(64 * 1024 * 1024),
        "insufficient_space",
        "Insufficient restore space",
    )?;
    let base = format!(
        "{}/{asset_id}",
        asset["project"].as_str().unwrap_or_default()
    );
    crate::rules::relative(&base)?;
    let copies = reader.json_rows(
        "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
        [
            plan["id"].as_str().unwrap_or_default(),
            archive["id"].as_str().unwrap_or_default(),
        ],
    )?;
    let by_path: BTreeMap<_, _> = copies
        .iter()
        .filter_map(|copy| copy["path"].as_str().map(|path| (path, copy)))
        .collect();
    let source = Root::open(source_root)?;
    let target = Root::open(&root)?;
    let catalog = WritableCatalog::open(home)?;
    let mut transfer = Transfer::new(home, &catalog, backend, &|_, _| Ok(()));
    claim_directory(
        &target,
        &base,
        &json!({"asset_id": asset_id, "revision": asset["revision"]}),
    )?;
    let source_name = crate::rules::relative(plan["source_name"].as_str().unwrap_or_default())?;
    for file in files {
        let path = file["path"].as_str().unwrap_or_default();
        let copy = by_path
            .get(path)
            .ok_or_else(|| Error::new("missing_receipt", "Archive copy missing"))?;
        let sha = copy["sha256"]
            .as_str()
            .ok_or_else(|| Error::new("missing_receipt", "Archive hash missing"))?;
        transfer.copy_many(
            &source,
            copy["destination"].as_str().unwrap_or_default(),
            &[Target {
                root: &target,
                rel: format!("{base}/{source_name}/{path}"),
                record: &record,
            }],
            Some(sha),
            None,
        )?;
    }
    target.write_json(&format!("{base}/.ncreator-restore.json"),
        &json!({"asset_id": asset_id, "revision": asset["revision"], "verified_at": nu9ve_time::now_utc()}))?;
    catalog.set(
        &format!("restore:{asset_id}"),
        &json!({"root": root, "path": base, "verified_at": nu9ve_time::now_utc()}),
    )?;
    Ok(json!({"asset_id": asset_id, "path": root.join(base).join(source_name), "verified": true}))
}

pub(crate) fn claim_directory(tree: &Root, rel: &str, owner: &Value) -> Result<()> {
    let marker = format!("{rel}/.ncreator-owner.json");
    let (directory, _) = tree.parent(&marker, true)?;
    let mut entries =
        std::fs::read_dir(&directory).map_err(|e| nu9ve_volumes::fs::io_error(e, &directory))?;
    if entries
        .next()
        .transpose()
        .map_err(|e| nu9ve_volumes::fs::io_error(e, &directory))?
        .is_none()
    {
        return tree.write_bytes(&marker, &canonical(owner), false);
    }
    let file = tree
        .open_read(&marker)?
        .ok_or_else(|| Error::new("operation_failed", "Destination has no owner marker"))?;
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|e| nu9ve_volumes::fs::io_error(e, &directory))?;
    let actual: Value = serde_json::from_slice(&bytes)
        .map_err(|e| Error::new("operation_failed", e.to_string()))?;
    require(
        &actual == owner,
        "destination_conflict",
        "Destination is owned by another operation",
    )
}
