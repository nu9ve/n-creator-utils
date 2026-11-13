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

fn checked_hash(tree: &Root, path: &str) -> Result<Option<(String, Value)>> {
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

fn consumers(
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
    let mut paths = Vec::new();
    let mut preserved = Vec::new();
    walk(&origin, &scope, |path, size| {
        require(
            size.is_some(),
            "unsafe_file",
            format!("Links/special files prevent release: {path}"),
        )?;
        let in_bundle = path
            .rsplit_once('/')
            .is_some_and(|(parent, _)| bundle_root(parent).is_some());
        let media = crate::snapshot::kind_of(&path)
            .is_some_and(|kind| crate::layout::MEDIA_KINDS.contains(&kind));
        if protected(&path) || (!local && !in_bundle && !media) {
            preserved.push(path);
        } else {
            paths.push(path);
        }
        Ok(())
    })?;
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
    let _lock = writer_lock(home)?;
    WritableCatalog::open(home)?.set(&format!("release:{id}"), &plan)?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn release_plan_and_review_match_python() {
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
            for key in ["id", "created_at", "digest"] {
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
