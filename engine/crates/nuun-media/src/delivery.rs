//! Deliver verified archive members to a registered editing disk.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::planner::destination_path;
use crate::restore::claim_directory;
use crate::transfer::{Meter, Target, Transfer};
use crate::tree::Root;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, NAMESPACE_URL, Result, canonical, require, uuid5};
use serde_json::{Value, json};
use std::path::Path;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

pub fn deliver(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let ids = params["asset_ids"]
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| Error::new("empty_selection", "Select assets to deliver"))?;
    let _lock = writer_lock(home)?;
    let reader = Catalog::open(home)?;
    let work = reader.volume(
        params["work"]
            .as_str()
            .ok_or_else(|| Error::new("operation_failed", "'work'"))?,
    )?;
    require(
        work["role"] == "work",
        "invalid_role",
        "Work volume required",
    )?;
    let catalog = WritableCatalog::open(home)?;
    let mut transfer = Transfer::new(home, &catalog, backend, &|_, _| Ok(()));
    let mut progress = DeliveryProgress {
        catalog: &catalog,
        key: params["progress_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(|id| format!("action-progress:{id}")),
        meter: Meter::default(),
        done: 0,
    };
    let mut delivered = Vec::new();
    let mut assets = Vec::new();
    for id in ids {
        let id = id
            .as_str()
            .ok_or_else(|| Error::new("unknown_asset", "Asset not found"))?;
        let asset = reader
            .json_rows("SELECT payload FROM assets WHERE id=?", [id])?
            .into_iter()
            .next()
            .ok_or_else(|| Error::new("unknown_asset", "Asset not found"))?;
        let job = crate::reads::job(&reader, &json!({"job_id": asset["job_id"]}))?;
        for file in job["plan"]["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|file| file["unit_id"] == id)
        {
            if destination_path(&work, &job["plan"], &asset, text(file, "path"), false)?.is_some() {
                progress.meter.files_total += 1;
                progress.meter.total += 2 * file["stamp"]["size"].as_u64().unwrap_or_default();
            }
        }
        assets.push((id, asset, job));
    }
    for (id, asset, job) in assets {
        let plan = &job["plan"];
        let job_id = text(plan, "id");
        let archive = reader.volume(text(plan, "archive_id"))?;
        let archive_info = volumes::check(backend, &archive)?;
        let work_info = volumes::check(backend, &work)?;
        require(
            volumes::independent(&archive_info, &work_info),
            "same_physical_disk",
            "Work and archive must be separate physical disks",
        )?;
        let files: Vec<_> = plan["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|file| file["unit_id"] == id)
            .collect();
        let needed = files
            .iter()
            .filter_map(|file| file["stamp"]["size"].as_u64())
            .sum::<u64>();
        require(
            work_info["free"].as_u64().unwrap_or_default()
                >= needed.saturating_add(64 * 1024 * 1024),
            "insufficient_space",
            "Insufficient work space",
        )?;
        let source = Root::open(Path::new(text(&archive, "root")))?;
        let target = Root::open(Path::new(text(&work, "root")))?;
        let overrides = reader
            .get(&format!("layout_override:{job_id}"))?
            .unwrap_or(Value::Null);
        let mut targets = Vec::new();
        for file in files {
            let path = text(file, "path");
            if let Some(mut destination) = destination_path(&work, plan, &asset, path, false)? {
                if let Some(rel) = overrides[path]["work"]
                    .as_str()
                    .filter(|path| !path.is_empty())
                {
                    destination =
                        format!("{}/{}", text(&work, "prefix"), crate::rules::relative(rel)?);
                }
                targets.push((file, destination));
            }
        }
        require(
            !targets.is_empty(),
            "empty_selection",
            "No work files to deliver",
        )?;
        let work_id = text(&work, "id");
        if plan.get("layout").is_none() {
            let (base, _) = targets[0]
                .1
                .split_once(&format!("/{job_id}/"))
                .ok_or_else(|| Error::new("invalid_plan", "Missing work job folder"))?;
            claim_directory(
                &target,
                &format!("{base}/{job_id}"),
                &json!({"job_id": job_id, "digest": plan["digest"], "volume_id": work_id}),
            )?;
        }
        let receipt_id = uuid5(&NAMESPACE_URL, &format!("{job_id}:{work_id}:{id}"));
        let mut copies = Vec::new();
        for (file, destination) in targets {
            let path = text(file, "path");
            let prior = reader
                .json_rows(
                    "SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?",
                    [job_id, text(&archive, "id"), path],
                )?
                .into_iter()
                .next()
                .ok_or_else(|| Error::new("missing_receipt", "Archive copy is not verified"))?;
            progress.meter.holding(
                &destination,
                text(&archive, "name"),
                text(&work, "name"),
                "copy",
            );
            progress.report()?;
            let [(sha, stamp)] = transfer
                .copy_many(
                    &source,
                    text(&prior, "destination"),
                    &[Target {
                        root: &target,
                        rel: destination.clone(),
                        record: &work,
                    }],
                    Some(text(&prior, "sha256")),
                    None,
                )?
                .try_into()
                .map_err(|_| Error::new("operation_failed", "Missing verified work copy"))?;
            let copy = json!({"job_id": job_id, "volume_id": work_id, "path": path, "destination": destination,
                "sha256": sha, "bytes": file["stamp"]["size"], "verified_at": nu9ve_time::now_utc(),
                "stamp": stamp, "receipt_id": receipt_id});
            catalog
                .db()
                .execute(
                    "INSERT OR REPLACE INTO copies VALUES(?,?,?,?,?)",
                    (
                        job_id,
                        work_id,
                        path,
                        sha,
                        String::from_utf8(canonical(&copy)).unwrap_or_default(),
                    ),
                )
                .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
            progress.done += 2 * file["stamp"]["size"].as_u64().unwrap_or_default();
            progress.meter.reach(progress.done);
            progress
                .meter
                .finished("copied", text(&copy, "destination"), text(&work, "name"));
            progress.report()?;
            copies.push(copy);
        }
        target.write_json(
            &format!(".ncreator/receipts/{job_id}-{id}.json"),
            &json!({"schema": "nu9ve-work-receipt/1", "receipt_id": receipt_id, "asset_id": id,
                "job_id": job_id, "files": copies, "verified_at": nu9ve_time::now_utc()}),
        )?;
        catalog
            .db()
            .execute(
                "UPDATE work SET state='ready',target=?,updated_at=? WHERE asset_id=?",
                (work_id, nu9ve_time::now_utc(), id),
            )
            .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
        catalog.event(
            job_id,
            "work_ready",
            &json!({"asset_id": id, "volume_id": work_id}),
        )?;
        delivered.push(id);
    }
    progress.meter.current.clear();
    progress.report()?;
    Ok(json!({"delivered": delivered, "volume_id": work["id"]}))
}

struct DeliveryProgress<'a> {
    catalog: &'a WritableCatalog,
    key: Option<String>,
    meter: Meter,
    done: u64,
}

impl DeliveryProgress<'_> {
    fn report(&self) -> Result<()> {
        if let Some(key) = &self.key {
            let mut fields = self.meter.fields();
            fields.insert("stage".into(), json!("delivering_work"));
            fields.insert("files_total".into(), json!(self.meter.files_total));
            fields.insert("files_done".into(), json!(self.meter.files_done));
            self.catalog.set(key, &Value::Object(fields))?;
        }
        Ok(())
    }
}

impl Drop for DeliveryProgress<'_> {
    fn drop(&mut self) {
        if let Some(key) = &self.key {
            let _ = self
                .catalog
                .db()
                .execute("DELETE FROM metadata WHERE key=?", [key]);
        }
    }
}
