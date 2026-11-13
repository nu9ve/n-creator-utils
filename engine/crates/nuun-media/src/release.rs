//! Reviewed release plans. Applying a plan is a separate operation.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::coverage::walk;
use crate::tree::{Root, stamp_json};
use crate::volumes::truthy;
use crate::volumes::{self, Backend};
use crate::{clipper, rules::relative};
use nu9ve_json::{Error, Result, digest, require};
use nu9ve_volumes::fs;
use rusqlite::{Connection, OptionalExtension, Params};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn required<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
}

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn parse(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|e| Error::new("operation_failed", e.to_string()))
}

fn payloads(db: &Connection, query: &str, params: impl Params) -> Result<Vec<Value>> {
    let mut statement = db.prepare(query).map_err(sql)?;
    let rows = statement
        .query_map(params, |row| row.get::<_, String>(0))
        .map_err(sql)?;
    rows.map(|row| parse(&row.map_err(sql)?)).collect()
}

fn protected(path: &str) -> bool {
    path.split('/').map(str::to_lowercase).any(|part| {
        matches!(part.as_str(), "clipper" | "renders" | ".ncreator")
            || part.ends_with(".fcpbundle")
            || part.ends_with(".fcpcache")
    })
}

fn preserve(path: &str, local: bool) -> bool {
    let in_bundle = path
        .rsplit_once('/')
        .is_some_and(|(parent, _)| bundle_root(parent).is_some());
    let media = crate::snapshot::kind_of(path)
        .is_some_and(|kind| crate::layout::MEDIA_KINDS.contains(&kind));
    protected(path) || (!local && !in_bundle && !media)
}

pub(crate) fn selected_files(
    tree: &Root,
    scope: &str,
    local: bool,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut paths = Vec::new();
    let mut preserved = Vec::new();
    walk(tree, scope, |path, size| {
        require(
            size.is_some(),
            "unsafe_file",
            format!("Links/special files prevent release: {path}"),
        )?;
        if preserve(&path, local) {
            preserved.push(path);
        } else {
            paths.push(path);
        }
        Ok(())
    })?;
    Ok((paths, preserved))
}

pub(crate) fn raw_closeout_plan(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
) -> Result<Value> {
    build_review(home, backend, params, |_, _| Ok(Value::Null))
}

fn bundle_root(path: &str) -> Option<&str> {
    path.split('/')
        .scan(0, |end, part| {
            *end += part.len() + 1;
            Some((part, *end - 1))
        })
        .find(|(part, _)| part.to_lowercase().ends_with(".stalker"))
        .map(|(_, end)| &path[..end])
}

fn manifest(bundle: &Path) -> Result<Value> {
    let data = crate::tree::read_bytes(bundle, "manifest.json", 1024 * 1024)
        .map_err(|error| {
            if error.code == "file_too_large" {
                Error::new("document_too_large", "JSON input exceeds size limit")
            } else {
                error
            }
        })?
        .ok_or_else(|| Error::new("operation_failed", "Manifest not found"))?;
    serde_json::from_slice(&data).map_err(|_| Error::new("invalid_json", "Invalid JSON document"))
}

pub(crate) fn checked_hash(tree: &Root, path: &str) -> Result<Option<(String, Value)>> {
    let Some(before) = tree.stat(path)? else {
        return Ok(None);
    };
    let Some(file) = tree.open_read(path)? else {
        return Ok(None);
    };
    let sha = crate::transfer::hash_file(&mut fs::verify_read(file), |_| {})?;
    require(
        tree.stat(path)?.as_ref() == Some(&before),
        "source_changed",
        format!("File changed while hashing: {path}"),
    )?;
    Ok(Some((sha, stamp_json(&before))))
}

pub(crate) fn consumers(
    home: &Path,
    catalog: &Catalog,
    replacements: &[(String, Option<String>)],
) -> Result<Value> {
    let integrations = crate::integrations::Integrations::read(catalog)?;
    if integrations.clipper_executable().is_ok() {
        // Python records discovery even when the installed consumer fails. A later disappearance must
        // never be treated as if Clipper had never been configured.
        WritableCatalog::open(home)?.set("clipper-known", &json!(true))?;
    }
    Ok(clipper::inspect(catalog, replacements).unwrap_or_else(
        |error| json!({"blockers": [error.message], "relinks": [], "dependencies": null}),
    ))
}

fn verify_bundle(original: &Path, replacement: &Path) -> Result<()> {
    let left = manifest(original)?;
    let right = manifest(replacement)?;
    require(
        left["producer"] == "stalker"
            && left["status"] == "completed"
            && digest(&left) == digest(&right),
        "revision_mismatch",
        "Stalker replacement must have the same completed manifest",
    )?;
    let members = |root| -> Result<BTreeSet<String>> {
        Ok(crate::scan::scan(root, false, false, false)?
            .files
            .iter()
            .map(|v| text(v, "path").to_owned())
            .collect())
    };
    require(
        members(original)? == members(replacement)?,
        "revision_mismatch",
        "Stalker bundle membership differs",
    )
}

fn prefer_work(
    catalog: &Catalog,
    backend: &dyn Backend,
    archive: &Value,
    copies: &[Value],
    entries: &[Value],
    replacements: &mut [(String, Option<String>)],
) -> Result<Vec<Value>> {
    let db = catalog
        .db()
        .expect("archive registration requires a database");
    let archive_copies: HashMap<_, _> = copies
        .iter()
        .map(|copy| (text(copy, "destination"), copy))
        .collect();
    let volumes = catalog.volumes()?;
    let mut checks = Vec::new();
    for (original, replacement) in replacements {
        let Some(fallback) = replacement.as_deref() else {
            continue;
        };
        if !original.to_lowercase().ends_with(".stalker") {
            continue;
        }
        let related: Vec<_> = entries
            .iter()
            .filter(|entry| {
                let path = Path::new(text(archive, "root")).join(text(entry, "archive_path"));
                path != Path::new(fallback) && path.starts_with(fallback)
            })
            .collect();
        if related.is_empty() {
            continue;
        }
        for volume in &volumes {
            if volume["role"] != "work" || volumes::online(backend, volume).is_none() {
                continue;
            }
            let candidate = (|| -> Result<(String, Vec<Value>)> {
                let tree = Root::open(Path::new(text(volume, "root")))?;
                let mut bundle = None;
                let mut verified = Vec::new();
                for entry in &related {
                    let receipt = archive_copies
                        .get(text(entry, "archive_path"))
                        .ok_or_else(|| Error::new("missing_receipt", "Archive receipt missing"))?;
                    let raw: Option<String> = db.query_row(
                        "SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?",
                        [text(receipt, "job_id"), text(volume, "id"), text(receipt, "path")], |row| row.get(0)
                    ).optional().map_err(sql)?;
                    let copy =
                        parse(&raw.ok_or_else(|| {
                            Error::new("missing_receipt", "Work receipt missing")
                        })?)?;
                    let path = text(&copy, "destination");
                    let root = bundle_root(path)
                        .ok_or_else(|| Error::new("revision_mismatch", "Work bundle mismatch"))?;
                    require(
                        bundle.as_deref().is_none_or(|b| b == root),
                        "revision_mismatch",
                        "Work bundle mismatch",
                    )?;
                    require(
                        checked_hash(&tree, path)?.is_some_and(|(sha, _)| entry["sha256"] == sha),
                        "checksum_mismatch",
                        "Work changed",
                    )?;
                    bundle = Some(root.to_owned());
                    verified.push(
                        json!({"volume_id": volume["id"], "path": path, "sha256": entry["sha256"]}),
                    );
                }
                let root = tree
                    .path
                    .join(bundle.unwrap_or_default())
                    .to_string_lossy()
                    .into_owned();
                require(
                    !clipper::overlaps(original, &root),
                    "overlapping_roots",
                    "Replacement overlaps released source",
                )?;
                Ok((root, verified))
            })();
            if let Ok((candidate, verified)) = candidate {
                *replacement = Some(candidate);
                checks.extend(verified);
                break;
            }
        }
    }
    Ok(checks)
}

pub fn plan_release(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    plan_with(home, backend, params, |catalog, paths| {
        consumers(home, catalog, paths)
    })
}

fn plan_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    inspect: impl FnOnce(&Catalog, &[(String, Option<String>)]) -> Result<Value>,
) -> Result<Value> {
    let plan = build_review(home, backend, params, inspect)?;
    let _lock = writer_lock(home)?;
    WritableCatalog::open(home)?.set(&format!("release:{}", text(&plan, "id")), &plan)?;
    Ok(plan)
}

fn build_review(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    inspect: impl FnOnce(&Catalog, &[(String, Option<String>)]) -> Result<Value>,
) -> Result<Value> {
    if let Some(key) = params.as_object().and_then(|map| {
        map.keys().find(|key| {
            ![
                "archive",
                "work",
                "path",
                "source",
                "project",
                "auto_ingest_job_id",
            ]
            .contains(&key.as_str())
        })
    }) {
        require(
            key != "progress",
            "invalid_request",
            "Progress callbacks are internal to the local service",
        )?;
        return Err(Error::new(
            "operation_failed",
            format!("plan_release() got an unexpected keyword argument '{key}'"),
        ));
    }
    let catalog = Catalog::open(home)?;
    let archive = catalog.volume(required(params, "archive")?)?;
    require(
        archive["role"] == "archive",
        "invalid_role",
        "Archive required",
    )?;
    let archive_info = volumes::check(backend, &archive)?;
    let local = truthy(&params["source"]);
    require(
        local != truthy(&params["work"]),
        "invalid_release",
        "Choose one Stalker original or a work folder",
    )?;
    let project = text(params, "project");
    require(
        !project.trim().is_empty(),
        "invalid_release",
        "Name the project being released",
    )?;
    let db = catalog
        .db()
        .expect("archive registration requires a database");
    let mut session = Value::Null;
    let mut manifest_digest = Value::Null;
    let mut assets = Vec::new();
    let (source_root, scope, work_id) = if local {
        let root = crate::planner::absolute_source(required(params, "source")?)?;
        require(
            root.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("stalker")),
            "invalid_release",
            "Local release only supports one completed Stalker bundle",
        )?;
        let manifest = manifest(&root)?;
        require(
            manifest["status"] == "completed" && manifest["producer"] == "stalker",
            "active_capture",
            "Only completed Stalker captures may be released",
        )?;
        let id = crate::captures::python_uuid(required(&manifest, "sessionID")?)
            .ok_or_else(|| Error::new("operation_failed", "Invalid session UUID"))?;
        let sha = digest(&manifest);
        assets = payloads(
            db,
            "SELECT assets.payload FROM assets JOIN sessions ON sessions.asset_id=assets.id WHERE lower(sessions.session_id)=? AND sessions.manifest_digest=?",
            [&id, &sha],
        )?;
        require(
            !assets.is_empty(),
            "missing_receipt",
            "Back up this exact Stalker revision before releasing it",
        )?;
        session = json!(id);
        manifest_digest = json!(sha);
        (
            root.parent().unwrap_or(&root).to_path_buf(),
            root.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            Value::Null,
        )
    } else {
        let work = catalog.volume(required(params, "work")?)?;
        require(work["role"] == "work", "invalid_role", "Work disk required")?;
        volumes::check(backend, &work)?;
        let scope = relative(text(params, "path"))?;
        require(
            scope.starts_with(&format!("{}/", text(&work, "prefix"))) && !protected(scope),
            "invalid_release",
            "Choose a raw project folder inside the work prefix",
        )?;
        (
            Path::new(text(&work, "root")).to_path_buf(),
            scope.to_owned(),
            work["id"].clone(),
        )
    };
    let source_info = backend.describe(&source_root, false)?;
    let archive_root = Path::new(text(&archive, "root"));
    require(
        volumes::independent(&source_info, &archive_info),
        "same_physical_disk",
        "Archive and released source must be independent disks",
    )?;
    require(
        !source_root.starts_with(archive_root) && !archive_root.starts_with(&source_root),
        "overlapping_roots",
        "Source overlaps archive",
    )?;
    let origin = Root::open(&source_root)?;
    let target = Root::open(archive_root)?;
    let (paths, preserved) = selected_files(&origin, &scope, local)?;
    require(
        !paths.is_empty(),
        "empty_selection",
        "No editable media to release in this folder",
    )?;
    let copies = payloads(
        db,
        "SELECT payload FROM copies WHERE volume_id=?",
        [text(&archive, "id")],
    )?;
    let mut receipts: HashMap<&str, Vec<&str>> = HashMap::new();
    for copy in &copies {
        receipts
            .entry(text(copy, "sha256"))
            .or_default()
            .push(text(copy, "destination"));
    }
    let mut legacy: Option<HashMap<u64, Vec<String>>> = None;
    let mut cache = HashMap::new();
    let mut entries = Vec::new();
    let mut replacements: Vec<(String, Option<String>)> = Vec::new();
    let mut bytes = 0;
    for path in paths {
        let (sha, stamp) = checked_hash(&origin, &path)?
            .ok_or_else(|| Error::new("operation_failed", format!("[FileNotFoundError] {path}")))?;
        let size = stamp["size"].as_u64().unwrap_or_default();
        let mut candidates = receipts.get(sha.as_str()).cloned().unwrap_or_default();
        let mut local_candidates = Vec::new();
        if local {
            let inner = &path[scope.len() + 1..];
            for asset in &assets {
                let job = crate::reads::job(&catalog, &json!({"job_id": asset["job_id"]}))?;
                let member = if asset["bundle"] == "." {
                    inner.to_owned()
                } else {
                    format!("{}/{inner}", text(asset, "bundle"))
                };
                local_candidates.extend(payloads(
                    db,
                    "SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?",
                    [text(&job["plan"], "id"), text(&archive, "id"), &member],
                )?);
            }
            candidates = local_candidates
                .iter()
                .filter(|copy| copy["sha256"] == sha)
                .map(|copy| text(copy, "destination"))
                .collect();
        } else if candidates.is_empty() {
            if legacy.is_none() {
                let mut by_size = HashMap::<u64, Vec<String>>::new();
                walk(&target, "", |path, size| {
                    if let Some(size) = size {
                        by_size.entry(size).or_default().push(path);
                    }
                    Ok(())
                })?;
                legacy = Some(by_size);
            }
            candidates = legacy
                .as_ref()
                .and_then(|by_size| by_size.get(&size))
                .into_iter()
                .flatten()
                .map(String::as_str)
                .collect();
        }
        let mut matched = None;
        for candidate in candidates {
            let found = match cache.entry(candidate.to_owned()) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let Some((hash, _)) = checked_hash(&target, candidate)? else {
                        continue;
                    };
                    entry.insert(hash)
                }
            };
            if *found == sha {
                matched = Some(candidate);
                break;
            }
        }
        let matched = matched.ok_or_else(|| {
            Error::new(
                "not_covered",
                format!("No verified archive match; archive first: {path}"),
            )
        })?;
        let (original, replacement) = if let Some(bundle) = bundle_root(&path) {
            let replacement = bundle_root(matched).ok_or_else(|| {
                Error::new(
                    "invalid_release",
                    "Bundle needs a complete bundle replacement",
                )
            })?;
            (source_root.join(bundle), archive_root.join(replacement))
        } else {
            (source_root.join(&path), archive_root.join(matched))
        };
        let original = original.to_string_lossy().into_owned();
        let replacement = replacement.to_string_lossy().into_owned();
        if let Some((_, existing)) = replacements.iter().find(|(p, _)| *p == original) {
            require(
                existing.as_deref() == Some(&replacement),
                "invalid_release",
                "Bundle members resolve to different archive bundles",
            )?;
        } else {
            replacements.push((original, Some(replacement)));
        }
        entries.push(json!({"path": path, "stamp": stamp, "sha256": sha, "archive_path": matched}));
        bytes += size;
    }
    for (original, replacement) in &replacements {
        if original.to_lowercase().ends_with(".stalker") {
            verify_bundle(
                Path::new(original),
                Path::new(replacement.as_deref().unwrap_or_default()),
            )?;
        }
    }
    let replacement_checks = if local {
        prefer_work(
            &catalog,
            backend,
            &archive,
            &copies,
            &entries,
            &mut replacements,
        )?
    } else {
        Vec::new()
    };
    let consumers = inspect(&catalog, &replacements)?;
    let replacements: Map<String, Value> = replacements
        .into_iter()
        .map(|(path, value)| (path, json!(value)))
        .collect();
    let id = nu9ve_json::uid()?;
    let mut plan = json!({
        "schema": "nu9ve-release/1", "id": id, "created_at": nu9ve_time::now_utc(), "archive_id": archive["id"],
        "source_root": source_root, "source_identity": source_info, "scope": scope, "work_id": work_id,
        "project": project, "entries": entries, "preserved": preserved, "replacements": replacements,
        "replacement_checks": replacement_checks, "capture_session_id": session, "capture_manifest_digest": manifest_digest,
        "auto_ingest_job_id": params["auto_ingest_job_id"], "bytes": bytes, "consumers": consumers, "state": "review", "cleaned": {},
    });
    plan["digest"] = json!(digest(&plan));
    Ok(plan)
}

pub fn review_release(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    review_with(home, backend, params, |catalog, paths| {
        consumers(home, catalog, paths)
    })
}

fn review_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    inspect: impl FnOnce(&Catalog, &[(String, Option<String>)]) -> Result<Value>,
) -> Result<Value> {
    let id = required(params, "plan_id")?;
    let reviewed = required(params, "digest")?;
    let _lock = writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let key = format!("release:{id}");
    let mut plan = catalog
        .get(&key)?
        .filter(|p| p["digest"] == reviewed)
        .ok_or_else(|| Error::new("plan_changed", "Use the recorded reviewed release plan"))?;
    if plan["state"] != "completed" {
        let current = backend.describe(Path::new(text(&plan, "source_root")), false)?;
        require(
            ["mode", "volume_uuid", "disk_uuid", "root_inode"]
                .iter()
                .all(|key| current[*key] == plan["source_identity"][*key]),
            "source_changed",
            "Release source identity changed",
        )?;
        let paths: Vec<_> = plan["replacements"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(path, value)| (path.clone(), value.as_str().map(str::to_owned)))
            .collect();
        plan["consumers"] = inspect(&catalog, &paths)?;
        WritableCatalog::open(home)?.set(&key, &plan)?;
    }
    Ok(plan)
}

pub(crate) fn stored_plan(catalog: &Catalog, params: &Value) -> Result<Value> {
    let id = required(params, "plan_id")?;
    let reviewed = required(params, "digest")?;
    catalog
        .get(&format!("release:{id}"))?
        .filter(|plan| plan["digest"] == reviewed && plan["id"] == id)
        .ok_or_else(|| Error::new("plan_changed", "Use the recorded reviewed release plan"))
}

fn simple_release(plan: &Value) -> Result<()> {
    require(
        plan["closure_project_id"].is_null(),
        "not_ported",
        "Use the project closeout operation",
    )?;
    release_shape(plan)
}

fn release_shape(plan: &Value) -> Result<()> {
    require(
        plan["schema"] == "nu9ve-release/1"
            && plan["entries"].is_array()
            && plan["cleaned"].is_object()
            && plan["replacements"].is_object()
            && plan["preserved"].is_array(),
        "invalid_release",
        "Malformed stored release plan",
    )
}

pub(crate) fn check_source(backend: &dyn Backend, plan: &Value) -> Result<()> {
    let current = backend.describe(Path::new(text(plan, "source_root")), false)?;
    require(
        ["mode", "volume_uuid", "disk_uuid", "root_inode"]
            .iter()
            .all(|key| current[*key] == plan["source_identity"][*key]),
        "source_changed",
        "Release source identity changed",
    )
}

pub(crate) fn replacements(plan: &Value) -> Vec<(String, Option<String>)> {
    plan["replacements"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(path, value)| (path.clone(), value.as_str().map(str::to_owned)))
        .collect()
}

pub(crate) fn validate_remaining(tree: &Root, plan: &Value) -> Result<()> {
    let entries: HashMap<_, _> = plan["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| (text(entry, "path"), entry))
        .collect();
    let preserved: BTreeSet<_> = plan["preserved"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let (preserved, present, observed, scanned) = if !plan["closure_project_id"].is_null() {
        let (present, observed) = crate::closeout::inventory(tree, plan)?;
        require(
            present
                .iter()
                .all(|path| entries.contains_key(path.as_str())),
            "source_changed",
            "New material appeared; review a fresh release plan",
        )?;
        (preserved, present, observed, Ok(()))
    } else {
        let scope = relative(text(plan, "scope"))?;
        let local = !truthy(&plan["work_id"]);
        let mut present = BTreeSet::new();
        let mut observed = BTreeSet::new();
        let scanned = walk(tree, scope, |path, size| {
            require(
                size.is_some(),
                "unsafe_file",
                format!("Links/special files prevent release: {path}"),
            )?;
            if preserve(&path, local) {
                observed.insert(path);
            } else {
                require(
                    entries.contains_key(path.as_str()),
                    "source_changed",
                    "New material appeared; review a fresh release plan",
                )?;
                present.insert(path);
            }
            Ok(())
        });
        (preserved, present, observed, scanned)
    };
    let removed = |path: &str| matches!(plan["cleaned"][path].as_str(), Some("intent" | "done"));
    if let Err(error) = scanned {
        if fs::python_exception(&error) != Some("FileNotFoundError") {
            return Err(error);
        }
        require(
            entries.keys().all(|path| removed(path)) && preserved.is_empty(),
            "source_changed",
            "Release folder disappeared before cleanup",
        )?;
    }
    require(
        observed
            .iter()
            .map(String::as_str)
            .eq(preserved.iter().copied()),
        "source_changed",
        "New material appeared; review a fresh release plan",
    )?;
    for (path, entry) in entries {
        if present.contains(path) {
            require(
                plan["cleaned"][path] != "done"
                    && tree
                        .stat(path)?
                        .is_some_and(|stamp| stamp_json(&stamp) == entry["stamp"]),
                "source_changed",
                format!("Release source changed: {path}"),
            )?;
        } else {
            require(
                removed(path),
                "source_changed",
                "Release source is missing before cleanup",
            )?;
        }
    }
    Ok(())
}

fn verify_replacements(
    catalog: &WritableCatalog,
    backend: &dyn Backend,
    plan: &Value,
) -> Result<()> {
    for entry in plan["replacement_checks"].as_array().into_iter().flatten() {
        let volume = catalog.volume(text(entry, "volume_id"))?;
        volumes::check(backend, &volume)?;
        let target = Root::open(Path::new(text(&volume, "root")))?;
        require(
            checked_hash(&target, text(entry, "path"))?
                .is_some_and(|(sha, _)| entry["sha256"] == sha),
            "checksum_mismatch",
            "Replacement work copy changed",
        )?;
    }
    Ok(())
}

fn check_archive(target: &Root, entry: &Value, message: &str) -> Result<()> {
    if entry["category"] == "intermediate" {
        return Ok(());
    }
    require(
        checked_hash(target, text(entry, "archive_path"))?
            .is_some_and(|(sha, _)| entry["sha256"] == sha),
        "checksum_mismatch",
        message,
    )
}

fn clear_to_release(consumers: &Value, message: &str) -> Result<()> {
    require(
        consumers["blockers"].as_array().is_some_and(Vec::is_empty)
            && consumers["relinks"].as_array().is_some_and(Vec::is_empty),
        "dependencies_blocked",
        message,
    )
}

pub fn relink_release(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let plan = stored_plan(&Catalog::open(home)?, params)?;
    simple_release(&plan)?;
    let guard = crate::clipper_release::Guard::acquire(home)?;
    relink_with(home, backend, params, &guard)
}

pub(crate) fn relink_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    guard: &crate::clipper_release::Guard,
) -> Result<Value> {
    let _lock = writer_lock(home)?;
    let mut plan = stored_plan(&Catalog::open(home)?, params)?;
    release_shape(&plan)?;
    if !plan["closure_project_id"].is_null() {
        crate::closeout::validate_shape(&plan)?;
        crate::closeout::validate_consent(&Catalog::open(home)?, &plan)?;
    }
    require(
        matches!(plan["state"].as_str(), Some("review" | "applying")),
        "invalid_state",
        "Release already completed",
    )?;
    let id = text(&plan, "id").to_owned();
    let uuid = crate::captures::python_uuid(&id)
        .ok_or_else(|| Error::new("invalid_release", "Invalid release ID"))?
        .replace('-', "");
    let mut namespace = [0; 16];
    for (index, byte) in namespace.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&uuid[index * 2..index * 2 + 2], 16)
            .map_err(|_| Error::new("invalid_release", "Invalid release ID"))?;
    }
    let catalog = WritableCatalog::open(home)?;
    check_source(backend, &plan)?;
    let archive_id = text(&plan, "archive_id").to_owned();
    let archive = catalog.volume(&archive_id)?;
    volumes::check(backend, &archive)?;
    let origin = Root::open(Path::new(text(&plan, "source_root")))?;
    let target = Root::open(Path::new(text(&archive, "root")))?;
    validate_remaining(&origin, &plan)?;
    if !plan["closure_project_id"].is_null() {
        crate::closeout::verify_finals(&target, &origin, &plan, false)?;
    }
    for entry in plan["entries"].as_array().into_iter().flatten() {
        check_archive(&target, entry, "Archive changed before relink")?;
    }
    let paths = replacements(&plan);
    let consumers = guard.inspect(&paths)?;
    let blockers = consumers["blockers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    require(
        blockers.is_empty(),
        "dependencies_blocked",
        blockers.join("; "),
    )?;
    verify_replacements(&catalog, backend, &plan)?;
    crate::units_map::backup_metadata_locked(home, backend, &archive_id, &catalog)?;
    let mut acks = catalog
        .get(&format!("release-acks:{id}"))?
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let pending_key = format!("release-relink-intent:{id}");
    let mut pending = catalog.get(&pending_key)?.unwrap_or(json!({}));
    require(
        pending.is_object(),
        "invalid_release",
        "Malformed relink journal",
    )?;
    for request in consumers["relinks"].as_array().into_iter().flatten() {
        let operation = nu9ve_json::uuid5(
            &namespace,
            &format!(
                "{}:{}:{}",
                text(request, "project"),
                text(request, "asset_id"),
                text(request, "to")
            ),
        );
        require(
            pending.get(&operation).is_none_or(|prior| prior == request),
            "operation_conflict",
            "Relink operation reused with another payload",
        )?;
        pending[operation] = request.clone();
    }
    catalog.set(&pending_key, &pending)?;
    let requests = pending.as_object().cloned().unwrap_or_default();
    for (operation, request) in requests {
        let ack = guard.relink(home, &catalog, &request, &operation)?;
        acks.retain(|prior| prior["operation_id"] != operation);
        acks.push(ack);
        pending.as_object_mut().unwrap().remove(&operation);
        let tx = catalog.db().unchecked_transaction().map_err(sql)?;
        catalog.set(&format!("release-acks:{id}"), &json!(acks))?;
        catalog.set(&pending_key, &pending)?;
        tx.commit().map_err(sql)?;
    }
    let consumers = guard.inspect(&paths)?;
    clear_to_release(&consumers, "Clipper still references released paths")?;
    catalog.set(&format!("release-acks:{id}"), &json!(acks))?;
    plan["consumers"] = consumers.clone();
    catalog.set(&format!("release:{id}"), &plan)?;
    crate::units_map::backup_metadata_locked(home, backend, &archive_id, &catalog)?;
    Ok(json!({"acks": acks, "consumers": consumers}))
}

pub fn apply_release(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let plan = stored_plan(&Catalog::open(home)?, params)?;
    simple_release(&plan)?;
    if plan["state"] == "completed" {
        return Ok(plan);
    }
    let guard = crate::clipper_release::Guard::acquire(home)?;
    apply_with(
        home,
        backend,
        params,
        |paths| guard.inspect(paths),
        &|_, _| Ok(()),
    )
}

fn apply_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    inspect: impl Fn(&[(String, Option<String>)]) -> Result<Value>,
    hook: crate::transfer::Hook<'_>,
) -> Result<Value> {
    apply_reviewed(home, backend, params, inspect, hook, false)
}

pub(crate) fn apply_reviewed(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    inspect: impl Fn(&[(String, Option<String>)]) -> Result<Value>,
    hook: crate::transfer::Hook<'_>,
    closeout: bool,
) -> Result<Value> {
    let _lock = writer_lock(home)?;
    let mut plan = stored_plan(&Catalog::open(home)?, params)?;
    release_shape(&plan)?;
    if closeout {
        crate::closeout::validate_shape(&plan)?;
        crate::closeout::validate_consent(&Catalog::open(home)?, &plan)?;
    } else {
        simple_release(&plan)?;
    }
    if plan["state"] == "completed" {
        return Ok(plan);
    }
    let catalog = WritableCatalog::open(home)?;
    let id = text(&plan, "id").to_owned();
    let key = format!("release:{id}");
    let archive_id = text(&plan, "archive_id").to_owned();
    let archive = catalog.volume(&archive_id)?;
    let work = plan["work_id"]
        .as_str()
        .map(|id| catalog.volume(id))
        .transpose()?;
    let pending = catalog.get(&format!("release-relink-intent:{id}"))?;
    require(
        !pending.as_ref().is_some_and(truthy),
        "dependencies_blocked",
        "Complete the pending Clipper relink and ACK before releasing",
    )?;
    let entries = plan["entries"].as_array().cloned().unwrap_or_default();
    let size = |entry: &Value| entry["stamp"]["size"].as_u64().unwrap_or_default();
    let bytes: u64 = entries.iter().map(size).sum();
    let make_progress = if closeout {
        crate::delivery::ActionProgress::closure
    } else {
        crate::delivery::ActionProgress::new
    };
    let mut progress = make_progress(&catalog, Some(&id), "revalidating_source");
    progress.meter.total = bytes.saturating_mul(2);
    progress.meter.files_total = entries.len();
    progress.report()?;
    let paths = replacements(&plan);
    check_source(backend, &plan)?;
    let archive_info = volumes::check(backend, &archive)?;
    let source_info = backend.describe(Path::new(text(&plan, "source_root")), false)?;
    require(
        volumes::independent(&source_info, &archive_info),
        "same_physical_disk",
        "Archive and released source must be independent disks",
    )?;
    if let Some(work) = &work {
        volumes::check(backend, work)?;
    }
    progress.stage = "revalidating_clipper";
    progress.report()?;
    if !entries.is_empty() {
        clear_to_release(
            &inspect(&paths)?,
            "Resolve Clipper references/queues/offline projects before releasing",
        )?;
    }
    verify_replacements(&catalog, backend, &plan)?;
    let origin = Root::open(Path::new(text(&plan, "source_root")))?;
    let target = Root::open(Path::new(text(&archive, "root")))?;
    validate_remaining(&origin, &plan)?;
    let mut guards = if closeout {
        crate::closeout::verify_finals(&target, &origin, &plan, true)?
    } else {
        Vec::new()
    };
    progress.stage = "verifying_archive";
    let mut done = 0u64;
    for entry in &entries {
        progress.meter.holding(
            text(entry, "archive_path"),
            text(&archive, "name"),
            "",
            "verify",
        );
        progress.report()?;
        if !closeout {
            check_archive(&target, entry, "Archive changed before release")?;
        }
        done += size(entry);
        progress.meter.reach(done);
    }
    progress.stage = "metadata_backup_before";
    progress.meter.current.clear();
    progress.report()?;
    crate::units_map::backup_metadata_locked(home, backend, &archive_id, &catalog)?;
    verify_replacements(&catalog, backend, &plan)?;
    validate_remaining(&origin, &plan)?;
    plan["state"] = json!("applying");
    catalog.set(&key, &plan)?;
    progress.stage = "releasing_originals";
    for entry in &entries {
        let path = text(entry, "path");
        if plan["cleaned"][path] != "done" {
            progress.meter.holding(
                path,
                work.as_ref().map_or("", |v| text(v, "name")),
                "",
                "release",
            );
            progress.report()?;
            check_source(backend, &plan)?;
            volumes::check(backend, &archive)?;
            hook("before_unlink", entry)?;
            clear_to_release(
                &inspect(&paths)?,
                "Clipper dependencies changed during release",
            )?;
            if closeout {
                crate::closeout::watch_finals(&target, &mut guards)?;
            }
            check_archive(&target, entry, "Archive changed during release")?;
            if let Some((sha, stamp)) = checked_hash(&origin, path)? {
                require(
                    entry["sha256"] == sha && entry["stamp"] == stamp,
                    "source_changed",
                    "Source changed before release",
                )?;
                plan["cleaned"][path] = json!("intent");
                catalog.set(&key, &plan)?;
                origin.unlink(path, Some(&stamp))?;
                hook("after_unlink", entry)?;
            } else {
                require(
                    plan["cleaned"][path] == "intent",
                    "source_changed",
                    "Unexpected missing release source",
                )?;
            }
            plan["cleaned"][path] = json!("done");
            catalog.set(&key, &plan)?;
        }
        done += size(entry);
        progress.meter.reach(done);
        progress.meter.finished(
            "released",
            path,
            work.as_ref().map_or("", |v| text(v, "name")),
        );
        progress.report()?;
    }
    validate_remaining(&origin, &plan)?;
    remove_empty_parents(&origin, &plan, &entries);
    hook("before_complete", &Value::Null)?;
    if closeout {
        crate::closeout::watch_finals(&target, &mut guards)?;
    }
    progress.stage = "metadata_backup_after";
    progress.meter.current.clear();
    progress.report()?;
    plan["state"] = json!("completed");
    finish_release(home, &catalog, &plan, &key, &entries, &guards)?;
    match crate::units_map::backup_metadata_locked(home, backend, &archive_id, &catalog) {
        Ok(backup) => plan["metadata_backup"] = backup,
        Err(error) => plan["metadata_backup_error"] = json!(error.message),
    }
    catalog.set(&key, &plan)?;
    progress.stage = "release_complete";
    progress.report()?;
    Ok(plan)
}

/// Continue journaled releases for the verified bundles of an ingest.
pub(crate) fn release_stalker_sources(
    home: &Path,
    backend: &dyn Backend,
    ingest: &Value,
) -> Result<Value> {
    release_stalker_with(home, backend, ingest, || {
        crate::clipper_release::Guard::acquire(home)
    })
}

fn release_stalker_with(
    home: &Path,
    backend: &dyn Backend,
    ingest: &Value,
    acquire: impl Fn() -> Result<crate::clipper_release::Guard>,
) -> Result<Value> {
    let job_id = text(ingest, "id");
    let archive = text(ingest, "archive_id");
    let units: Vec<_> = ingest["units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|unit| unit["kind"] == "stalker_session")
        .collect();
    let mut released = Vec::new();
    let mut retained = Vec::new();
    let catalog = WritableCatalog::open(home)?;
    let key = format!("progress:{job_id}");
    let mut progress = catalog.get(&key)?.unwrap_or(json!({}));
    let mut release_done = 0u64;
    let guard = acquire();
    for (index, unit) in units.iter().enumerate() {
        let root = Path::new(text(ingest, "source"));
        let bundle = text(unit, "bundle");
        let source = if bundle == "." || bundle.is_empty() {
            root.to_path_buf()
        } else {
            root.join(relative(bundle)?)
        };
        progress["phase"] = json!("releasing_stalker");
        progress["sessions_total"] = json!(units.len());
        progress["sessions_done"] = json!(index);
        progress["session_name"] = json!(source.file_name().unwrap_or_default());
        progress["stage"] = json!("preparing");
        catalog.set(&key, &progress)?;
        let item = json!({"session_id": unit["session_id"], "path": source});
        let result = (|| -> Result<()> {
            let plans = crate::reads::release_plans(&Catalog::open(home)?)?;
            let prior = plans.iter().rev().find(|plan| {
                plan["auto_ingest_job_id"] == job_id
                    && plan["archive_id"] == archive
                    && plan["capture_manifest_digest"] == unit["capture_manifest_digest"]
                    && Path::new(text(plan, "source_root")).join(text(plan, "scope")) == source
            });
            if prior.is_some_and(|plan| plan["state"] == "completed") {
                return Ok(());
            }
            let guard = guard.as_ref().map_err(Clone::clone)?;
            let inspect = |_: &Catalog, paths: &[(String, Option<String>)]| guard.inspect(paths);
            let plan = match prior {
                Some(plan) => review_with(
                    home,
                    backend,
                    &json!({"plan_id": plan["id"], "digest": plan["digest"]}),
                    inspect,
                )?,
                None => plan_with(
                    home,
                    backend,
                    &json!({
                        "archive": archive, "source": source, "project": unit["project"], "auto_ingest_job_id": job_id,
                    }),
                    inspect,
                )?,
            };
            let args = json!({"plan_id": plan["id"], "digest": plan["digest"]});
            let blockers = plan["consumers"]["blockers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            require(
                blockers.is_empty(),
                "dependencies_blocked",
                blockers.join("; "),
            )?;
            let pending = Catalog::open(home)?
                .get(&format!("release-relink-intent:{}", text(&plan, "id")))?;
            if truthy(&plan["consumers"]["relinks"]) || pending.as_ref().is_some_and(truthy) {
                relink_with(home, backend, &args, guard)?;
            }
            let result = apply_with(
                home,
                backend,
                &args,
                |paths| guard.inspect(paths),
                &|_, _| Ok(()),
            )?;
            require(
                result["state"] == "completed",
                "invalid_state",
                "La liberación no terminó; el original se conserva para reintentar",
            )
        })();
        match result {
            Ok(()) => {
                release_done += ingest["files"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|file| file["unit_id"] == unit["id"])
                    .filter_map(|file| file["stamp"]["size"].as_u64())
                    .sum::<u64>();
                released.push(item);
            }
            Err(error) => {
                let mut item = item;
                item["reason"] = json!(error.message);
                retained.push(item);
            }
        }
        progress["sessions_done"] = json!(index + 1);
        let total = progress["global_total"].as_u64().unwrap_or_default();
        let done = progress["backup_budget"]
            .as_u64()
            .unwrap_or_default()
            .saturating_add(release_done)
            .min(total);
        progress["global_done"] = json!(done);
        progress["global_fraction"] = json!((done as f64 / total.max(1) as f64).min(0.999));
        progress["stage"] = json!("session_finished");
        catalog.set(&key, &progress)?;
    }
    let _lock = writer_lock(home)?;
    if guard.is_err() {
        let latest = crate::reads::job(&Catalog::open(home)?, &json!({"job_id": job_id}))?;
        if latest["state"] == "completed"
            && let Some(summary) = catalog.get(&format!("stalker-release:{job_id}"))?
        {
            return Ok(summary);
        }
    }
    let state = if retained.is_empty() {
        "completed"
    } else {
        "source_retained"
    };
    let status = if retained.is_empty() {
        "released"
    } else if released.is_empty() {
        "retained"
    } else {
        "partial"
    };
    let summary = json!({"status": status, "released": released, "retained": retained});
    let tx = catalog.db().unchecked_transaction().map_err(sql)?;
    catalog.state(job_id, state, None)?;
    progress["phase"] = json!("stalker_release_done");
    if retained.is_empty() {
        progress["global_fraction"] = json!(1.0);
        progress["global_done"] = progress["global_total"].clone();
    }
    catalog.set(&key, &progress)?;
    catalog.set(&format!("stalker-release:{job_id}"), &summary)?;
    tx.commit().map_err(sql)?;
    Ok(summary)
}

fn remove_empty_parents(tree: &Root, plan: &Value, entries: &[Value]) {
    let scope = Path::new(text(plan, "scope"));
    for entry in entries {
        let mut path = Path::new(text(entry, "path"));
        while let Some(parent) = path.parent().filter(|parent| parent.starts_with(scope)) {
            if !plan["closure_project_id"].is_null()
                && !plan["closure_scopes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|scope| parent.starts_with(text(scope, "path")))
            {
                break;
            }
            let Some(rel) = parent.to_str() else { break };
            let Ok((directory, name)) = tree.parent(rel, false) else {
                break;
            };
            if std::fs::remove_dir(directory.join(name)).is_err()
                || fs::sync_dir(&directory).is_err()
            {
                break;
            }
            path = parent;
        }
    }
}

fn finish_release(
    home: &Path,
    catalog: &WritableCatalog,
    plan: &Value,
    key: &str,
    entries: &[Value],
    guards: &[Value],
) -> Result<()> {
    let tx = catalog.db().unchecked_transaction().map_err(sql)?;
    if let Some(work) = plan["work_id"].as_str() {
        let removed: BTreeSet<_> = entries.iter().map(|entry| text(entry, "path")).collect();
        for asset in payloads(&tx, "SELECT payload FROM assets", [])? {
            let copies = payloads(
                &tx,
                "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
                [text(&asset, "job_id"), work],
            )?;
            let mut paths = copies.iter().filter(|copy| {
                asset["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.contains(&copy["path"]))
            });
            if paths
                .next()
                .is_some_and(|first| removed.contains(text(first, "destination")))
                && paths.all(|copy| removed.contains(text(copy, "destination")))
            {
                tx.execute("UPDATE work SET state='skipped',target=NULL,updated_at=? WHERE asset_id=? AND target=?",
                    (nu9ve_time::now_utc(), text(&asset, "id"), work)).map_err(sql)?;
            }
        }
    }
    if !plan["closure_project_id"].is_null() {
        crate::closeout::save_final_locations(&Catalog::open(home)?, catalog, plan, guards)?;
    }
    catalog.set(key, plan)?;
    catalog.event(
        text(plan, "id"),
        "release_completed",
        &json!({"bytes": plan["bytes"], "scope": plan["scope"]}),
    )?;
    tx.commit().map_err(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_fixture() -> (crate::volumes::test_backend::Setup, Value, Value) {
        let s = crate::volumes::test_backend::setup();
        let scope = "001_proyectos/intro";
        let folder = s.work.join(scope);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("a.mov"), b"original").unwrap();
        std::fs::write(folder.join("b.mov"), b"second").unwrap();
        std::fs::write(folder.join("notes.txt"), b"preserved").unwrap();
        std::fs::write(s.archive.join("a.mov"), b"original").unwrap();
        std::fs::write(s.archive.join("b.mov"), b"second").unwrap();
        let plan = plan_with(
            &s.home,
            &s.backend,
            &json!({"archive": "MEDIA", "work": "ESSD", "path": scope, "project": "intro"}),
            |_, paths| empty_consumers(paths),
        );
        let plan = plan.unwrap();
        let args = json!({"plan_id": plan["id"], "digest": plan["digest"]});
        (s, plan, args)
    }

    fn empty_consumers(_: &[(String, Option<String>)]) -> Result<Value> {
        Ok(json!({"blockers": [], "relinks": []}))
    }

    #[test]
    fn release_checks_all_archive_bytes_before_deleting_and_recovers_unlink_intent() {
        let (s, plan, args) = release_fixture();
        let folder = s.work.join(text(&plan, "scope"));
        let target = s.archive.join("b.mov");
        let no_hook = |_: &str, _: &Value| Ok(());
        std::fs::write(&target, b"badbad").unwrap();
        let error = apply_with(&s.home, &s.backend, &args, empty_consumers, &no_hook).unwrap_err();
        assert_eq!(error.code, "checksum_mismatch");
        assert!(folder.join("a.mov").exists() && folder.join("b.mov").exists());
        std::fs::write(target, b"second").unwrap();
        let stop = |stage: &str, _: &Value| {
            require(
                stage != "after_unlink",
                "fixture_interrupted",
                "process stopped after unlink",
            )
        };
        let error = apply_with(&s.home, &s.backend, &args, empty_consumers, &stop).unwrap_err();
        assert_eq!(error.code, "fixture_interrupted");
        assert!(!folder.join("a.mov").exists() && folder.join("b.mov").exists());
        let persisted = stored_plan(&Catalog::open(&s.home).unwrap(), &args).unwrap();
        assert_eq!(persisted["cleaned"]["001_proyectos/intro/a.mov"], "intent");
        let done = apply_with(&s.home, &s.backend, &args, empty_consumers, &no_hook).unwrap();
        assert_eq!(done["state"], "completed");
        assert!(!folder.join("b.mov").exists());
        assert_eq!(
            std::fs::read(folder.join("notes.txt")).unwrap(),
            b"preserved"
        );
        assert_eq!(
            apply_with(&s.home, &s.backend, &args, empty_consumers, &no_hook).unwrap(),
            done
        );
    }

    #[test]
    fn release_rechecks_consumers_and_preserves_pending_relinks() {
        let (s, plan, args) = release_fixture();
        let checks = std::cell::Cell::new(0);
        let inspect = |paths: &[(String, Option<String>)]| {
            checks.set(checks.get() + 1);
            if checks.get() >= 3 {
                Ok(json!({"blockers": ["queued"], "relinks": []}))
            } else {
                empty_consumers(paths)
            }
        };
        let no_hook = |_: &str, _: &Value| Ok(());
        let error = apply_with(&s.home, &s.backend, &args, inspect, &no_hook).unwrap_err();
        assert_eq!(error.code, "dependencies_blocked");
        let folder = s.work.join(text(&plan, "scope"));
        assert!(!folder.join("a.mov").exists() && folder.join("b.mov").exists());
        let catalog = WritableCatalog::open(&s.home).unwrap();
        catalog
            .set(
                &format!("release-relink-intent:{}", plan["id"].as_str().unwrap()),
                &json!({"op": {"to": "pending"}}),
            )
            .unwrap();
        let error = apply_with(&s.home, &s.backend, &args, empty_consumers, &no_hook).unwrap_err();
        assert_eq!(error.code, "dependencies_blocked");
        assert!(folder.join("b.mov").exists());
    }

    #[cfg(unix)]
    #[test]
    fn clipper_lock_token_and_relink_ack_survive_retry() {
        use std::os::unix::fs::PermissionsExt;
        let (s, plan, args) = release_fixture();
        let home = s.home.with_file_name("clipper-home");
        let lock_dir = home.join(".cache/clipper");
        let executable = s.home.join("clipper-fixture");
        let paths = replacements(&plan);
        let original = &paths[0].0;
        let destination = paths[0].1.as_ref().unwrap();
        let dependency = json!({"schema": "clipper-library-deps/1", "capabilities": ["media-release-lock/1"],
            "offline_projects": [], "projects": [{"clipper_root": "/fixture/project", "relinks": [],
                "assets": [{"path": original, "in_use": true, "queued_exports": [], "items": [],
                    "revision": "r1", "asset_id": "a", "kind": "camera_clip"}]}]});
        let empty = json!({"schema": "clipper-library-deps/1", "capabilities": ["media-release-lock/1"],
            "offline_projects": [], "projects": []});
        let script = format!(
            r#"#!/bin/sh
set -eu
lock='{}/.media-access.lock'
[ -n "$CLIPPER_MEDIA_RELEASE_TOKEN" ]
/usr/bin/grep -q "\"token\":\"$CLIPPER_MEDIA_RELEASE_TOKEN\"" "$lock"
case "$2" in
  deps)
    if [ -f '{}/relinked' ]; then
      printf '%s\n' '{}'
    else
      printf '%s\n' '{}'
    fi ;;
  relink)
    [ "$3" = /fixture/project ] && [ "$4" = a ] && [ "$5" = --to ] && [ "$6" = '{}' ] && [ "$7" = --operation-id ]
    if [ -f '{}/operation' ]; then
      [ "$(cat '{}/operation')" = "$8" ]
    fi
    printf '%s' "$8" > '{}/operation'
    touch '{}/relinked'
    pins='[]'
    if [ ! -f '{}/valid' ]; then pins='["export"]'; fi
    printf '{{"schema":"clipper-relink-ack/1","status":"applied","asset_id":"a","operation_id":"%s","from":"{}","to":"{}","revision":"r1","still_pinned":%s}}\n' "$8" "$pins" ;;
esac
"#,
            lock_dir.display(),
            s.home.display(),
            empty,
            dependency,
            destination,
            s.home.display(),
            s.home.display(),
            s.home.display(),
            s.home.display(),
            s.home.display(),
            original,
            destination
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let catalog = WritableCatalog::open(&s.home).unwrap();
        catalog
            .set("clipper-executable", &json!(executable))
            .unwrap();
        let guard = crate::clipper_release::Guard::acquire_at(&s.home, home.clone()).unwrap();
        let error = crate::clipper_release::Guard::acquire_at(&s.home, home.clone())
            .err()
            .unwrap();
        assert_eq!(error.code, "clipper_busy");
        assert_eq!(
            relink_with(&s.home, &s.backend, &args, &guard)
                .unwrap_err()
                .code,
            "clipper_ack_invalid"
        );
        assert!(Path::new(original).exists());
        assert_eq!(
            apply_with(
                &s.home,
                &s.backend,
                &args,
                |paths| guard.inspect(paths),
                &|_, _| Ok(())
            )
            .unwrap_err()
            .code,
            "dependencies_blocked"
        );
        std::fs::write(s.home.join("valid"), b"").unwrap();
        let result = relink_with(&s.home, &s.backend, &args, &guard).unwrap();
        assert_eq!(result["acks"].as_array().unwrap().len(), 1);
        assert!(Path::new(original).exists());
        let done = apply_with(
            &s.home,
            &s.backend,
            &args,
            |paths| guard.inspect(paths),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(done["state"], "completed");
        assert!(!Path::new(original).exists());
        drop(guard);
        assert!(crate::clipper_release::Guard::acquire_at(&s.home, home).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn auto_stalker_release_resumes_from_a_completed_release_without_the_source() {
        use std::os::unix::fs::PermissionsExt;
        let s = crate::volumes::test_backend::setup();
        let bundle = s.card.join("S.stalker");
        std::fs::create_dir(&bundle).unwrap();
        let manifest = json!({"producer": "stalker", "schemaVersion": 3, "status": "completed",
            "sessionID": "123e4567-e89b-12d3-a456-426614174000", "createdAt": "2026-11-04T23:32:12Z",
            "tracks": [{"relativePath": "screen.mov"}]});
        std::fs::write(bundle.join("manifest.json"), manifest.to_string()).unwrap();
        std::fs::write(bundle.join("screen.mov"), b"capture").unwrap();
        let ingest = crate::planner::build_plan(&s.home, &s.backend, &json!({
            "source": s.card, "source_kind": "stalker", "archive": "MEDIA", "work": "ESSD", "project": "intro"
        })).unwrap();
        assert_eq!(ingest["release_stalker_source"], true);
        let catalog = WritableCatalog::open(&s.home).unwrap();
        {
            let _lock = writer_lock(&s.home).unwrap();
            let mut transfer =
                crate::transfer::Transfer::new(&s.home, &catalog, &s.backend, &|_, _| Ok(()));
            assert_eq!(
                transfer.apply(&ingest, text(&ingest, "digest")).unwrap()["state"],
                "source_retained"
            );
        }
        let clipper = s.home.join("clipper-fixture");
        std::fs::write(&clipper, r#"#!/bin/sh
echo '{"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],"projects":[],"offline_projects":[]}'
"#).unwrap();
        std::fs::set_permissions(&clipper, std::fs::Permissions::from_mode(0o700)).unwrap();
        catalog.set("clipper-executable", &json!(clipper)).unwrap();
        let acquire = || {
            crate::clipper_release::Guard::acquire_at(
                &s.home,
                s.home.with_file_name("clipper-home"),
            )
        };
        let summary = release_stalker_with(&s.home, &s.backend, &ingest, acquire).unwrap();
        assert_eq!(summary["status"], "released");
        assert!(!bundle.exists());
        assert_eq!(
            release_stalker_with(&s.home, &s.backend, &ingest, acquire).unwrap(),
            summary
        );
        let job = crate::reads::job(
            &Catalog::open(&s.home).unwrap(),
            &json!({"job_id": ingest["id"]}),
        )
        .unwrap();
        assert_eq!(job["state"], "completed");
    }

    #[test]
    fn stalker_release_requires_the_archived_revision_and_prefers_verified_work() {
        let s = crate::volumes::test_backend::setup();
        let bundle = s.card.join("S.stalker");
        std::fs::create_dir(&bundle).unwrap();
        let manifest = json!({"producer": "stalker", "schemaVersion": 3, "status": "completed",
            "sessionID": "123e4567-e89b-12d3-a456-426614174000", "createdAt": "2026-11-04T23:32:12Z",
            "tracks": [{"relativePath": "screen.mov"}]});
        std::fs::write(
            bundle.join("manifest.json"),
            nu9ve_json::canonical(&manifest),
        )
        .unwrap();
        std::fs::write(bundle.join("screen.mov"), b"capture").unwrap();
        let options = json!({"archive": "MEDIA", "source": bundle, "project": "intro"});
        let inspect = |_: &Catalog, _: &[(String, Option<String>)]| {
            Ok(json!({"blockers": [], "relinks": []}))
        };
        assert_eq!(
            plan_with(&s.home, &s.backend, &options, inspect)
                .unwrap_err()
                .code,
            "missing_receipt"
        );
        WritableCatalog::open(&s.home)
            .unwrap()
            .set("retain-stalker-source", &json!(true))
            .unwrap();
        let ingest = crate::planner::build_plan(&s.home, &s.backend, &json!({
            "source": s.card, "source_kind": "stalker", "archive": "MEDIA", "work": "ESSD", "project": "intro"
        })).unwrap();
        crate::transfer::apply(
            &s.home,
            &s.backend,
            &ingest,
            text(&ingest, "digest"),
            &|_, _| Ok(()),
        )
        .unwrap();
        let planned = plan_with(&s.home, &s.backend, &options, inspect).unwrap();
        assert_eq!(planned["capture_session_id"], manifest["sessionID"]);
        assert_eq!(planned["replacement_checks"].as_array().unwrap().len(), 2);
        let replacement = planned["replacements"][bundle.to_string_lossy().as_ref()]
            .as_str()
            .unwrap();
        assert!(Path::new(replacement).starts_with(&s.work));
        assert!(bundle.join("screen.mov").is_file());
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn release_plan_review_and_apply_match_python() {
        let s = crate::volumes::test_backend::setup();
        let root = s.archive.parent().unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_slice(&std::fs::read(fixtures).unwrap()).unwrap();
        let case = &golden["release"];
        for (path, data) in case["files"].as_object().unwrap() {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, data.as_str().unwrap()).unwrap();
        }
        let normalize = |mut plan: Value| {
            for key in ["id", "created_at", "digest", "metadata_backup"] {
                plan.as_object_mut().unwrap().remove(key);
            }
            plan["source_identity"]["root_inode"] = json!("<INODE>");
            plan["source_identity"]["device"] = json!("<DEVICE>");
            for entry in plan["entries"].as_array_mut().unwrap() {
                entry["stamp"]["inode"] = json!("<INODE>");
                entry["stamp"]["ctime_ns"] = json!("<CTIME>");
                entry["stamp"]["mtime_ns"] = json!("<MTIME>");
            }
            plan["archive_id"] = json!("MEDIA");
            plan["work_id"] = json!("ESSD");
            let root = json!(root).to_string();
            serde_json::from_str::<Value>(
                &plan.to_string().replace(&root[1..root.len() - 1], "<ROOT>"),
            )
            .unwrap()
        };
        let params = json!({"archive": "MEDIA", "work": "ESSD", "path": "001_proyectos/intro", "project": "intro"});
        let plan = plan_with(&s.home, &s.backend, &params, |_, _| {
            Ok(case["plan"]["consumers"].clone())
        })
        .unwrap();
        assert_eq!(normalize(plan.clone()), case["plan"]);
        let mut body = plan.clone();
        body.as_object_mut().unwrap().remove("digest");
        assert_eq!(plan["digest"], digest(&body));
        let reviewed = review_with(
            &s.home,
            &s.backend,
            &json!({"plan_id": plan["id"], "digest": plan["digest"]}),
            |_, _| Ok(case["review"]["consumers"].clone()),
        )
        .unwrap();
        assert_eq!(normalize(reviewed.clone()), case["review"]);
        assert_eq!(reviewed["digest"], plan["digest"]);
        let saved = Catalog::open(&s.home)
            .unwrap()
            .get(&format!("release:{}", text(&plan, "id")))
            .unwrap()
            .unwrap();
        assert_eq!(saved, reviewed);
        let applied = apply_with(
            &s.home,
            &s.backend,
            &json!({"plan_id": plan["id"], "digest": plan["digest"]}),
            |_| Ok(case["plan"]["consumers"].clone()),
            &|_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(normalize(applied), case["applied"]);
        let mut remaining = Vec::new();
        walk(&Root::open(&s.work).unwrap(), "", |path, _| {
            remaining.push(path);
            Ok(())
        })
        .unwrap();
        remaining.sort();
        assert_eq!(json!(remaining), case["remaining"]);
        assert_eq!(
            review_with(
                &s.home,
                &s.backend,
                &json!({"plan_id": plan["id"], "digest": "changed"}),
                |_, _| unreachable!()
            )
            .unwrap_err()
            .code,
            "plan_changed"
        );
    }
}
