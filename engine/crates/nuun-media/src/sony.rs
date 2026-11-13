//! Observe camera-side cleanup using the original card identity and the verified removal journal.

use crate::catalog::{Catalog, WritableCatalog, sql, writer_lock};
use crate::layout::{MEDIA_KINDS, sony_index_member};
use crate::planner::destination_path;
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend, truthy};
use nu9ve_json::{Error, Result, require};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
}

type Members = BTreeMap<String, (Option<String>, i64)>;

fn members(catalog: &Catalog, id: &str) -> Result<Members> {
    let db = catalog
        .db()
        .ok_or_else(|| Error::new("unknown_job", "Job not found"))?;
    let mut stmt = db
        .prepare("SELECT relative_path,sha256,cleaned FROM members WHERE job_id=?")
        .map_err(sql)?;
    let rows = stmt
        .query_map([id], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))
        .map_err(sql)?;
    rows.collect::<rusqlite::Result<_>>().map_err(sql)
}

pub(crate) fn removal_recorded(catalog: &Catalog, job: &Value) -> Result<bool> {
    let plan = &job["plan"];
    if !matches!(
        text(&job["progress"], "phase"),
        "cleaning" | "checking_card"
    ) || plan["cleanup"] != true
    {
        return Ok(false);
    }
    let id = text(job, "id");
    let members = members(catalog, id)?;
    let copies = catalog.json_rows("SELECT payload FROM copies WHERE job_id=?", [id])?;
    let registered = catalog.volumes()?;
    let mut selected = Vec::new();
    for id in [&plan["archive_id"], &plan["work_id"]]
        .into_iter()
        .filter(|id| truthy(id))
    {
        let Some(volume) = registered.iter().find(|volume| &volume["id"] == id) else {
            return Ok(false);
        };
        selected.push(volume);
    }
    let Some(files) = plan["files"]
        .as_array()
        .filter(|files| !files.is_empty() && !selected.is_empty())
    else {
        return Ok(false);
    };
    for file in files {
        let path = text(file, "path");
        let Some((Some(sha), cleaned)) = members.get(path) else {
            return Ok(false);
        };
        if sha.is_empty() || *cleaned != i64::from(!sony_index_member(path)) {
            return Ok(false);
        }
        let unit = plan["units"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|unit| unit["id"] == file["unit_id"])
            .ok_or_else(|| Error::new("invalid_plan", "Missing file unit"))?;
        for volume in &selected {
            if destination_path(volume, plan, unit, path, true)?.is_some()
                && !copies.iter().any(|copy| {
                    copy["path"] == path
                        && copy["volume_id"] == volume["id"]
                        && copy["sha256"] == *sha
                })
            {
                return Ok(false);
            }
        }
    }
    Ok(members.values().any(|(_, cleaned)| *cleaned == 1))
}

pub fn reconcile(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    require(
        params.get("identity").is_some(),
        "operation_failed",
        "'identity'",
    )?;
    observe(
        home,
        backend,
        required(params, "source")?,
        params.get("identity"),
        None,
    )
}

fn observe(
    home: &Path,
    backend: &dyn Backend,
    source: &str,
    identity: Option<&Value>,
    only: Option<&str>,
) -> Result<Value> {
    let root =
        std::path::absolute(source).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    let info = backend.describe(&root, true)?;
    volumes::validate_card(&root, &info)?;
    if let Some(identity) = identity.filter(|id| !id.is_null()) {
        require(
            &volumes::card_identity(&info) == identity,
            "volume_replaced",
            "Card changed before checking it",
        )?;
    }
    let catalog = Catalog::open(home)?;
    volumes::reject_destination(&info, &volumes::destination_identities(&catalog, backend)?)?;
    let jobs = catalog.json_rows(
        "SELECT json_object('id',id,'state',state,'plan',json(plan)) FROM jobs
         WHERE (state='camera_action_required' OR (state='failed' AND error='FileNotFoundError'))
          AND json_extract(plan,'$.cleanup_profile')='sony_clips_then_format'
          AND json_extract(plan,'$.source_kind')='card'
          AND json_extract(plan,'$.source_identity.volume_uuid')=?
          AND (? IS NULL OR id=?)",
        (text(&info, "volume_uuid"), only, only),
    )?;
    let tree = Root::open(&root)?;
    let stamp = |path: &str| -> Result<Value> {
        Ok(tree.stat(path)?.map_or(Value::Null, |s| stamp_json(&s)))
    };
    let mut observations = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut checks = Vec::new();
    for job in jobs {
        let id = text(&job, "id");
        if job["state"] == "failed"
            && !removal_recorded(
                &catalog,
                &crate::reads::job(&catalog, &json!({"job_id": id}))?,
            )?
        {
            continue;
        }
        let plan = &job["plan"];
        if plan["cleanup"] != true
            || ["mode", "volume_uuid", "disk_uuid"]
                .iter()
                .any(|key| plan["source_identity"][*key] != info[*key])
        {
            continue;
        }
        let members = members(&catalog, id)?;
        let mut originals = Vec::new();
        let mut index_count = 0;
        let mut index_changed = false;
        for file in plan["files"].as_array().into_iter().flatten() {
            let path = text(file, "path");
            if !observations.contains_key(path) {
                observations.insert(path.to_owned(), stamp(path)?);
            }
            let current = &observations[path];
            let previous = &file["stamp"];
            if sony_index_member(path) {
                index_count += 1;
                index_changed |= current["size"].as_u64().is_some_and(|n| n > 0)
                    && (current["size"] != previous["size"]
                        || current["mtime_ns"] != previous["mtime_ns"]);
            } else {
                originals.push((path, previous));
            }
        }
        if index_count == 0
            || originals.is_empty()
            || originals
                .iter()
                .any(|(path, _)| members.get(*path).is_none_or(|(_, cleaned)| *cleaned != 1))
        {
            continue;
        }
        let mut reappeared = Vec::new();
        for (path, previous) in originals {
            let current = &observations[path];
            if current.is_null() || current["size"] != previous["size"] {
                continue;
            }
            if !hashes.contains_key(path) {
                let (sha, after) = crate::release::checked_hash(&tree, path)?
                    .ok_or_else(|| Error::new("source_changed", "Card changed during check"))?;
                require(
                    &after == current,
                    "source_changed",
                    "Card changed during check",
                )?;
                hashes.insert(path.to_owned(), sha);
            }
            if members[path].0.as_ref() == hashes.get(path) {
                reappeared.push(path.to_owned());
            }
        }
        reappeared.sort();
        let status = if !reappeared.is_empty() {
            "originals_present"
        } else if index_changed {
            "ready"
        } else {
            "waiting_camera"
        };
        checks.push((id.to_owned(), status, reappeared));
    }
    let mut updated = Vec::new();
    if !checks.is_empty() {
        let _lock = writer_lock(home)?;
        backend.fast_check(&root, &info)?;
        for (path, observed) in observations {
            require(
                stamp(&path)? == observed,
                "source_changed",
                "Card changed during check",
            )?;
        }
        let writer = WritableCatalog::open(home)?;
        for (id, status, reappeared) in &checks {
            if record_check(&writer, id, status, reappeared)? {
                updated.push(id);
            }
        }
    }
    let checked: Map<String, Value> = checks
        .iter()
        .map(|(id, status, _)| (id.clone(), json!(status)))
        .collect();
    let reappeared: Map<String, Value> = checks
        .iter()
        .filter(|(_, _, paths)| !paths.is_empty())
        .map(|(id, _, paths)| (id.clone(), json!(paths)))
        .collect();
    Ok(json!({"checked": checked, "updated": updated, "reappeared": reappeared}))
}

fn record_check(
    catalog: &WritableCatalog,
    id: &str,
    status: &str,
    reappeared: &[String],
) -> Result<bool> {
    let key = format!("sony-card-check:{id}");
    let paths = format!("sony-card-reappeared:{id}");
    let state = catalog.db().query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=? AND state IN ('camera_action_required','failed'))",
        [id], |row| row.get::<_, bool>(0),
    ).map_err(sql)?;
    if !state
        || (catalog.get(&key)? == Some(json!(status))
            && catalog.get(&paths)?.unwrap_or_else(|| json!([])) == json!(reappeared))
    {
        return Ok(false);
    }
    let tx = catalog.db().unchecked_transaction().map_err(sql)?;
    if status == "ready" {
        catalog
            .db()
            .execute(
                "UPDATE jobs SET state='completed',error=NULL,updated_at=? WHERE id=?",
                (nu9ve_time::now_utc(), id),
            )
            .map_err(sql)?;
        let progress = format!("progress:{id}");
        if let Some(mut value) = catalog.get(&progress)?.filter(truthy) {
            value["phase"] = json!("completed");
            catalog.set(&progress, &value)?;
        }
    }
    catalog.set(&key, &json!(status))?;
    catalog.set(&paths, &json!(reappeared))?;
    let mut payload = json!({"result": status});
    if !reappeared.is_empty() {
        payload["reappeared"] = json!(reappeared);
    }
    catalog.event(id, "sony_card_checked", &payload)?;
    tx.commit().map_err(sql)?;
    Ok(true)
}

pub fn confirm(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let id = required(params, "job_id")?;
    let source = required(params, "source")?;
    let job = crate::reads::job(&catalog, params)?;
    require(
        job["state"] == "camera_action_required",
        "invalid_state",
        "Job is not waiting for a camera-side cleanup",
    )?;
    if job["plan"]["cleanup_profile"] == "sony_clips_then_format" {
        let result = observe(home, backend, source, None, Some(id))?;
        let status = &result["checked"][id];
        require(
            !status.is_null(),
            "card_unconfirmed",
            "Choose the original Sony card with recorded removals",
        )?;
        require(
            status != "originals_present",
            "source_not_clean",
            "Backed-up clips are still on the card",
        )?;
        require(
            status == "ready",
            "card_not_formatted",
            "Card indexes are unchanged; format the card in the camera, then confirm",
        )?;
    } else {
        let _lock = writer_lock(home)?;
        let current = crate::reads::job(&catalog, params)?;
        require(
            current["state"] == "camera_action_required",
            "invalid_state",
            "Job is not waiting for a camera-side cleanup",
        )?;
        let root = std::path::absolute(source)
            .map_err(|e| Error::new("operation_failed", e.to_string()))?;
        let info = backend.describe(&root, true)?;
        require(
            info["mount"] == json!(root),
            "card_root_required",
            "Choose the root of the mounted card",
        )?;
        let scan = crate::scan::scan(&root, false, false, false)?;
        let remaining: Vec<_> = job["plan"]["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|file| {
                crate::snapshot::kind_of(text(file, "path"))
                    .is_some_and(|kind| MEDIA_KINDS.contains(&kind))
            })
            .filter(|file| {
                scan.files.iter().any(|current| {
                    current["path"] == file["path"]
                        && current["stamp"]["size"] == file["stamp"]["size"]
                        && current["stamp"]["mtime_ns"] == file["stamp"]["mtime_ns"]
                })
            })
            .take(5)
            .map(|file| text(file, "path"))
            .collect();
        require(
            remaining.is_empty(),
            "source_not_clean",
            format!(
                "Backed-up clips are still on the card: {}",
                remaining.join(", ")
            ),
        )?;
        backend.fast_check(&root, &info)?;
        let writer = WritableCatalog::open(home)?;
        let tx = writer.db().unchecked_transaction().map_err(sql)?;
        writer.event(id, "camera_cleanup_confirmed", &json!({"source": source, "volume_uuid": info["volume_uuid"], "files_now": scan.files.len()}))?;
        writer.state(id, "completed", None)?;
        tx.commit().map_err(sql)?;
    }
    crate::reads::job(&catalog, params)
}
