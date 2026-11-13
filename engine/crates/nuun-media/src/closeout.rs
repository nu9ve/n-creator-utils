//! Reviewed project closeout using the shared ingest and release journals.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::clipper_release::Guard;
use crate::delivery::ActionProgress;
use crate::planner::{build_plan, destination_path};
use crate::reads::{closure_settings, project_record, related};
use crate::release::{self, checked_hash};
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend, truthy};
use crate::{editorial, transcripts};
use nu9ve_json::{Error, Result, digest, require, uid};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn param<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params[key]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
}

fn settings_group(catalog: &Catalog, id: &str) -> Result<Vec<(String, Value)>> {
    let mut ids = vec![id.to_owned()];
    ids.extend(related(catalog, id)?);
    ids.into_iter()
        .map(|id| Ok((id.clone(), closure_settings(catalog, &id)?)))
        .collect()
}

fn group_digest(group: &[(String, Value)]) -> String {
    if group.len() == 1 {
        digest(&group[0].1)
    } else {
        digest(&Value::Object(group.iter().cloned().collect()))
    }
}

fn check_decisions(catalog: &Catalog, plan: &Value) -> Result<()> {
    require(
        group_digest(&settings_group(catalog, text(plan, "closure_project_id"))?)
            == plan["closure_settings_digest"],
        "plan_changed",
        "Cambió el cierre del proyecto; revisa un plan nuevo",
    )
}

pub(crate) fn validate_consent(catalog: &Catalog, plan: &Value) -> Result<()> {
    require(
        catalog
            .get(&format!("closure-consent:{}", text(plan, "id")))?
            .is_some_and(|consent| consent["digest"] == plan["digest"]),
        "confirmation_required",
        "Confirma el cierre completo desde su revisión antes de liberar",
    )?;
    check_decisions(catalog, plan)
}

pub fn plan(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    plan_with(home, backend, params, |catalog, paths| {
        release::consumers(home, catalog, paths)
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
            !["project_id", "archive", "work", "release_work", "review_id"].contains(&key.as_str())
        })
    }) {
        require(
            key != "progress",
            "invalid_request",
            "Progress callbacks are internal to the local service",
        )?;
        return Err(Error::new(
            "operation_failed",
            format!("plan_close() got an unexpected keyword argument '{key}'"),
        ));
    }
    let project_id = param(params, "project_id")?;
    let archive = param(params, "archive")?;
    let work = param(params, "work")?;
    let release_work = params
        .get("release_work")
        .map_or(Some(true), Value::as_bool)
        .ok_or_else(|| Error::new("invalid_request", "release_work debe ser booleano"))?;
    if !params["review_id"].is_null() {
        require(
            params["review_id"].as_str().is_some_and(|id| {
                crate::captures::python_uuid(id)
                    .is_some_and(|normalized| normalized == id.to_ascii_lowercase())
            }),
            "invalid_request",
            "Expected a review UUID",
        )?;
    }
    let catalog = Catalog::open(home)?;
    let progress_catalog = WritableCatalog::open(home)?;
    let mut progress = ActionProgress::closure(
        &progress_catalog,
        params["review_id"].as_str(),
        "review_inventory",
    );
    progress.report()?;
    let (_, profile, title) = project_record(&catalog, project_id)?;
    let group = settings_group(&catalog, project_id)?;
    let config_digest = group_digest(&group);
    let mut titles = Map::new();
    let mut owned = Vec::new();
    for (id, settings) in &group {
        let (_, _, title) = project_record(&catalog, id)?;
        titles.insert(id.clone(), json!(title));
        for item in settings["finals"].as_array().into_iter().flatten() {
            let published = item
                .get("published")
                .map_or_else(|| truthy(&settings["published"]), truthy);
            if release_work || published {
                owned.push((id, item, published));
            }
        }
    }
    if release_work {
        require(
            group[0].1["no_more"] == true,
            "project_incomplete",
            "Confirma que la selección contiene todas las entregas definitivas",
        )?;
        require(
            group[0].1["required"]
                .as_array()
                .into_iter()
                .flatten()
                .all(|role| owned.iter().any(|(_, item, _)| item["role"] == *role)),
            "missing_deliveries",
            "Faltan tipos de entrega requeridos",
        )?;
    } else {
        require(
            !owned.is_empty(),
            "project_incomplete",
            "Marca al menos un final como publicado",
        )?;
    }
    let archive = catalog.volume(archive)?;
    let work = catalog.volume(work)?;
    require(
        work["role"] == "work" && archive["role"] == "archive",
        "invalid_role",
        "Elige trabajo y respaldo",
    )?;
    let source_info = volumes::check(backend, &work)?;
    let archive_info = volumes::check(backend, &archive)?;
    require(
        volumes::independent(&source_info, &archive_info),
        "same_physical_disk",
        "Trabajo y respaldo deben ser discos distintos",
    )?;
    let archive_root = Path::new(text(&archive, "root"));
    let work_root = Path::new(text(&work, "root"));
    let archive_tree = Root::open(archive_root)?;
    let mut candidates = Vec::new();
    let prefix = |volume: &Value| {
        format!(
            "{}/{profile}",
            volume["finals_prefix"].as_str().unwrap_or("002_finales")
        )
    };
    crate::projects::delivery_files(&archive_root.join(prefix(&archive)), &mut candidates);
    let mut inventory = if release_work {
        review_editorial(home, backend, &catalog, &group, &archive, &work, &title)?
    } else {
        EditorialReview::default()
    };
    let mut copies = Vec::new();
    let mut claimed = BTreeMap::new();
    for raw in &inventory.finals {
        let path = text(raw, "archive_path");
        let sha = text(raw, "sha256");
        require(
            claimed
                .insert(path.to_owned(), (sha.to_owned(), Value::Null))
                .is_none_or(|(prior, _)| prior == sha),
            "destination_conflict",
            "Dos archivos distintos llegarían al mismo destino",
        )?;
    }
    let mut originals = BTreeSet::new();
    let mut needed = 0u64;
    progress.meter.files_total = owned.len();
    progress.stage = "review_final";
    for (owner, item, published) in owned {
        let volume = catalog.volume(text(item, "volume_id"))?;
        volumes::check(backend, &volume)?;
        let rel = crate::rules::relative(text(item, "path"))?;
        let tree = Root::open(Path::new(text(&volume, "root")))?;
        progress
            .meter
            .holding(rel, text(&volume, "name"), "", "verify");
        progress.report()?;
        let (sha, stamp) = checked_hash(&tree, rel)?
            .ok_or_else(|| Error::new("operation_failed", format!("[FileNotFoundError] {rel}")))?;
        require(
            stamp == item["stamp"],
            "source_changed",
            "Un final cambió; vuelve a vincularlo",
        )?;
        let mut copy_required = false;
        let keep_work = volume["role"] == "work" && !published;
        let mut archive_stamp = stamp.clone();
        let archive_path = if volume["role"] == "archive" {
            require(
                volume["id"] == archive["id"],
                "invalid_volume",
                "El final está en otro respaldo",
            )?;
            rel.to_owned()
        } else {
            require(
                volume["id"] == work["id"],
                "invalid_volume",
                "El final está en otro disco de trabajo",
            )?;
            require(
                Path::new(rel).starts_with(prefix(&work)),
                "unsafe_path",
                "Selecciona finales del mismo perfil",
            )?;
            let name = rel.rsplit('/').next().unwrap_or(rel);
            let mut existing = None;
            for candidate in candidates
                .iter()
                .filter(|path| path.file_name().is_some_and(|n| n == name))
            {
                if nu9ve_volumes::stamp(candidate)?.size
                    != stamp["size"].as_u64().unwrap_or_default()
                {
                    continue;
                }
                let path = candidate
                    .strip_prefix(archive_root)
                    .map_err(|_| Error::new("unsafe_path", "Archive path outside its root"))?
                    .to_string_lossy();
                if let Some((found, candidate_stamp)) = checked_hash(&archive_tree, &path)?
                    && found == sha
                {
                    archive_stamp = candidate_stamp;
                    existing = Some(path.into_owned());
                    break;
                }
            }
            let dest = if let Some(path) = existing {
                path
            } else {
                let parent = work_root
                    .join(rel)
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| Error::new("unsafe_path", "Final has no parent"))?;
                let role = if item["role"] == "auxiliary"
                    && rel
                        .split('/')
                        .any(|p| p.to_lowercase().contains("thumbnail"))
                {
                    "thumbnail"
                } else {
                    text(item, "role")
                };
                let transfer = build_plan(
                    home,
                    backend,
                    &json!({
                        "source": parent, "archive": archive["id"], "project": titles[owner],
                        "profile": profile, "archive_only": true, "confirm_archive_only": true,
                        "work_decision": "skipped", "cleanup": false, "camera": "generic", "source_kind": "delivery",
                        "delivery_paths": [name], "finals_route": true, "finals_role": role,
                    }),
                )?;
                let dest =
                    destination_path(&archive, &transfer, &transfer["units"][0], name, true)?
                        .ok_or_else(|| Error::new("invalid_plan", "Missing finals destination"))?;
                if let Some((existing, _)) = checked_hash(&archive_tree, &dest)? {
                    require(
                        existing == sha,
                        "destination_conflict",
                        "Ya existe otro final distinto en el destino elegido",
                    )?;
                }
                needed += stamp["size"].as_u64().unwrap_or_default();
                copies.push(transfer);
                copy_required = true;
                archive_stamp = Value::Null;
                dest
            };
            if !keep_work {
                require(
                    originals.insert(rel),
                    "overlapping_roots",
                    "Hay carpetas superpuestas; revisa las asociaciones",
                )?;
                inventory.entries.push(json!({"path": rel, "stamp": stamp, "sha256": sha, "archive_path": dest, "category": "final"}));
                inventory.replacements.insert(
                    work_root.join(rel).to_string_lossy().into_owned(),
                    Some(archive_root.join(&dest).to_string_lossy().into_owned()),
                );
            }
            dest
        };
        if let Some((previous, role)) =
            claimed.insert(archive_path.clone(), (sha.clone(), item["role"].clone()))
        {
            require(
                previous == sha,
                "destination_conflict",
                "Dos archivos distintos llegarían al mismo destino",
            )?;
            require(
                role.is_null() || role == item["role"],
                "destination_conflict",
                "La misma entrega no puede contar como dos tipos distintos",
            )?;
        }
        inventory.finals.push(json!({"archive_path": archive_path, "sha256": sha, "role": item["role"], "owner": owner,
            "original": {"volume_id": item["volume_id"], "path": rel}, "original_stamp": stamp,
            "copy_required": copy_required, "keep_work": keep_work, "archive_stamp": archive_stamp}));
        progress
            .meter
            .finished("verified", rel, text(&volume, "name"));
    }
    let source_info = volumes::check(backend, &work)?;
    let archive_info = volumes::check(backend, &archive)?;
    require(
        volumes::independent(&source_info, &archive_info),
        "same_physical_disk",
        "Trabajo y respaldo deben ser discos distintos",
    )?;
    require(
        needed == 0
            || archive_info["free"].as_u64().unwrap_or_default()
                >= needed + (64 * 1024 * 1024).max(needed / 20),
        "insufficient_space",
        "No hay espacio para todos los finales seleccionados",
    )?;
    require(
        release_work || !inventory.entries.is_empty(),
        "nothing_to_release",
        "Los finales publicados elegidos ya están en el respaldo",
    )?;
    let mut paths = BTreeSet::new();
    require(
        inventory
            .entries
            .iter()
            .all(|entry| paths.insert(text(entry, "path"))),
        "overlapping_roots",
        "Hay carpetas superpuestas; revisa las asociaciones",
    )?;
    let (regenerable, transcript_plan) = if release_work {
        let roots = transcripts::clipper_roots(&work, &inventory.editors)?;
        let (regenerable, notes) = editorial::regenerables(home, &catalog, work_root, &roots);
        inventory.retained.extend(notes);
        let transcripts =
            transcripts::plan(&catalog, backend, text(&archive, "id"), &roots, |root| {
                editorial::invoke(
                    home,
                    &catalog,
                    &["transcripts-export", &root.to_string_lossy()],
                )
            })?;
        (regenerable, transcripts)
    } else {
        (Vec::new(), Value::Null)
    };
    progress.stage = "inspecting_clipper";
    progress.meter.current.clear();
    progress.report()?;
    let replacements: Vec<_> = inventory
        .replacements
        .iter()
        .map(|(from, to)| (from.clone(), to.clone()))
        .collect();
    let consumers = if inventory.entries.is_empty() {
        json!({"blockers": [], "relinks": [], "dependencies": null})
    } else {
        inspect(&catalog, &replacements)?
    };
    let bytes: u64 = inventory
        .entries
        .iter()
        .filter_map(|entry| entry["stamp"]["size"].as_u64())
        .sum();
    let count = |field: &str| {
        regenerable
            .iter()
            .filter_map(|group| group[field].as_u64())
            .sum::<u64>()
    };
    let already_complete = release_work
        && inventory.entries.is_empty()
        && copies.is_empty()
        && regenerable.is_empty()
        && !truthy(&transcript_plan["files"])
        && group
            .iter()
            .map(|(id, _)| catalog.get(&format!("project-released:{id}")))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .all(|value| value.as_ref().is_some_and(truthy));
    let id = uid()?;
    let mut value = json!({"schema": "nu9ve-release/1", "id": id, "created_at": nu9ve_time::now_utc(),
        "archive_id": archive["id"], "source_root": work["root"], "source_identity": source_info,
        "work_id": work["id"], "scope": work["prefix"], "project": title, "entries": inventory.entries, "preserved": inventory.preserved,
        "replacements": inventory.replacements, "replacement_checks": [], "bytes": bytes, "consumers": consumers,
        "state": "review", "cleaned": {}, "retained_editors": inventory.retained, "closure_project_id": project_id,
        "closure_settings_digest": config_digest, "closure_scopes": inventory.scopes, "closure_finals": inventory.finals, "closure_copies": copies,
        "closure_finish": release_work, "closure_inventory_checked": true,
        "closure_group": group.iter().map(|(id, _)| id).collect::<Vec<_>>(), "closure_group_titles": titles,
        "clipper_regenerable": regenerable, "clipper_files": count("files"), "clipper_bytes": count("bytes"), "clipper_cleaned": {},
        "transcripts": transcript_plan, "already_complete": already_complete});
    value["digest"] = json!(digest(&value));
    let _lock = writer_lock(home)?;
    check_decisions(&Catalog::open(home)?, &value)?;
    WritableCatalog::open(home)?.set(&format!("release:{id}"), &value)?;
    Ok(value)
}

#[derive(Default)]
struct EditorialReview {
    entries: Vec<Value>,
    finals: Vec<Value>,
    replacements: BTreeMap<String, Option<String>>,
    preserved: BTreeSet<String>,
    editors: Vec<String>,
    scopes: Vec<Value>,
    retained: Vec<String>,
}

fn review_editorial(
    home: &Path,
    backend: &dyn Backend,
    catalog: &Catalog,
    group: &[(String, Value)],
    archive: &Value,
    work: &Value,
    title: &str,
) -> Result<EditorialReview> {
    let work_root = Path::new(text(work, "root"));
    let origin = Root::open(work_root)?;
    let target = Root::open(Path::new(text(archive, "root")))?;
    let all_assets = crate::reads::library(catalog, &json!({}))?;
    let mut assets = Vec::new();
    let mut raw_scopes = Vec::new();
    let mut review = EditorialReview::default();
    let profiles = catalog.get("creative-profiles")?.unwrap_or(Value::Null);
    for (id, _) in group {
        let (_, profile, title) = project_record(catalog, id)?;
        let fallback = format!("{profile}_raw");
        let raw_dir = profiles[&profile]["raw_dir"].as_str().unwrap_or(&fallback);
        let mut raw = BTreeSet::new();
        for asset in all_assets
            .iter()
            .filter(|asset| asset["project_id"] == *id && asset["kind"] != "delivery")
        {
            let job_id = text(asset, "job_id");
            let job = crate::reads::job(catalog, &json!({"job_id": job_id}))?;
            let profile = text(asset, "profile");
            let default_route = format!("{profile}_raw");
            let route = job["plan"]["profile_routes"][profile]
                .as_str()
                .unwrap_or(&default_route);
            let depth = text(work, "prefix").split('/').count() + route.split('/').count() + 1;
            for copy in catalog.json_rows(
                "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
                (job_id, text(work, "id")),
            )? {
                if asset["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.contains(&copy["path"]))
                {
                    let parts: Vec<_> = crate::rules::relative(text(&copy, "destination"))?
                        .split('/')
                        .collect();
                    if parts.len() > depth {
                        raw.insert(parts[..depth].join("/"));
                    }
                }
            }
            assets.push((asset, job));
        }
        let mut titles = BTreeSet::from([title]);
        let aliases = catalog.json_rows(
            "SELECT json_quote(alias) FROM project_aliases WHERE project_id=?",
            [id],
        )?;
        titles.extend(aliases.iter().filter_map(Value::as_str).map(str::to_owned));
        raw.extend(
            titles
                .iter()
                .map(|title| format!("{}/{raw_dir}/{title}", text(work, "prefix"))),
        );
        for scope in raw {
            if !raw_scopes.contains(&scope) {
                raw_scopes.push(scope);
            }
        }
        for editor in transcripts::editor_roots(catalog, id, work)? {
            if !review.editors.contains(&editor) {
                review.editors.push(editor);
            }
        }
    }
    require(
        !assets.is_empty(),
        "missing_sources",
        "Este proyecto no tiene fuentes catalogadas; adopta/verifica sus raws antes de liberar",
    )?;
    for scope in raw_scopes {
        review.scopes.push(json!({"kind": "raw", "path": scope}));
        let (selected, kept) = match release::selected_files(&origin, &scope, false) {
            Ok(found) => found,
            Err(error)
                if nu9ve_volumes::fs::python_exception(&error) == Some("FileNotFoundError") =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        review.preserved.extend(kept);
        if !selected.is_empty() {
            let raw = release::raw_closeout_plan(
                home,
                backend,
                &json!({
                    "archive": archive["id"], "work": work["id"], "path": scope, "project": title,
                }),
            )?;
            review.entries.extend(
                raw["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(|mut entry| {
                        entry["category"] = json!("raw");
                        entry
                    }),
            );
            review.replacements.extend(release::replacements(&raw));
        }
    }
    let mut verified: BTreeMap<(String, String), Value> = review
        .entries
        .iter()
        .map(|entry| {
            (
                (
                    text(entry, "archive_path").to_owned(),
                    text(entry, "sha256").to_owned(),
                ),
                Value::Null,
            )
        })
        .collect();
    for (asset, job) in assets {
        require(
            !matches!(
                text(&job, "state"),
                "copying" | "cleaning_source" | "paused" | "failed" | "backup_verified"
            ),
            "open_job",
            "Hay una operación de fuentes pendiente",
        )?;
        for original in asset["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let copy = catalog
                .json_rows(
                    "SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?",
                    (text(asset, "job_id"), text(archive, "id"), original),
                )?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    Error::new(
                        "missing_receipt",
                        "Falta respaldo registrado de una fuente del proyecto",
                    )
                })?;
            let path = text(&copy, "destination");
            let sha = text(&copy, "sha256");
            let stamp = match verified.entry((path.to_owned(), sha.to_owned())) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let (_, stamp) = checked_hash(&target, path)?
                        .filter(|(found, _)| found == sha)
                        .ok_or_else(|| Error::new("checksum_mismatch", "Respaldo raw alterado"))?;
                    entry.insert(stamp)
                }
            };
            let mut final_ = json!({"archive_path": path, "sha256": sha, "role": "source"});
            if !stamp.is_null() {
                final_["archive_stamp"] = stamp.clone();
            }
            review.finals.push(final_);
        }
    }
    for editor in &review.editors {
        let renders = format!("{editor}/renders");
        review
            .scopes
            .push(json!({"kind": "discard", "path": renders}));
        for path in editorial::folder_files(&origin, &renders)? {
            let (sha, stamp) = checked_hash(&origin, &path)?.ok_or_else(|| {
                Error::new(
                    "source_changed",
                    format!("Release source is missing: {path}"),
                )
            })?;
            review
                .entries
                .push(json!({"path": path, "stamp": stamp, "sha256": sha,
                "archive_path": null, "category": "intermediate"}));
            review
                .replacements
                .insert(work_root.join(&path).to_string_lossy().into_owned(), None);
        }
        if work_root.join(editor).is_dir() {
            review.retained.push(format!(
                "{editor} (editable, Final Cut y contenido fuera de renders se conservan)"
            ));
        }
    }
    Ok(review)
}

pub(crate) fn inventory(tree: &Root, plan: &Value) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let mut found = BTreeSet::new();
    let mut preserved = BTreeSet::new();
    for scope in plan["closure_scopes"].as_array().into_iter().flatten() {
        let path = text(scope, "path");
        if scope["kind"] == "raw" {
            match release::selected_files(tree, path, false) {
                Ok((paths, kept)) => {
                    found.extend(paths);
                    preserved.extend(kept);
                }
                Err(error)
                    if nu9ve_volumes::fs::python_exception(&error) == Some("FileNotFoundError") => {
                }
                Err(error) => return Err(error),
            }
        } else {
            found.extend(editorial::folder_files(tree, path)?);
        }
    }
    for entry in plan["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["category"] == "final")
    {
        let path = text(entry, "path");
        if tree.stat(path)?.is_some() {
            found.insert(path.to_owned());
        }
    }
    Ok((found, preserved))
}

pub fn apply(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    apply_with(home, backend, params, || Guard::acquire(home))
}

fn apply_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    acquire: impl FnOnce() -> Result<Guard>,
) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let mut plan = release::stored_plan(&catalog, params)?;
    let id = text(&plan, "id").to_owned();
    let project = text(&plan, "closure_project_id").to_owned();
    require(
        !project.is_empty(),
        "invalid_plan",
        "No es un plan de cierre",
    )?;
    validate_shape(&plan)?;
    if plan["state"] == "completed" {
        editorial::cleanup(home, &mut plan)?;
        plan["closure_settings"] = closure_settings(&catalog, &project)?;
        return Ok(plan);
    }
    require(
        params["confirm_discard"] == true,
        "confirmation_required",
        "Confirma el plan revisado antes de completar",
    )?;
    let guard = if plan["entries"]
        .as_array()
        .is_some_and(|entries| !entries.is_empty())
    {
        Some(acquire()?)
    } else {
        None
    };
    let progress_catalog = WritableCatalog::open(home)?;
    let mut progress = ActionProgress::closure(&progress_catalog, Some(&id), "revalidating_source");
    progress.report()?;
    {
        let _lock = writer_lock(home)?;
        check_decisions(&Catalog::open(home)?, &plan)?;
        WritableCatalog::open(home)?.set(
            &format!("closure-consent:{id}"),
            &json!({"digest": plan["digest"], "confirmed_at": nu9ve_time::now_utc()}),
        )?;
    }
    release::check_source(backend, &plan)?;
    let origin = Root::open(Path::new(text(&plan, "source_root")))?;
    release::validate_remaining(&origin, &plan)?;
    let paths = release::replacements(&plan);
    let inspect = |paths: &[(String, Option<String>)]| match &guard {
        Some(guard) => guard.inspect(paths),
        None => Ok(json!({"blockers": [], "relinks": [], "dependencies": null})),
    };
    let consumers = inspect(&paths)?;
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
    for copy in plan["closure_copies"].as_array().into_iter().flatten() {
        progress.stage = "copying_finals";
        progress.report()?;
        let _lock = writer_lock(home)?;
        check_decisions(&Catalog::open(home)?, &plan)?;
        let catalog = WritableCatalog::open(home)?;
        crate::transfer::Transfer::new(home, &catalog, backend, &|_, _| Ok(()))
            .apply(copy, text(copy, "digest"))?;
    }
    if truthy(&consumers["relinks"])
        || catalog
            .get(&format!("release-relink-intent:{id}"))?
            .as_ref()
            .is_some_and(truthy)
    {
        progress.stage = "relinking";
        progress.report()?;
        let guard = guard
            .as_ref()
            .ok_or_else(|| Error::new("invalid_plan", "Missing Clipper exclusion"))?;
        release::relink_with(home, backend, params, guard)?;
    }
    if truthy(&plan["transcripts"]["files"]) {
        progress.stage = "copying_transcripts";
        progress.report()?;
        let result = transcripts::copy(home, backend, &plan["transcripts"], &id, None)?;
        let _lock = writer_lock(home)?;
        let mut stored = release::stored_plan(&catalog, params)?;
        if stored["transcripts_result"].is_null() {
            stored["transcripts_result"] = result;
            WritableCatalog::open(home)?.set(&format!("release:{id}"), &stored)?;
        }
    }
    drop(progress);
    let mut result = release::apply_reviewed(home, backend, params, inspect, &|_, _| Ok(()), true)?;
    drop(guard);
    editorial::cleanup(home, &mut result)?;
    result["closure_settings"] = closure_settings(&Catalog::open(home)?, &project)?;
    Ok(result)
}

pub(crate) fn validate_shape(plan: &Value) -> Result<()> {
    require(
        !plan["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|entry| entry["origin"] == "clipper_cache"),
        "plan_changed",
        "Este plan es anterior a la lista de regenerables de Clipper; revisa un plan nuevo",
    )?;
    require(
        plan["closure_inventory_checked"] == true,
        "plan_changed",
        "Revisa un plan nuevo antes de completar el cierre",
    )?;
    let full = plan["closure_finish"] == true;
    let scopes = plan["closure_scopes"].as_array();
    let entries = plan["entries"].as_array();
    let finals = plan["closure_finals"].as_array();
    let in_scope = |entry: &Value, kind: &str| {
        scopes.into_iter().flatten().any(|scope| {
            scope["kind"] == kind
                && text(entry, "path").starts_with(&format!("{}/", text(scope, "path")))
        })
    };
    require(
        plan["closure_finish"].is_boolean()
            && plan["preserved"]
                .as_array()
                .is_some_and(|p| full || p.is_empty())
            && plan["closure_group"]
                .as_array()
                .is_some_and(|group| !group.is_empty())
            && plan["closure_copies"].is_array()
            && scopes.is_some_and(|scopes| {
                (full || scopes.is_empty())
                    && scopes.iter().all(|scope| {
                        matches!(scope["kind"].as_str(), Some("raw" | "discard"))
                            && crate::rules::relative(text(scope, "path")).is_ok()
                            && text(scope, "path").starts_with(&format!("{}/", text(plan, "scope")))
                    })
            })
            && entries.is_some_and(|entries| {
                (full || !entries.is_empty())
                    && entries.iter().all(|entry| match text(entry, "category") {
                        "raw" => {
                            full && in_scope(entry, "raw") && entry["archive_path"].is_string()
                        }
                        "intermediate" => {
                            full && in_scope(entry, "discard")
                                && entry["archive_path"].is_null()
                                && entry["origin"].is_null()
                        }
                        "final" => finals.is_some_and(|finals| {
                            finals.iter().any(|final_| {
                                final_["keep_work"] == false
                                    && final_["original"]["volume_id"] == plan["work_id"]
                                    && final_["original"]["path"] == entry["path"]
                                    && final_["original_stamp"] == entry["stamp"]
                                    && final_["archive_path"] == entry["archive_path"]
                                    && final_["sha256"] == entry["sha256"]
                            })
                        }),
                        _ => false,
                    })
            })
            && finals.is_some()
            && (full || !truthy(&plan["clipper_regenerable"]) && !truthy(&plan["transcripts"])),
        "invalid_plan",
        "El plan no contiene las asociaciones y ámbitos revisados",
    )
}

pub(crate) fn verify_finals(
    target: &Root,
    origin: &Root,
    plan: &Value,
    transcripts_ready: bool,
) -> Result<Vec<Value>> {
    let mut guards = Vec::new();
    let mut verified = BTreeMap::new();
    let raw_entries = plan["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["category"] == "raw");
    let transcript_entries = plan["transcripts"]["entries"]
        .as_array()
        .filter(|_| transcripts_ready)
        .into_iter()
        .flatten();
    for final_ in plan["closure_finals"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(raw_entries)
        .chain(transcript_entries)
    {
        if truthy(&final_["keep_work"]) {
            require(
                origin
                    .stat(text(&final_["original"], "path"))?
                    .is_some_and(|stamp| stamp_json(&stamp) == final_["original_stamp"]),
                "source_changed",
                "Una entrega conservada cambió; revisa un plan nuevo",
            )?;
        }
        let path = final_["archive_path"]
            .as_str()
            .unwrap_or_else(|| text(final_, "target"));
        let sha = text(final_, "sha256");
        let stamp = match verified.entry((path, sha)) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let (_, stamp) = checked_hash(target, path)?
                    .filter(|(found, _)| found == sha)
                    .ok_or_else(|| {
                        Error::new(
                            "checksum_mismatch",
                            format!("Final sin respaldo íntegro: {path}"),
                        )
                    })?;
                entry.insert(stamp)
            }
        };
        let mut guard = final_.clone();
        guard["archive_path"] = json!(path);
        guard["stamp"] = stamp.clone();
        guards.push(guard);
    }
    Ok(guards)
}

pub(crate) fn watch_finals(target: &Root, guards: &mut [Value]) -> Result<()> {
    for guard in guards {
        let path = text(guard, "archive_path");
        if target
            .stat(path)?
            .is_some_and(|stamp| stamp_json(&stamp) == guard["stamp"])
        {
            continue;
        }
        let (_, stamp) = checked_hash(target, path)?
            .filter(|(sha, _)| guard["sha256"] == *sha)
            .ok_or_else(|| {
                Error::new("checksum_mismatch", "Project backup changed during release")
            })?;
        guard["stamp"] = stamp;
    }
    Ok(())
}

pub(crate) fn save_final_locations(
    reader: &Catalog,
    writer: &WritableCatalog,
    plan: &Value,
    guards: &[Value],
) -> Result<()> {
    let replacements: BTreeMap<_, _> = guards
        .iter()
        .filter(|guard| guard["original"].is_object() && !truthy(&guard["keep_work"]))
        .map(|guard| {
            (
                (
                    text(&guard["original"], "volume_id"),
                    text(&guard["original"], "path"),
                ),
                guard,
            )
        })
        .collect();
    for id in plan["closure_group"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let mut config = closure_settings(reader, id)?;
        let mut seen = BTreeSet::new();
        let mut finals = Vec::new();
        for mut item in config["finals"].as_array().cloned().unwrap_or_default() {
            let guard = replacements
                .get(&(text(&item, "volume_id"), text(&item, "path")))
                .copied();
            if let Some(guard) = guard {
                item["volume_id"] = plan["archive_id"].clone();
                item["path"] = guard["archive_path"].clone();
                item["stamp"] = guard["stamp"].clone();
            }
            if seen.insert((
                text(&item, "volume_id").to_owned(),
                text(&item, "path").to_owned(),
            )) {
                finals.push(item);
            }
        }
        config["finals"] = json!(finals);
        config["updated_at"] = json!(nu9ve_time::now_utc());
        writer.set(&format!("project-closure:{id}"), &config)?;
        if plan["closure_finish"] == true {
            let now = nu9ve_time::now_utc();
            writer.set(
                &format!("project-state:{id}"),
                &json!({"state": "finished", "updated_at": now}),
            )?;
            writer.set(
                &format!("project-released:{id}"),
                &json!({
                    "plan_id": plan["id"], "bytes": plan["bytes"], "updated_at": now,
                }),
            )?;
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::volumes::test_backend::{Setup, setup};
    use std::os::unix::fs::PermissionsExt;

    const PUBLISHED: &str = "002_finales/general/intro/main.mov";
    const UNPUBLISHED: &str = "002_finales/general/intro/preview.mov";

    fn fixture() -> Setup {
        let s = setup();
        for (root, path, bytes) in [
            (&s.work, PUBLISHED, b"published".as_slice()),
            (&s.work, UNPUBLISHED, b"preview".as_slice()),
        ] {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, bytes).unwrap();
        }
        let catalog = WritableCatalog::open(&s.home).unwrap();
        catalog
            .db()
            .execute("INSERT INTO projects VALUES('p1','general','intro')", [])
            .unwrap();
        crate::settings::closure_save(&s.home, Some(&s.backend), &json!({
            "project_id": "p1", "required": ["horizontal"], "no_more": true, "published": false,
            "finals": [{"volume_id": "ESSD", "path": PUBLISHED, "role": "horizontal", "published": true},
                       {"volume_id": "ESSD", "path": UNPUBLISHED, "role": "auxiliary", "published": false}],
        })).unwrap();
        let clipper = s.home.join("clipper");
        std::fs::write(&clipper, r#"#!/bin/sh
printf '%s\n' '{"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],"offline_projects":[],"projects":[]}'
"#).unwrap();
        std::fs::set_permissions(&clipper, std::fs::Permissions::from_mode(0o700)).unwrap();
        catalog.set("clipper-executable", &json!(clipper)).unwrap();
        s
    }

    fn review(s: &Setup) -> Value {
        let review_id = "11111111-1111-4111-8111-111111111111";
        let plan = plan_with(
            &s.home,
            &s.backend,
            &json!({
                "project_id": "p1", "archive": "MEDIA", "work": "ESSD", "release_work": false,
                "review_id": review_id,
            }),
            |catalog, paths| {
                assert_eq!(paths.len(), 1);
                let progress = catalog
                    .get(&format!("closure-progress:{review_id}"))?
                    .unwrap();
                assert_eq!(progress["stage"], "inspecting_clipper");
                Ok(json!({"blockers": [], "relinks": [], "dependencies": null}))
            },
        )
        .unwrap();
        assert!(
            Catalog::open(&s.home)
                .unwrap()
                .get(&format!("closure-progress:{review_id}"))
                .unwrap()
                .is_none()
        );
        plan
    }

    fn run(s: &Setup, args: &Value) -> Result<Value> {
        apply_with(&s.home, &s.backend, args, || {
            Guard::acquire_at(&s.home, s.home.join("home"))
        })
    }

    const RAW: &str = "001_proyectos/general_raw/intro/clip.mov";
    const ARCHIVE_RAW: &str = "001_raw/2026/11/clip.mov";
    const RENDER: &str = "001_proyectos/general/intro/renders/cache.mov";
    const CLIPPER: &str = "001_proyectos/general/intro/clipper";

    fn put(root: &Path, rel: &str, data: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    }

    fn editorial_fixture() -> Setup {
        let s = fixture();
        let transcript_rel = format!("{CLIPPER}/transcripts/clip.srt");
        for (root, rel, body) in [
            (&s.work, RAW, b"raw".as_slice()),
            (&s.archive, ARCHIVE_RAW, b"raw"),
            (&s.archive, PUBLISHED, b"published"),
            (&s.archive, UNPUBLISHED, b"preview"),
            (&s.work, RENDER, b"render"),
            (
                &s.work,
                "001_proyectos/general/intro/renders/Keep.fcpbundle/project.json",
                b"editable",
            ),
            (&s.work, &format!("{CLIPPER}/clipper-project.json"), b"{}"),
            (&s.work, &transcript_rel, b"transcript"),
        ] {
            put(root, rel, body);
        }
        let writer = WritableCatalog::open(&s.home).unwrap();
        let db = writer.db();
        let sha = checked_hash(&Root::open(&s.archive).unwrap(), ARCHIVE_RAW)
            .unwrap()
            .unwrap()
            .0;
        db.execute(
            "INSERT INTO project_aliases VALUES('general','intro','p1')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO jobs VALUES('j1','digest','{\"files\":[]}', 'completed',NULL,'t','t')",
            [],
        )
        .unwrap();
        let asset = json!({"id":"a1","kind":"file","job_id":"j1","project_id":"p1","profile":"general","paths":["clip.mov"]});
        db.execute(
            "INSERT INTO assets VALUES('a1','j1',?)",
            [asset.to_string()],
        )
        .unwrap();
        db.execute("INSERT INTO work VALUES('a1','ready','id-ESSD','t')", [])
            .unwrap();
        db.execute(
            "INSERT INTO members(job_id,relative_path,sha256) VALUES('j1','clip.mov',?)",
            [&sha],
        )
        .unwrap();
        for (id, dest) in [("id-MEDIA", ARCHIVE_RAW), ("id-ESSD", RAW)] {
            db.execute(
                "INSERT INTO copies VALUES('j1',?,'clip.mov',?,?)",
                (
                    id,
                    &sha,
                    json!({"path":"clip.mov","destination":dest,"sha256":sha}).to_string(),
                ),
            )
            .unwrap();
        }
        let project_root = s.work.join(CLIPPER);
        let digest = "d".repeat(64);
        let cleanup = json!({"schema":"clipper-regenerable-files/1","project_root":project_root,
            "project_state":"active","inventory_complete":true,"digest":digest,"entries":[],
            "categories":[{"id":"logs","default_selected":true,"files":0,"bytes":0}]});
        let (transcript_sha, mut stamp) =
            checked_hash(&Root::open(&s.work).unwrap(), &transcript_rel)
                .unwrap()
                .unwrap();
        stamp["device"] = json!(1);
        let export = json!({"schema":"clipper-transcript-export/1","project_root":project_root,"digest":"a".repeat(64),
            "entries":[{"path":s.work.join(&transcript_rel),"format":"srt","sha256":transcript_sha,"bytes":stamp["size"],
                "stamp":stamp,"source":{"path":s.work.join(RAW),"asset_id":"a1","member_sha256":sha}}]});
        let ack = json!({"schema":"clipper-regenerable-cleanup/1","plan_digest":digest,"categories":["logs"],
            "deleted_files":0,"deleted_bytes":0,"skipped":[]});
        let deps = json!({"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],
            "offline_projects":[],"projects":[]});
        let clipper = s.home.join("clipper");
        std::fs::write(&clipper, format!(r#"#!/bin/sh
set -eu
case "$2" in
deps) printf '%s\n' '{deps}' ;;
cleanup-plan) cat <<'JSON'
{cleanup}
JSON
;;
transcripts-export) cat <<'JSON'
{export}
JSON
;;
cleanup)
[ -f '{}/retry-cleanup' ]
[ "$3" = --plan ] && [ -f "$4" ] && [ "$5" = --digest ] && [ "$6" = '{digest}' ] && [ "$7" = --category ] && [ "$8" = logs ]
printf '%s\n' '{ack}' ;;
*) exit 9 ;;
esac
"#, s.home.display())).unwrap();
        s
    }

    fn full_review(s: &Setup) -> Value {
        plan_with(
            &s.home,
            &s.backend,
            &json!({"project_id":"p1","archive":"MEDIA","work":"ESSD"}),
            |_, _| Ok(json!({"blockers":[],"relinks":[],"dependencies":null})),
        )
        .unwrap()
    }

    #[test]
    fn editorial_closeout_rechecks_scopes_and_transcripts_then_retries_clipper_cleanup() {
        let s = editorial_fixture();
        let plan = full_review(&s);
        assert_eq!(plan["closure_finish"], true);
        assert_eq!(plan["entries"].as_array().unwrap().len(), 3);
        assert_eq!(plan["transcripts"]["files"], 1);
        let args = json!({"plan_id":plan["id"],"digest":plan["digest"],"confirm_discard":true});
        let arrival = "001_proyectos/general/intro/renders/new.mov";
        put(&s.work, arrival, b"new");
        assert_eq!(run(&s, &args).unwrap_err().code, "source_changed");
        std::fs::remove_file(s.work.join(arrival)).unwrap();
        assert!(s.work.join(RAW).exists() && s.work.join(RENDER).exists());
        let first = run(&s, &args).unwrap();
        assert_eq!(first["state"], "completed");
        assert_eq!(first["transcripts_result"]["copied"], 1);
        let clipper = s.work.join(CLIPPER);
        let clipper = clipper.to_str().unwrap();
        assert!(first["clipper_cleaned"][clipper]["error"].is_string());
        assert!(
            !s.work.join(RAW).exists()
                && !s.work.join(RENDER).exists()
                && !s.work.join(PUBLISHED).exists()
        );
        assert!(s.work.join(UNPUBLISHED).exists());
        assert!(
            s.work
                .join("001_proyectos/general/intro/renders/Keep.fcpbundle/project.json")
                .exists()
        );
        assert!(
            s.work
                .join(format!("{CLIPPER}/transcripts/clip.srt"))
                .exists()
        );
        assert_eq!(
            std::fs::read(
                s.archive
                    .join("004_datos/clipper/transcripts/2026/11/clip/clip.srt")
            )
            .unwrap(),
            b"transcript"
        );
        let catalog = Catalog::open(&s.home).unwrap();
        assert_eq!(
            catalog.get("project-state:p1").unwrap().unwrap()["state"],
            "finished"
        );
        put(&s.home, "retry-cleanup", b"");
        let retried = run(&s, &args).unwrap();
        assert!(retried["clipper_cleaned"][clipper]["error"].is_null());
        assert_eq!(retried["clipper_cleaned"][clipper]["skipped_count"], 0);
    }

    #[test]
    fn editorial_closeout_stops_for_changed_transcript_or_archive_before_any_release() {
        let s = editorial_fixture();
        let plan = full_review(&s);
        let args = json!({"plan_id":plan["id"],"digest":plan["digest"],"confirm_discard":true});
        put(
            &s.work,
            &format!("{CLIPPER}/transcripts/clip.srt"),
            b"changed",
        );
        assert_eq!(run(&s, &args).unwrap_err().code, "source_changed");
        assert!(
            s.work.join(RAW).exists()
                && s.work.join(RENDER).exists()
                && s.work.join(PUBLISHED).exists()
        );
        let s = editorial_fixture();
        let plan = full_review(&s);
        let args = json!({"plan_id":plan["id"],"digest":plan["digest"],"confirm_discard":true});
        put(&s.archive, ARCHIVE_RAW, b"bad");
        assert_eq!(run(&s, &args).unwrap_err().code, "checksum_mismatch");
        assert!(
            s.work.join(RAW).exists()
                && s.work.join(RENDER).exists()
                && s.work.join(PUBLISHED).exists()
        );
    }

    #[test]
    fn editorial_closeout_finishes_linked_projects_and_reports_an_empty_repeat_review() {
        let s = editorial_fixture();
        std::fs::remove_dir_all(s.work.join(CLIPPER)).unwrap();
        let writer = WritableCatalog::open(&s.home).unwrap();
        writer
            .db()
            .execute("INSERT INTO projects VALUES('p2','general','linked')", [])
            .unwrap();
        crate::settings::closure_link(&s.home, &json!({"project_id":"p1","other_id":"p2"}))
            .unwrap();
        let render = "001_proyectos/general/linked/renders/linked.mov";
        put(&s.work, render, b"linked render");
        let plan = full_review(&s);
        assert_eq!(plan["closure_group"], json!(["p1", "p2"]));
        assert!(
            plan["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["path"] == render)
        );
        let args = json!({"plan_id":plan["id"],"digest":plan["digest"],"confirm_discard":true});
        assert_eq!(run(&s, &args).unwrap()["state"], "completed");
        assert!(!s.work.join(render).exists());
        assert_eq!(
            writer.get("project-state:p2").unwrap().unwrap()["state"],
            "finished"
        );
        let repeat = full_review(&s);
        assert_eq!(repeat["already_complete"], true);
        assert_eq!(repeat["entries"], json!([]));
        let args = json!({"plan_id":repeat["id"],"digest":repeat["digest"],"confirm_discard":true});
        let result = apply_with(&s.home, &s.backend, &args, || {
            panic!("no Clipper lock needed")
        })
        .unwrap();
        assert_eq!(result["state"], "completed");
    }

    #[test]
    fn published_closeout_copies_verifies_and_reassigns_only_the_reviewed_final() {
        let s = fixture();
        let plan = review(&s);
        assert_eq!(plan["closure_copies"].as_array().unwrap().len(), 1);
        let path = text(&plan["entries"][0], "archive_path");
        let mut args = json!({"plan_id": plan["id"], "digest": plan["digest"]});
        assert_eq!(run(&s, &args).unwrap_err().code, "confirmation_required");
        args["confirm_discard"] = json!(true);
        let done = run(&s, &args).unwrap();
        assert_eq!(done["state"], "completed");
        assert_eq!(
            done["closure_settings"]["finals"][0]["volume_id"],
            plan["archive_id"]
        );
        assert_eq!(done["closure_settings"]["finals"][0]["path"], path);
        assert_eq!(done["closure_settings"]["finals"][1]["path"], UNPUBLISHED);
        assert_eq!(std::fs::read(s.archive.join(path)).unwrap(), b"published");
        assert!(!s.work.join(PUBLISHED).exists());
        assert!(s.work.join(UNPUBLISHED).exists());
        assert_eq!(run(&s, &args).unwrap(), done);
        let catalog = Catalog::open(&s.home).unwrap();
        assert!(catalog.get("project-released:p1").unwrap().is_none());
        assert!(
            catalog
                .get(&format!("closure-progress:{}", text(&plan, "id")))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn published_closeout_matches_python() {
        let s = fixture();
        let path = s.archive.join(PUBLISHED);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"published").unwrap();
        let plan = review(&s);
        let args =
            json!({"plan_id": plan["id"], "digest": plan["digest"], "confirm_discard": true});
        let result = run(&s, &args).unwrap();
        matches_python(&s, "closeout", [(plan, "plan"), (result, "applied")]);
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn editorial_closeout_matches_python() {
        let s = editorial_fixture();
        std::fs::remove_dir_all(s.work.join(CLIPPER)).unwrap();
        let plan = full_review(&s);
        let id = text(&plan, "id");
        let args = json!({"plan_id": id, "digest": plan["digest"], "confirm_discard": true});
        let result = run(&s, &args).unwrap();
        let catalog = Catalog::open(&s.home).unwrap();
        let mut released = catalog.get("project-released:p1").unwrap().unwrap();
        released["plan_id"] = json!("<ID>");
        let work = catalog
            .db()
            .unwrap()
            .query_row("SELECT state FROM work WHERE asset_id='a1'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        let terminal = json!({"state": catalog.get("project-state:p1").unwrap(), "release": released, "work": work});
        matches_python(
            &s,
            "editorial_closeout",
            [(plan, "plan"), (result, "applied"), (terminal, "terminal")],
        );
    }

    fn matches_python<'a>(
        s: &Setup,
        case: &str,
        values: impl IntoIterator<Item = (Value, &'a str)>,
    ) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        fn visit(value: &mut Value) {
            match value {
                Value::Object(fields) => {
                    for key in [
                        "created_at",
                        "updated_at",
                        "closure_settings_digest",
                        "digest",
                        "metadata_backup",
                    ] {
                        fields.remove(key);
                    }
                    for (key, v) in fields {
                        if ["inode", "ctime_ns", "mtime_ns", "root_inode", "device"]
                            .contains(&key.as_str())
                        {
                            *v = json!(format!("<{}>", key.to_uppercase()));
                        } else {
                            visit(v);
                        }
                    }
                }
                Value::Array(items) => items.iter_mut().for_each(visit),
                _ => {}
            }
        }
        for (mut value, key) in values {
            value.as_object_mut().unwrap().remove("id");
            visit(&mut value);
            let root = s.archive.parent().unwrap().to_string_lossy();
            let normalized: Value = serde_json::from_str(
                &value
                    .to_string()
                    .replace(root.as_ref(), "<ROOT>")
                    .replace("id-MEDIA", "MEDIA")
                    .replace("id-ESSD", "ESSD"),
            )
            .unwrap();
            assert_eq!(normalized, golden[case][key], "{case}: {key}");
        }
    }

    #[test]
    fn published_closeout_recovers_interruption_after_unlink() {
        let s = fixture();
        let path = s.archive.join(PUBLISHED);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"published").unwrap();
        let plan = review(&s);
        let id = text(&plan, "id");
        let args = json!({"plan_id": id, "digest": plan["digest"], "confirm_discard": true});
        let writer = WritableCatalog::open(&s.home).unwrap();
        writer
            .set(
                &format!("closure-consent:{id}"),
                &json!({"digest": plan["digest"]}),
            )
            .unwrap();
        let interrupted = release::apply_reviewed(
            &s.home,
            &s.backend,
            &args,
            |_| Ok(json!({"blockers": [], "relinks": []})),
            &|stage, _| {
                require(
                    stage != "after_unlink",
                    "fixture_interrupted",
                    "Interrupted after unlink",
                )
            },
            true,
        )
        .unwrap_err();
        assert_eq!(interrupted.code, "fixture_interrupted");
        assert!(!s.work.join(PUBLISHED).exists());
        assert_eq!(
            writer.get(&format!("release:{id}")).unwrap().unwrap()["cleaned"][PUBLISHED],
            "intent"
        );
        let done = run(&s, &args).unwrap();
        assert_eq!(done["state"], "completed");
        assert_eq!(
            done["closure_settings"]["finals"][0]["volume_id"],
            plan["archive_id"]
        );
        assert_eq!(done["cleaned"][PUBLISHED], "done");
        assert!(s.work.join(UNPUBLISHED).exists());
    }

    #[test]
    fn published_closeout_stops_for_corruption_and_changed_decisions() {
        let s = fixture();
        let path = s.archive.join(PUBLISHED);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"published").unwrap();
        let plan = review(&s);
        assert!(plan["closure_copies"].as_array().unwrap().is_empty());
        let args =
            json!({"plan_id": plan["id"], "digest": plan["digest"], "confirm_discard": true});
        std::fs::write(&path, b"corrupted").unwrap();
        assert_eq!(run(&s, &args).unwrap_err().code, "checksum_mismatch");
        assert!(s.work.join(PUBLISHED).exists());
        let reader = Catalog::open(&s.home).unwrap();
        let mut settings = closure_settings(&reader, "p1").unwrap();
        settings["no_more"] = json!(false);
        WritableCatalog::open(&s.home)
            .unwrap()
            .set("project-closure:p1", &settings)
            .unwrap();
        assert_eq!(run(&s, &args).unwrap_err().code, "plan_changed");
        assert!(s.work.join(PUBLISHED).exists());
    }
}
