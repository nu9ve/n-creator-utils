//! Closeout of published deliveries, using the shared ingest and release journals.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::clipper_release::Guard;
use crate::delivery::ActionProgress;
use crate::planner::{build_plan, destination_path};
use crate::reads::{closure_settings, project_record, related};
use crate::release::{self, checked_hash};
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend, truthy};
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
    require(
        !release_work,
        "not_ported",
        "Full closeout with editorial scopes still uses Python",
    )?;
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
            if item
                .get("published")
                .map_or_else(|| truthy(&settings["published"]), truthy)
            {
                owned.push((id, item));
            }
        }
    }
    require(
        !owned.is_empty(),
        "project_incomplete",
        "Marca al menos un final como publicado",
    )?;
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
    let mut entries = Vec::new();
    let mut finals = Vec::new();
    let mut copies = Vec::new();
    let mut replacements = Vec::new();
    let mut claimed = BTreeMap::new();
    let mut originals = BTreeSet::new();
    let mut needed = 0u64;
    progress.meter.files_total = owned.len();
    progress.stage = "review_final";
    for (owner, item) in owned {
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
            require(
                originals.insert(rel),
                "overlapping_roots",
                "Hay carpetas superpuestas; revisa las asociaciones",
            )?;
            entries.push(json!({"path": rel, "stamp": stamp, "sha256": sha, "archive_path": dest, "category": "final"}));
            replacements.push((
                work_root.join(rel).to_string_lossy().into_owned(),
                Some(archive_root.join(&dest).to_string_lossy().into_owned()),
            ));
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
                role == item["role"],
                "destination_conflict",
                "La misma entrega no puede contar como dos tipos distintos",
            )?;
        }
        finals.push(json!({"archive_path": archive_path, "sha256": sha, "role": item["role"], "owner": owner,
            "original": {"volume_id": item["volume_id"], "path": rel}, "original_stamp": stamp,
            "copy_required": copy_required, "keep_work": false, "archive_stamp": archive_stamp}));
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
        !entries.is_empty(),
        "nothing_to_release",
        "Los finales publicados elegidos ya están en el respaldo",
    )?;
    progress.stage = "inspecting_clipper";
    progress.meter.current.clear();
    progress.report()?;
    let consumers = inspect(&catalog, &replacements)?;
    let replacements: Map<String, Value> = replacements
        .into_iter()
        .map(|(from, to)| (from, json!(to)))
        .collect();
    let bytes: u64 = entries
        .iter()
        .filter_map(|entry| entry["stamp"]["size"].as_u64())
        .sum();
    let id = uid()?;
    let mut value = json!({"schema": "nu9ve-release/1", "id": id, "created_at": nu9ve_time::now_utc(),
        "archive_id": archive["id"], "source_root": work["root"], "source_identity": source_info,
        "work_id": work["id"], "scope": work["prefix"], "project": title, "entries": entries, "preserved": [],
        "replacements": replacements, "replacement_checks": [], "bytes": bytes, "consumers": consumers,
        "state": "review", "cleaned": {}, "retained_editors": [], "closure_project_id": project_id,
        "closure_settings_digest": config_digest, "closure_scopes": [], "closure_finals": finals, "closure_copies": copies,
        "closure_finish": false, "closure_inventory_checked": true,
        "closure_group": group.iter().map(|(id, _)| id).collect::<Vec<_>>(), "closure_group_titles": titles,
        "clipper_regenerable": [], "clipper_files": 0, "clipper_bytes": 0, "clipper_cleaned": {},
        "transcripts": null, "already_complete": false});
    value["digest"] = json!(digest(&value));
    let _lock = writer_lock(home)?;
    check_decisions(&Catalog::open(home)?, &value)?;
    WritableCatalog::open(home)?.set(&format!("release:{id}"), &value)?;
    Ok(value)
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
    let plan = release::stored_plan(&catalog, params)?;
    let id = text(&plan, "id");
    let project = text(&plan, "closure_project_id");
    require(
        !project.is_empty(),
        "invalid_plan",
        "No es un plan de cierre",
    )?;
    validate_shape(&plan)?;
    if plan["state"] == "completed" {
        let mut result = plan.clone();
        result["closure_settings"] = closure_settings(&catalog, project)?;
        return Ok(result);
    }
    require(
        params["confirm_discard"] == true,
        "confirmation_required",
        "Confirma el plan revisado antes de completar",
    )?;
    let guard = acquire()?;
    let progress_catalog = WritableCatalog::open(home)?;
    let mut progress = ActionProgress::closure(&progress_catalog, Some(id), "revalidating_source");
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
        release::relink_with(home, backend, params, &guard)?;
    }
    drop(progress);
    let mut result = release::apply_reviewed(
        home,
        backend,
        params,
        |paths| guard.inspect(paths),
        &|_, _| Ok(()),
        true,
    )?;
    result["closure_settings"] = closure_settings(&Catalog::open(home)?, project)?;
    Ok(result)
}

pub(crate) fn validate_shape(plan: &Value) -> Result<()> {
    require(
        plan["closure_finish"] == false
            && !truthy(&plan["clipper_regenerable"])
            && !truthy(&plan["transcripts"])
            && plan["closure_scopes"].as_array().is_some_and(Vec::is_empty)
            && plan["entries"]
                .as_array()
                .is_some_and(|entries| entries.iter().all(|e| e["category"] == "final")),
        "not_ported",
        "Full closeout with editorial scopes still uses Python",
    )?;
    let entries = plan["entries"].as_array().unwrap();
    let finals = plan["closure_finals"].as_array();
    require(
        plan["closure_inventory_checked"] == true
            && plan["preserved"].as_array().is_some_and(Vec::is_empty)
            && plan["closure_group"]
                .as_array()
                .is_some_and(|group| !group.is_empty())
            && plan["closure_copies"].is_array()
            && !entries.is_empty()
            && finals.is_some_and(|finals| {
                entries.iter().all(|entry| {
                    finals.iter().any(|final_| {
                        final_["keep_work"] == false
                            && final_["original"]["volume_id"] == plan["work_id"]
                            && final_["original"]["path"] == entry["path"]
                            && final_["original_stamp"] == entry["stamp"]
                            && final_["archive_path"] == entry["archive_path"]
                            && final_["sha256"] == entry["sha256"]
                    })
                })
            }),
        "invalid_plan",
        "El plan no contiene las asociaciones revisadas de los finales",
    )
}

pub(crate) fn verify_finals(target: &Root, origin: &Root, plan: &Value) -> Result<Vec<Value>> {
    let mut guards = Vec::new();
    for final_ in plan["closure_finals"].as_array().into_iter().flatten() {
        if truthy(&final_["keep_work"]) {
            require(
                origin
                    .stat(text(&final_["original"], "path"))?
                    .is_some_and(|stamp| stamp_json(&stamp) == final_["original_stamp"]),
                "source_changed",
                "Una entrega conservada cambió; revisa un plan nuevo",
            )?;
        }
        let path = text(final_, "archive_path");
        let found = checked_hash(target, path)?
            .filter(|(sha, _)| final_["sha256"] == *sha)
            .ok_or_else(|| {
                Error::new(
                    "checksum_mismatch",
                    format!("Final sin respaldo íntegro: {path}"),
                )
            })?;
        let mut guard = final_.clone();
        guard["stamp"] = found.1;
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
        .filter(|guard| !truthy(&guard["keep_work"]))
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
        for (mut value, key) in [(plan, "plan"), (result, "applied")] {
            let archive_id = text(&value, "archive_id").to_owned();
            let work_id = text(&value, "work_id").to_owned();
            value.as_object_mut().unwrap().remove("id");
            visit(&mut value);
            let root = s.archive.parent().unwrap().to_string_lossy();
            let normalized: Value = serde_json::from_str(
                &value
                    .to_string()
                    .replace(root.as_ref(), "<ROOT>")
                    .replace(&archive_id, "MEDIA")
                    .replace(&work_id, "ESSD"),
            )
            .unwrap();
            assert_eq!(normalized, golden["closeout"][key], "{key}");
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
