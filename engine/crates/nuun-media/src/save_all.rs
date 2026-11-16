//! Review and run a set of existing ingest plans in order.
use crate::{
    catalog::Catalog,
    planner, transfer,
    volumes::{self, Backend},
};
use nu9ve_json::{Error, Result, digest, require};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
fn living_record(catalog: &Catalog, reference: &Value) -> Result<Value> {
    let id = text(reference, "id");
    require(
        crate::captures::python_uuid(id).as_deref() == Some(id),
        "invalid_plan",
        "Invalid folder backup ID",
    )?;
    crate::living::stored(
        catalog,
        &format!("living-plan:{id}"),
        &json!({"digest": reference["digest"]}),
    )
}

fn started(catalog: &Catalog, plan: &Value) -> Result<bool> {
    if plan["schema"] == "nuun-save-all-work/1" {
        return Ok(false);
    }
    if plan["schema"] == "nuun-save-all-living/1" {
        #[cfg(target_os = "macos")]
        return match living_record(catalog, plan) {
            Ok(record) => Ok(record["state"] == "completed"),
            Err(error) if error.code == "plan_changed" => Ok(false),
            Err(error) => Err(error),
        };
        #[cfg(not(target_os = "macos"))]
        return Err(Error::new("not_ported", "Folder backups require macOS"));
    }
    Ok(!catalog
        .json_rows(
            "SELECT json_object('id', id) FROM jobs WHERE id=?",
            [text(plan, "id")],
        )?
        .is_empty())
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

const DRAFT: &str = "save-all.json";
const MAX_DRAFT: usize = 14 * 1024 * 1024;

fn draft_lock(home: &Path) -> Result<crate::catalog::OperationLock> {
    crate::catalog::operation_lock(
        home,
        "save-all.lock",
        "job_busy",
        "Another batch is running",
    )
}

fn document(value: &Value) -> Result<Value> {
    let value = value
        .as_str()
        .ok_or_else(|| Error::new("invalid_plan", "Expected original plan document"))?;
    require(
        value.len() <= 12 * 1024 * 1024,
        "plan_too_large",
        "Plan exceeds 12 MiB",
    )?;
    serde_json::from_str(value).map_err(|e| Error::new("invalid_plan", e.to_string()))
}

fn check_reviewed(catalog: &Catalog, plan: &Value) -> Result<()> {
    let stored = catalog.get(&format!("reviewed-plan:{}", text(plan, "id")))?;
    require(
        stored.is_some_and(|v| v["digest"] == plan["digest"] && v["cancelled"] != true),
        "plan_changed",
        "Regenerate the source plan before adding it to the batch",
    )
}

pub fn draft(home: &Path) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let mut draft = crate::tree::read_bytes(home, DRAFT, MAX_DRAFT + 1)?
        .map(|bytes| serde_json::from_slice::<Value>(&bytes))
        .transpose()
        .map_err(|e| Error::new("invalid_plan", e.to_string()))?
        .unwrap_or_else(|| json!({"plans": {}, "excluded": []}));
    let Some(plans) = draft["plans"].as_object_mut() else {
        return Err(Error::new("invalid_plan", "Invalid saved batch"));
    };
    // An interrupted parent can leave an already-started child. Its canonical plan
    // and recovery state belong to Activity, so never offer it as another draft.
    for key in plans.keys().cloned().collect::<Vec<_>>() {
        let plan = document(&plans[&key])?;
        if started(&catalog, &plan)? {
            plans.remove(&key);
        }
    }
    Ok(draft)
}

pub fn save_draft(home: &Path, params: &Value) -> Result<Value> {
    let _batch = draft_lock(home)?;
    let catalog = Catalog::open(home)?;
    let mut draft = draft(home)?;
    if let Some(excluded) = params.get("excluded") {
        require(
            excluded.as_array().is_some_and(|rows| {
                rows.len() <= 1024
                    && rows
                        .iter()
                        .all(|v| v.as_str().is_some_and(|s| s.len() <= 4096))
            }),
            "invalid_request",
            "Invalid selection",
        )?;
        draft["excluded"] = excluded.clone();
    }
    if let Some(key) = params.get("key") {
        let key = key
            .as_str()
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .ok_or_else(|| Error::new("invalid_request", "Invalid draft item"))?;
        let plans = draft["plans"].as_object_mut().expect("checked by draft");
        if params["document"].is_null() {
            plans.remove(key);
        } else {
            let plan = document(&params["document"])?;
            if plan["schema"] == "nuun-save-all-living/1" {
                #[cfg(target_os = "macos")]
                living_record(&catalog, &plan)?;
                require(
                    !started(&catalog, &plan)?,
                    "plan_changed",
                    "This folder backup has already completed",
                )?;
            } else if plan["schema"] != "nuun-save-all-work/1" {
                planner::validate_plan(&plan)?;
                check_reviewed(&catalog, &plan)?;
            }
            plans.insert(key.to_owned(), params["document"].clone());
        }
        if params["include"] == true {
            draft["excluded"]
                .as_array_mut()
                .expect("validated selection")
                .retain(|v| v != key);
        }
    }
    store(home, &draft)?;
    Ok(draft)
}

fn store(home: &Path, draft: &Value) -> Result<()> {
    require(
        nu9ve_json::canonical(draft).len() <= MAX_DRAFT,
        "plan_too_large",
        "The batch is full; review fewer sources at once",
    )?;
    crate::tree::Root::open(home)?.write_json(DRAFT, draft)
}

pub fn plan_work(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let mut plan = crate::delivery::work_preview(&Catalog::open(home)?, backend, params)?;
    let entries = plan
        .as_object_mut()
        .expect("work preview object")
        .remove("entries")
        .unwrap_or_default();
    let entries = entries.as_array().map(Vec::as_slice).unwrap_or_default();
    plan["bytes"] = json!(entries.iter().fold(0u64, |sum, row| {
        sum.saturating_add(row["bytes"].as_u64().unwrap_or_default())
    }));
    plan["files"] = json!(entries.len());
    Ok(plan)
}

pub fn review(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let documents = params["plans"]
        .as_array()
        .ok_or_else(|| Error::new("invalid_plan", "Missing plans"))?;
    require(
        !documents.is_empty(),
        "invalid_plan",
        "Select at least one reviewed plan",
    )?;
    let catalog = Catalog::open(home)?;
    let cached = volumes::Cached::new(backend);
    let registered = catalog.volumes()?;
    let mut sources = Vec::<PathBuf>::new();
    let mut destinations = BTreeSet::new();
    let mut ids = Vec::new();
    let mut bytes = BTreeMap::<String, u64>::new();
    let mut shared = BTreeMap::<String, u64>::new();
    let mut release = BTreeMap::<String, u64>::new();
    #[allow(unused_mut)]
    let mut folder_targets = BTreeSet::<PathBuf>::new();
    for document in documents {
        let plan = self::document(document)?;
        if plan["schema"] == "nuun-save-all-work/1" {
            let checked = crate::delivery::work_preview(&catalog, &cached, &plan)?;
            require(
                plan["id"] == checked["id"] && plan["digest"] == checked["digest"],
                "plan_changed",
                "Review the editing copies again",
            )?;
            let id = text(&plan, "work");
            let record = catalog.volume(id)?;
            let current = volumes::check(&cached, &record)?;
            let root = Path::new(text(&record, "root"));
            let block = current["block_size"].as_u64().unwrap_or_default().max(4096);
            let volume = current["volume_uuid"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(id);
            let mut allocated = 0u64;
            for entry in checked["entries"].as_array().into_iter().flatten() {
                let path = root.join(text(entry, "path"));
                require(
                    destinations.insert(path.to_string_lossy().to_lowercase()),
                    "destination_conflict",
                    format!("Two plans would write {}", path.display()),
                )?;
                allocated = allocated.saturating_add(
                    entry["bytes"]
                        .as_u64()
                        .unwrap_or_default()
                        .div_ceil(block)
                        .saturating_mul(block),
                );
            }
            let amount = bytes.entry(id.to_owned()).or_default();
            *amount = amount.saturating_add(allocated);
            let amount = shared.entry(volume.to_owned()).or_default();
            *amount = amount.saturating_add(allocated);
            ids.push(json!({"id": checked["id"], "digest": checked["digest"], "source": record["name"], "kind": "work"}));
            continue;
        }
        #[cfg(target_os = "macos")]
        if plan["schema"] == "nuun-save-all-living/1" {
            let stored = living_record(&catalog, &plan)?;
            require(
                stored["state"] != "completed",
                "plan_changed",
                "This folder backup already completed",
            )?;
            let source = PathBuf::from(text(&stored, "source"));
            crate::living::validate_source(&cached, &source, &stored["source_identity"])?;
            let id = text(&stored, "archive_id");
            let record = catalog.volume(id)?;
            let current = volumes::check(&cached, &record)?;
            let target = Path::new(text(&record, "root")).join(text(&stored, "folder"));
            let target = PathBuf::from(target.to_string_lossy().to_lowercase());
            require(
                !folder_targets
                    .iter()
                    .any(|other| target.starts_with(other) || other.starts_with(&target)),
                "destination_conflict",
                "Folder backups overlap",
            )?;
            folder_targets.insert(target);
            let block = current["block_size"].as_u64().unwrap_or_default().max(4096);
            let allocated = ["new", "changed"]
                .into_iter()
                .flat_map(|key| stored[key].as_array().into_iter().flatten())
                .fold(0u64, |sum, row| {
                    sum.saturating_add(
                        row["stamp"]["size"]
                            .as_u64()
                            .unwrap_or_default()
                            .div_ceil(block)
                            .saturating_mul(block),
                    )
                });
            let volume = current["volume_uuid"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(id);
            let total = bytes.entry(id.to_owned()).or_default();
            *total = total.saturating_add(allocated);
            let total = shared.entry(volume.to_owned()).or_default();
            *total = total.saturating_add(allocated);
            sources.push(source);
            ids.push(json!({"id": stored["id"], "digest": stored["digest"], "source": stored["source"], "kind": "living"}));
            continue;
        }
        planner::preflight(&catalog, &cached, &plan, false)?;
        check_reviewed(&catalog, &plan)?;
        crate::missions::check_plan(&catalog, &plan)?;
        let source = PathBuf::from(text(&plan, "source"));
        let current = backend.describe(
            &source,
            plan["cleanup"] == true && plan["source_kind"] == "card",
        )?;
        require(
            plan["source_identity"].as_object().is_some_and(|fields| {
                fields
                    .iter()
                    .all(|(key, value)| current.get(key) == Some(value))
            }),
            "source_replaced",
            format!("Source identity changed: {}", source.display()),
        )?;
        require(
            !sources
                .iter()
                .any(|other| source.starts_with(other) || other.starts_with(&source)),
            "overlapping_sources",
            "Two plans use the same or overlapping sources",
        )?;
        sources.push(source);
        ids.push(json!({"id": plan["id"], "digest": plan["digest"], "source": plan["source"]}));

        let units: BTreeMap<_, _> = plan["units"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|unit| (text(unit, "id"), unit))
            .collect();
        for id in ["archive_id", "work_id"]
            .into_iter()
            .filter_map(|k| plan[k].as_str())
        {
            let record = catalog.volume(id)?;
            let current = volumes::check(&cached, &record)?;
            let root = Path::new(text(&record, "root"));
            let block = current["block_size"].as_u64().unwrap_or_default().max(4096);
            // Multiple registered folders can share a filesystem and its free-space budget.
            let pool = current["volume_uuid"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(id)
                .to_owned();
            let total = bytes.entry(id.to_owned()).or_default();
            let shared_total = shared.entry(pool).or_default();
            for file in plan["files"].as_array().into_iter().flatten() {
                let unit = units
                    .get(text(file, "unit_id"))
                    .copied()
                    .unwrap_or(&Value::Null);
                let Some(rel) =
                    planner::destination_path(&record, &plan, unit, text(file, "path"), true)?
                else {
                    continue;
                };
                let path = root.join(rel);
                // The macOS volumes in a batch can differ in case sensitivity. Treat case variants as a conflict.
                require(
                    destinations.insert(path.to_string_lossy().to_lowercase()),
                    "destination_conflict",
                    format!("Two plans would write {}", path.display()),
                )?;
                let size = file["stamp"]["size"].as_u64().unwrap_or_default();
                let allocated = size.div_ceil(block).saturating_mul(block);
                *total = total.saturating_add(allocated);
                *shared_total = shared_total.saturating_add(allocated);
            }
        }
        // Published finals have an explicitly registered work volume as source. Other release (notably
        // Stalker) is conditional and deliberately does not increase the capacity available to this batch.
        if plan["source_kind"] == "delivery"
            && plan["cleanup"] == true
            && let Some(record) = registered
                .iter()
                .filter(|record| {
                    record["role"] == "work"
                        && sources
                            .last()
                            .is_some_and(|s| s.starts_with(text(record, "root")))
                })
                .max_by_key(|record| text(record, "root").len())
        {
            let total = release.entry(text(record, "id").to_owned()).or_default();
            for file in plan["files"].as_array().into_iter().flatten() {
                *total = total.saturating_add(file["stamp"]["size"].as_u64().unwrap_or_default());
            }
        }
    }
    require(
        !destinations.iter().any(|p| {
            sources
                .iter()
                .any(|s| Path::new(p).starts_with(s.to_string_lossy().to_lowercase()))
        }),
        "overlapping_sources",
        "A destination is inside a source of the same batch",
    )?;
    for path in &destinations {
        require(
            !folder_targets
                .iter()
                .any(|folder| Path::new(path).starts_with(folder) || folder.starts_with(path)),
            "destination_conflict",
            "An ingest destination overlaps a folder backup",
        )?;
        for parent in Path::new(path).ancestors().skip(1) {
            require(
                !destinations.contains(parent.to_string_lossy().as_ref()),
                "destination_conflict",
                format!(
                    "File destination is also a parent folder: {}",
                    parent.display()
                ),
            )?;
        }
    }
    for (index, source) in sources.iter().enumerate() {
        require(
            !sources[..index]
                .iter()
                .any(|other| source.starts_with(other) || other.starts_with(source)),
            "overlapping_sources",
            "Two plans use the same or overlapping sources",
        )?;
        require(
            !folder_targets
                .iter()
                .any(|folder| folder.starts_with(source.to_string_lossy().to_lowercase())),
            "overlapping_sources",
            "A folder backup destination is inside a batch source",
        )?;
    }

    let mut effects = Vec::new();
    for id in bytes.keys().chain(release.keys()).collect::<BTreeSet<_>>() {
        let copy_bytes = bytes.get(id).copied().unwrap_or_default();
        let release_bytes = release.get(id).copied().unwrap_or_default();
        let record = catalog.volume(id)?;
        let current = volumes::check(&cached, &record)?;
        let free = current["free"].as_u64();
        let volume = current["volume_uuid"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(id);
        let needed = shared.get(volume).copied().unwrap_or_default();
        effects.push(json!({"volume_id": id, "name": record["name"], "free": free,
                            "copy_bytes": copy_bytes, "release_bytes": release_bytes,
                            "estimated_free": free.map(|n| n.saturating_sub(needed).saturating_add(release_bytes)),
                            "fits": needed == 0 || free.is_some_and(|n| n >= needed.saturating_add((64 * 1024 * 1024).max(needed / 20)))}));
    }
    Ok(
        json!({"digest": digest(&json!({"schema":"nuun-save-all/1","plans": ids})),
              "plans": ids, "volumes": effects}),
    )
}

pub fn apply(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    #[cfg(target_os = "macos")]
    let _cancel = crate::interrupt::Cancel::install()?;
    let _batch = draft_lock(home)?;
    let reviewed = review(home, backend, params)?;
    require(
        reviewed["digest"] == params["digest"],
        "plan_changed",
        "Review the selected batch again",
    )?;
    require(
        reviewed["volumes"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|v| v["fits"] == true),
        "insufficient_space",
        "The batch does not fit on the current destinations",
    )?;

    let mut results = Vec::new();
    for (document, item) in params["plans"]
        .as_array()
        .into_iter()
        .flatten()
        .zip(reviewed["plans"].as_array().into_iter().flatten())
    {
        #[cfg(target_os = "macos")]
        if crate::interrupt::cancelled() {
            break;
        }
        let outcome = match item["kind"].as_str() {
            Some("work") => self::document(document).and_then(|plan| {
                crate::delivery::deliver_reviewed(home, backend, &plan)?;
                Ok(json!({"state":"completed"}))
            }),
            #[cfg(target_os = "macos")]
            Some("living") => crate::living::apply_cancellable(
                home,
                backend,
                &json!({"plan_id":item["id"], "digest":item["digest"]}),
            ),
            _ => transfer::apply_cancellable(home, backend, document, text(item, "digest")),
        };
        match outcome {
            Ok(result) => {
                let state = text(&result, "state");
                results.push(json!({"id": item["id"], "source": item["source"], "state": state,
                    "metadata_backup_error": result["metadata_backup_error"], "stalker_release": result["stalker_release"],
                    "changed_since_review": result["result"]["changed_since_review"]}));
                if !["completed", "source_retained", "camera_action_required"].contains(&state) {
                    break;
                }
            }
            Err(error) => {
                results.push(json!({"id": item["id"], "source": item["source"], "error": {"code": error.code, "message": error.message}}));
                break;
            }
        }
    }
    let result =
        json!({"results": results, "total": reviewed["plans"].as_array().map_or(0, Vec::len)});
    // A single compact result supplements Activity. The per-job plans and receipts
    // needed for recovery remain owned by transfer.
    let mut draft = draft(home)?;
    if let Some(plans) = draft["plans"].as_object_mut() {
        plans.retain(|_, value| {
            !self::document(value).is_ok_and(|plan| {
                results
                    .iter()
                    .any(|row| row["id"] == plan["id"] && row["state"] == "completed")
            })
        });
    }
    draft["last_result"] = json!({
        "total": result["total"],
        "attempted": results.len(),
        "results": results.iter().take(64).map(|row| json!({
            "id": row["id"], "source": text(row, "source").chars().take(512).collect::<String>(),
            "state": row["state"],
            "changed_since_review": row["changed_since_review"],
            "error": row["error"].get("message").and_then(Value::as_str).map(|message|
                json!({"message": message.chars().take(240).collect::<String>()})),
        })).collect::<Vec<_>>()
    });
    if let Err(error) = store(home, &draft) {
        let mut result = result;
        result["draft_warning"] = json!(error.message);
        return Ok(result);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::test_backend::setup;
    use nu9ve_json::canonical;

    #[test]
    fn batch_rejects_conflicts_and_unreviewed_order_then_preserves_each_source_policy() {
        let s = setup();
        let second = s.card.with_file_name("card2");
        std::fs::create_dir(&second).unwrap();
        for source in [&s.card, &second] {
            std::fs::write(source.join("clip.mp4"), b"media").unwrap();
        }
        let make = |source: &Path, project: &str, cleanup| {
            let plan = planner::build_plan(&s.home, &s.backend, &json!({
                "source": source, "archive": "MEDIA", "work": "ESSD", "project": project, "cleanup": cleanup
            })).unwrap();
            String::from_utf8(canonical(&plan)).unwrap()
        };
        let first = make(&s.card, "first", false);
        let other = make(&second, "second", true);
        let request = json!({"plans":[first, other]});
        assert_eq!(
            review(&s.home, &s.backend, &json!({"plans":[first, first]}))
                .unwrap_err()
                .code,
            "overlapping_sources"
        );

        let reviewed = review(&s.home, &s.backend, &request).unwrap();
        assert_eq!(reviewed["volumes"].as_array().unwrap().len(), 2);
        let mut low_space = crate::volumes::test_backend::Fixture::default();
        low_space.overrides.insert(
            s.archive.to_string_lossy().into_owned(),
            json!({"free": 64 * 1024 * 1024 + 4096}),
        );
        let low_review = review(&s.home, &low_space, &request).unwrap();
        assert!(
            low_review["volumes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["fits"] == false)
        );
        assert_eq!(
            apply(
                &s.home,
                &low_space,
                &json!({"plans":[first, other], "digest":reviewed["digest"]})
            )
            .unwrap_err()
            .code,
            "insufficient_space"
        );
        assert_eq!(
            apply(
                &s.home,
                &s.backend,
                &json!({"plans":[other, first], "digest":reviewed["digest"]})
            )
            .unwrap_err()
            .code,
            "plan_changed"
        );
        assert!(second.join("clip.mp4").is_file());

        let mut request = request;
        request["digest"] = reviewed["digest"].clone();
        let result = apply(&s.home, &s.backend, &request).unwrap();
        assert_eq!(result["results"][0]["state"], "source_retained");
        assert_eq!(result["results"][1]["state"], "completed");
        assert!(s.card.join("clip.mp4").is_file());
        assert!(!second.join("clip.mp4").exists());
    }

    #[test]
    fn draft_recovers_pending_source_after_one_job_starts_and_compacts_completed_batch() {
        let s = setup();
        let second = s.card.with_file_name("card2");
        std::fs::create_dir(&second).unwrap();
        let make = |source: &Path, project: &str| {
            std::fs::write(source.join("clip.mp4"), b"media").unwrap();
            let plan = planner::build_plan(
                &s.home,
                &s.backend,
                &json!({
                    "source": source, "archive": "MEDIA", "work": "ESSD", "project": project
                }),
            )
            .unwrap();
            (String::from_utf8(canonical(&plan)).unwrap(), plan)
        };
        let (first, first_plan) = make(&s.card, "first");
        let (other, _) = make(&second, "second");
        save_draft(&s.home, &json!({"key":"first","document":first})).unwrap();
        save_draft(
            &s.home,
            &json!({"key":"second","document":other,"excluded":["ignore"]}),
        )
        .unwrap();
        assert_eq!(draft(&s.home).unwrap()["plans"]["second"], other);
        transfer::apply(
            &s.home,
            &s.backend,
            &json!(first),
            text(&first_plan, "digest"),
            &|_, _| Ok(()),
        )
        .unwrap();
        let restored = draft(&s.home).unwrap();
        assert_eq!(restored["plans"]["first"], Value::Null);
        assert_eq!(restored["excluded"], json!(["ignore"]));
        let params = json!({"plans":[other]});
        let review = review(&s.home, &s.backend, &params).unwrap();
        apply(
            &s.home,
            &s.backend,
            &json!({"plans":[other],"digest":review["digest"]}),
        )
        .unwrap();
        let compact = draft(&s.home).unwrap();
        assert_eq!(compact["plans"], json!({}));
        assert_eq!(compact["last_result"]["attempted"], 1);
        assert!(std::fs::metadata(s.home.join(DRAFT)).unwrap().len() < 4096);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn folder_backup_uses_saved_owner_plan_and_keeps_source() {
        let s = setup();
        std::fs::write(s.card.join("track.wav"), b"audio").unwrap();
        let plan = crate::living::plan(
            &s.home,
            &s.backend,
            &json!({
                "source":s.card,"archive":"MEDIA","folder":"Music/pilot","batch_managed":true
            }),
        )
        .unwrap();
        let document =
            json!({"schema":"nuun-save-all-living/1", "id":plan["id"],"digest":plan["digest"]})
                .to_string();
        save_draft(&s.home, &json!({"key":"folder","document":document})).unwrap();
        let review = review(&s.home, &s.backend, &json!({"plans":[document]})).unwrap();
        assert_eq!(review["volumes"][0]["copy_bytes"], 4096);
        assert_eq!(
            apply(
                &s.home,
                &s.backend,
                &json!({"plans":[document],"digest":review["digest"]})
            )
            .unwrap()["results"][0]["state"],
            "completed"
        );
        assert!(s.card.join("track.wav").is_file());
        assert_eq!(
            std::fs::read(s.archive.join("Music/pilot/track.wav")).unwrap(),
            b"audio"
        );
        assert_eq!(draft(&s.home).unwrap()["plans"], json!({}));
        assert_eq!(
            Catalog::open(&s.home)
                .unwrap()
                .get(&format!("living-plan:{}", text(&plan, "id")))
                .unwrap()
                .unwrap()["new"],
            Value::Null
        );
    }

    #[test]
    fn reviewed_editing_copy_reuses_archive_receipts_and_preserves_sources() {
        let s = setup();
        std::fs::write(s.card.join("clip.mp4"), b"media").unwrap();
        let ingest = planner::build_plan(
            &s.home,
            &s.backend,
            &json!({
                "source":s.card,"archive":"MEDIA","work":"ESSD","project":"editor",
                "archive_only":true,"confirm_archive_only":true,"work_decision":"pending"
            }),
        )
        .unwrap();
        transfer::apply(
            &s.home,
            &s.backend,
            &ingest,
            text(&ingest, "digest"),
            &|_, _| Ok(()),
        )
        .unwrap();
        let asset = &ingest["units"][0]["id"];
        let work = plan_work(
            &s.home,
            &s.backend,
            &json!({"asset_ids":[asset],"work":"ESSD"}),
        )
        .unwrap();
        assert_eq!(work["bytes"], 5);
        let doc = work.to_string();
        save_draft(&s.home, &json!({"key":"work","document":doc})).unwrap();
        let review = review(&s.home, &s.backend, &json!({"plans":[doc]})).unwrap();
        assert_eq!(
            apply(
                &s.home,
                &s.backend,
                &json!({"plans":[doc],"digest":review["digest"]})
            )
            .unwrap()["results"][0]["state"],
            "completed"
        );
        assert!(s.card.join("clip.mp4").is_file());
        assert_eq!(draft(&s.home).unwrap()["plans"], json!({}));
    }
}
