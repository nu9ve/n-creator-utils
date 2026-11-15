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

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
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
    for document in documents {
        let Some(document) = document.as_str() else {
            return Err(Error::new(
                "invalid_plan",
                "Expected original plan document",
            ));
        };
        require(
            document.len() <= 12 * 1024 * 1024,
            "plan_too_large",
            "Plan exceeds 12 MiB",
        )?;
        let plan: Value = serde_json::from_str(document)
            .map_err(|e| Error::new("invalid_plan", e.to_string()))?;
        planner::preflight(&catalog, &cached, &plan, false)?;
        let stored = catalog.get(&format!("reviewed-plan:{}", text(&plan, "id")))?;
        require(
            stored.is_some_and(|value| {
                value["digest"] == plan["digest"] && value["cancelled"] != true
            }),
            "plan_changed",
            "Regenerate the source plan before adding it to the batch",
        )?;
        if let Some(missions) = plan["missions"].as_object().filter(|m| !m.is_empty()) {
            let archive = catalog.volume(text(&plan, "archive_id"))?;
            for (id, planned) in missions {
                let flow = crate::mission_review::flow(&catalog, &json!(id))?;
                let current = crate::missions::index_for(&archive, &flow)?;
                require(
                    planned["history_sha256"] == json!(current.history_sha256),
                    "plan_changed",
                    "The mission registry changed; plan this source again",
                )?;
            }
        }
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
    let _batch = crate::catalog::operation_lock(
        home,
        "save-all.lock",
        "job_busy",
        "Another batch is running",
    )?;
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
        match transfer::apply_cancellable(home, backend, document, text(item, "digest")) {
            Ok(result) => {
                let state = text(&result, "state");
                results.push(json!({"id": item["id"], "source": item["source"], "state": state,
                    "metadata_backup_error": result["metadata_backup_error"], "stalker_release": result["stalker_release"]}));
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
    Ok(json!({"results": results, "total": reviewed["plans"].as_array().map_or(0, Vec::len)}))
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
}
