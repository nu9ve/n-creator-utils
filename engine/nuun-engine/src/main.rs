//! `nuun-engine [--state-dir DIR] rpc-serve | rpc | suite-frames | --version`
//!
//! The same boundary as `python -S nu9ve.py media [--state-dir DIR] rpc-serve` (media/cli.py `serve_reads`):
//! one JSON request per line `{id, method, params}`, one line back `{id, ok: true, data}` or
//! `{id, ok: false, error: {code, message}}`; lines up to 1 MiB; only read methods. Python stays the oracle
//! until each method reaches parity (rust-engine.md §7): a read method that is not ported yet answers
//! `not_ported`, so a client can ask the Python engine (or the Windows maqueta) instead.

use nu9ve_engine::{EngineInfo, Request, RpcError, serve_lines};
use nu9ve_json::{Error, Result, error_envelope, ok_envelope, require};
use nuun_media::volumes::{Backend, platform_backend, platform_backend_name};
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

mod mcp;
mod suite;

/// media/cli.py `READ_METHODS` and `READ_WHEN_EMPTY`; tests/test_engine_boundary.py keeps them equal.
const READ_METHODS: [&str; 30] = [
    "project_preview",
    "closure_overview",
    "closure_candidates",
    "closure_deliveries",
    "closure_link_candidates",
    "closure_progress",
    "action_progress",
    "snapshot",
    "job",
    "jobs",
    "library",
    "volumes",
    "source_cards",
    "capture_libraries",
    "capture_overview",
    "profiles",
    "devices",
    "recipe",
    "living_collections",
    "missions_review",
    "missions_status",
    "projects",
    "release_plans",
    "integration_capabilities",
    "suggest",
    "asset_revisions",
    "storage_overview",
    "storage_progress",
    "storage_tree",
    "storage_collections",
];
/// Every method Python's `Service.call` answers (media/service.py `METHODS`; tests/test_engine_boundary.py keeps
/// them equal). A method in neither this list nor `NATIVE` is `unknown_method`, before any mode check (§7).
const ORACLE_METHODS: [&str; 93] = [
    "action_progress",
    "apply",
    "apply_release",
    "asset_revisions",
    "backup_metadata",
    "cancel_plan",
    "capture_discard_check",
    "capture_discarded",
    "capture_libraries",
    "capture_overview",
    "captures",
    "closure_apply",
    "closure_candidates",
    "closure_deliveries",
    "closure_link",
    "closure_link_candidates",
    "closure_overview",
    "closure_plan",
    "closure_progress",
    "closure_save",
    "collection_release_apply",
    "collection_release_plan",
    "collection_release_plans",
    "confirm_camera_cleanup",
    "confirm_device",
    "coverage",
    "defaults",
    "deliver_work",
    "devices",
    "discover",
    "dismiss_job",
    "dng_apply",
    "dng_plan",
    "eject_card",
    "finals_status",
    "handoff",
    "integration_capabilities",
    "integration_settings",
    "job",
    "jobs",
    "library",
    "living_apply",
    "living_collections",
    "living_plan",
    "missions_incorporate",
    "missions_review",
    "missions_status",
    "plan",
    "plan_document",
    "plan_finals",
    "plan_finals_document",
    "plan_release",
    "profiles",
    "project_preview",
    "projects",
    "publications",
    "publish_revision",
    "recipe",
    "reconcile_sony_cleanup",
    "recover_catalog",
    "register_volume",
    "relayout",
    "release_plans",
    "relink_release",
    "rename_project",
    "resolve",
    "restore",
    "resume",
    "retitle_capture",
    "retitle_operations",
    "review_release",
    "save_capture_tag",
    "save_capture_tags",
    "save_profile",
    "set_finals_folder",
    "set_project_state",
    "set_publication",
    "set_work_state",
    "snapshot",
    "source_cards",
    "sources",
    "storage_add_folder",
    "storage_collections",
    "storage_compare",
    "storage_overview",
    "storage_progress",
    "storage_remove_folder",
    "storage_scan",
    "storage_tree",
    "suggest",
    "touch_project",
    "transcripts_backup",
    "volumes",
];
/// Methods only this engine has (no oracle, so no shadow): `native` in `engine_info` when this platform answers them.
const NATIVE: [&str; 38] = [
    "project_keys",
    "project_key_set",
    "suite_profiles",
    "suite_profiles_register",
    "storage_accumulations",
    "storage_accumulations_scan",
    "storage_accumulations_settings",
    "suite_device",
    "suite_suggestions",
    "suggestions_dismiss",
    "conversations_settings",
    "conversations_status",
    "conversations_backup_plan",
    "conversations_backup_apply",
    "conversations_run",
    "conversations_export_working",
    "volume_apps",
    "work_copies",
    "work_copy_plan",
    "work_copy_apply",
    "suite_lease",
    "suite_register_volume",
    "suite_resolve_volume",
    "suite_observe_volume",
    "suite_publish_index",
    "suite_adopt_volume",
    "clipper_project_status",
    "clipper_restore_apply",
    "clipper_restore_plan",
    "missions_archived_apply",
    "missions_archived_review",
    "save_all_apply",
    "save_all_draft",
    "save_all_incorporate_missions",
    "save_all_replan_dji",
    "save_all_review",
    "save_all_save_draft",
    "save_all_work_plan",
];
const READ_WHEN_EMPTY: [&str; 2] = ["defaults", "integration_settings"];
const MAX_ONCE_BYTES: usize = 16 * 1024 * 1024;

/// Methods this engine answers on every platform. Everything else in READ_METHODS is `not_ported`.
const PORTABLE: [&str; 26] = [
    "recipe",
    "snapshot",
    "engine_info",
    "jobs",
    "job",
    "library",
    "projects",
    "profiles",
    "devices",
    "release_plans",
    "defaults",
    "integration_settings",
    "project_preview",
    "closure_overview",
    "closure_link_candidates",
    "closure_progress",
    "action_progress",
    "asset_revisions",
    "living_collections",
    "missions_status",
    "integration_capabilities",
    "capture_libraries",
    "storage_progress",
    "storage_tree",
    "missions_review",
    "storage_collections",
];
/// Reads that need this platform's volume backend (`platform_backend`: Windows in Rust; macOS still Python).
const VOLUME_READS: [&str; 7] = [
    "storage_overview",
    "closure_candidates",
    "suggest",
    "volumes",
    "source_cards",
    "capture_overview",
    "closure_deliveries",
];

/// Writes this engine performs, only through the one-shot `rpc` command (as Python: `rpc-serve` is read only).
/// All of them need the volume backend. `register_volume` changes configuration (the catalog), never media,
/// under the writer lock.
const VOLUME_WRITES: [&str; 18] = [
    "register_volume",
    "plan",
    "plan_document",
    "apply",
    "resume",
    "backup_metadata",
    "transcripts_backup",
    "storage_add_folder",
    "storage_remove_folder",
    "plan_release",
    "review_release",
    "plan_finals",
    "plan_finals_document",
    "recover_catalog",
    "restore",
    "deliver_work",
    "confirm_camera_cleanup",
    "reconcile_sony_cleanup",
];

/// The methods this engine answers on this platform: the portable ones, plus the volume reads with a backend.
fn ported() -> Vec<&'static str> {
    let mut methods = PORTABLE.to_vec();
    if platform_backend_name().is_some() {
        methods.extend(VOLUME_READS);
    }
    methods
}

/// The methods only this engine has that it answers on this platform (`rpc` only).
fn native(platform: Option<&dyn Backend>) -> Vec<&'static str> {
    let writes = ported_writes(platform);
    NATIVE
        .into_iter()
        .filter(|m| ONCE_READS.contains(m) || writes.contains(m))
        .collect()
}

/// The writes this engine performs on this platform (none without a volume backend).
/// Writes that change only the catalog, never media (E1): every platform, through `rpc` only, as Python.
const CATALOG_WRITES: [&str; 17] = [
    "defaults",
    "integration_settings",
    "dismiss_job",
    "touch_project",
    "set_project_state",
    "rename_project",
    "set_work_state",
    "set_publication",
    "save_capture_tag",
    "save_capture_tags",
    "set_finals_folder",
    "cancel_plan",
    "capture_discarded",
    "closure_link",
    "closure_save",
    "save_profile",
    "confirm_device",
];

fn ported_writes(backend: Option<&dyn Backend>) -> Vec<&'static str> {
    writes_for(
        backend.is_some(),
        backend.is_some_and(|b| b.writes()),
        cfg!(target_os = "macos"),
        cfg!(unix),
    )
}

fn writes_for(has_backend: bool, can_write: bool, macos: bool, unix: bool) -> Vec<&'static str> {
    let mut writes = CATALOG_WRITES.to_vec();
    writes.push("suite_device");
    // profiles.json (nu9ve-profile-v1): only nüün's own `nuun` entry, after the person confirms.
    writes.push("suite_profiles_register");
    // Accumulations: settings in the catalog, and each scan kept under <home>/accumulations/ (derived).
    writes.extend([
        "storage_accumulations_scan",
        "storage_accumulations_settings",
    ]);
    // Only the catalog: which conversations may leave, to which disk, and when (the person's decision).
    writes.push("conversations_settings");
    // «No volver a preguntar» for a disk and flow: only the catalog.
    writes.push("suggestions_dismiss");
    // nu9ve-data-v1 §2: the suite key the person chose for a project; only the catalog.
    writes.push("project_key_set");
    // «Trabajar aquí» through the suite agent; without it the answer says so and nothing changes.
    writes.push("suite_lease");
    if has_backend {
        writes.extend([
            "suite_register_volume",
            "suite_resolve_volume",
            "suite_observe_volume",
            "suite_publish_index",
        ]);
    }
    if has_backend {
        writes.push("coverage");
    }
    if can_write {
        writes.extend(VOLUME_WRITES);
        writes.extend([
            "save_all_apply",
            "save_all_save_draft",
            "save_all_replan_dji",
        ]);
        writes.push("clipper_restore_apply");
        // Copies between suite work disks (docs/contracts/suite-work-copy-v1.md).
        writes.extend(["work_copy_plan", "work_copy_apply"]);
        writes.push("missions_archived_apply");
        if macos {
            writes.extend([
                "eject_card",
                "storage_scan",
                "storage_compare",
                "missions_incorporate",
                "save_all_incorporate_missions",
                "dng_plan",
                "dng_apply",
                "living_plan",
                "living_apply",
                "conversations_backup_plan",
                "conversations_backup_apply",
                "conversations_run",
                "conversations_export_working",
                "collection_release_plan",
                "collection_release_apply",
            ]);
        }
        if unix {
            writes.extend([
                "apply_release",
                "relink_release",
                "closure_plan",
                "closure_apply",
                "retitle_capture",
                "publish_revision",
                "relayout",
            ]);
        }
    }
    writes
}

/// This device's id for a new per-device folder, or why there is none (the error travels inside, so a collection
/// already in the catalog still plans without it).
fn this_device(isolated: bool) -> Result<std::result::Result<String, Error>> {
    let device = nuun_media::suite_device::current(isolated, &json!({}))?;
    Ok(match device["state"].as_str() {
        Some("ok") => Ok(device["device"]["device_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned()),
        _ => Err(Error::new(
            "device_unavailable",
            device["message"]
                .as_str()
                .unwrap_or("A device identity is required for a new collection"),
        )),
    })
}

/// The person's home folder, where the agents keep their history (`HOME`, `USERPROFILE` on Windows).
fn user_home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| Error::new("internal_error", "No home folder"))
}

/// This platform's volume backend, or `not_ported` so the client can ask the Python engine instead.
fn backend(backend: Option<&dyn Backend>) -> Result<&dyn Backend> {
    backend.ok_or_else(|| {
        Error::new(
            "not_ported",
            "No volume backend in the Rust engine for this platform yet",
        )
    })
}

/// The backend for a write must support the operation.
fn write_backend(platform: Option<&dyn Backend>) -> Result<&dyn Backend> {
    let backend = backend(platform)?;
    require(
        backend.writes(),
        "not_ported",
        "This platform's volume backend does not support writes; ask Python",
    )?;
    Ok(backend)
}

/// Reads that Python answers only through the one-shot `rpc` (not in `READ_METHODS`): `discover` walks a source.
const ONCE_READS: [&str; 24] = [
    "project_keys",
    "suite_profiles",
    "storage_accumulations",
    "suite_adopt_volume",
    "suite_suggestions",
    "conversations_status",
    "volume_apps",
    "work_copies",
    "missions_archived_review",
    "clipper_restore_plan",
    "save_all_work_plan",
    "clipper_project_status",
    "save_all_review",
    "save_all_draft",
    "discover",
    "finals_status",
    "captures",
    "publications",
    "retitle_operations",
    "collection_release_plans",
    "resolve",
    "capture_discard_check",
    "sources",
    "handoff",
];

/// How a request arrived: a line of `rpc-serve` (reads only, 1 MiB) or the one-shot `rpc` (16 MiB, writes allowed).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Serve,
    Once,
}

struct Engine {
    state_dir: Option<PathBuf>,
}

impl Engine {
    fn home(&self) -> PathBuf {
        self.state_dir
            .clone()
            .unwrap_or_else(nuun_media::catalog::default_home)
    }

    /// QA: a `--state-dir` or the sandbox never write the person's shared suite files.
    fn isolated(&self) -> bool {
        self.state_dir.is_some()
            || std::env::var_os("NU9VE_SANDBOX").is_some()
            || std::env::var_os("NU9VE_MEDIA_HOME").is_some()
    }

    /// The catalog of `--state-dir`, or of the default home; read only, empty when it does not exist.
    fn catalog(&self) -> Result<nuun_media::catalog::Catalog> {
        nuun_media::catalog::Catalog::open(&self.home())
    }

    fn call(&self, method: &str, params: &Value) -> Result<Value> {
        let platform = platform_backend(&self.home());
        let platform = platform.as_deref();
        let isolated = self.isolated();
        match method {
            "suite_device" => nuun_media::suite_device::current(isolated, params),
            "project_keys" => nuun_media::project_keys::keys(&self.catalog()?),
            "project_key_set" => nuun_media::project_keys::set(&self.home(), params),
            "suite_profiles" => nuun_media::suite_profiles::overview(&self.home(), isolated),
            "suite_profiles_register" => {
                let device = this_device(isolated)?.ok();
                nuun_media::suite_profiles::register(
                    &self.home(),
                    isolated,
                    device.as_deref(),
                    params,
                )
            }
            "storage_accumulations" => nuun_media::accumulations::read(&self.home()),
            "storage_accumulations_scan" => {
                nuun_media::accumulations::scan(&self.home(), &user_home()?, isolated, params)
            }
            "storage_accumulations_settings" => {
                nuun_media::accumulations::settings(&self.home(), params)
            }
            "suite_lease" => suite::lease(isolated, params),
            "suite_register_volume" => nuun_media::suite_volume::register(
                &self.home(),
                backend(platform)?,
                isolated,
                params,
            ),
            "suite_resolve_volume" => nuun_media::suite_volume::resolve(
                &self.home(),
                backend(platform)?,
                isolated,
                params,
            ),
            "suite_observe_volume" => {
                nuun_media::suite_volume::observe(&self.home(), backend(platform)?, params)
            }
            "suite_publish_index" => {
                nuun_media::suite_volume::publish(&self.home(), backend(platform)?, params)
            }
            "suite_adopt_volume" => {
                nuun_media::suite_volume::adopt(&self.home(), backend(platform)?, params)
            }
            "volume_apps" => {
                nuun_media::work_copy::volume_apps(&self.home(), backend(platform)?, params)
            }
            "work_copies" => nuun_media::work_copy::list(&self.home()),
            "work_copy_plan" => nuun_media::work_copy::plan(
                &self.home(),
                write_backend(platform)?,
                isolated,
                params,
            ),
            "work_copy_apply" => {
                nuun_media::work_copy::apply(&self.home(), write_backend(platform)?, params)
            }
            "missions_archived_apply" => {
                nuun_media::missions::archived_apply(&self.home(), write_backend(platform)?, params)
            }
            "missions_archived_review" => {
                nuun_media::missions::archived_review(&self.catalog()?, backend(platform)?, params)
            }
            "clipper_restore_plan" => {
                nuun_media::clipper::restore_plan(&self.home(), backend(platform)?, params)
            }
            "clipper_restore_apply" => {
                nuun_media::clipper::restore_apply(&self.home(), write_backend(platform)?, params)
            }
            "save_all_replan_dji" => {
                nuun_media::save_all::replan_dji(&self.home(), write_backend(platform)?, params)
            }
            #[cfg(target_os = "macos")]
            "save_all_incorporate_missions" => nuun_media::mission_review::incorporate(
                &self.home(),
                write_backend(platform)?,
                params,
            ),
            "save_all_work_plan" => {
                nuun_media::save_all::plan_work(&self.home(), backend(platform)?, params)
            }
            "clipper_project_status" => {
                nuun_media::clipper::project_status(&self.catalog()?, backend(platform)?, params)
            }
            "save_all_draft" => {
                let mut draft = nuun_media::save_all::draft(&self.home())?;
                draft["collections"] = nuun_media::reads::living_collections(&self.catalog()?)?;
                Ok(draft)
            }
            "save_all_save_draft" => nuun_media::save_all::save_draft(&self.home(), params),
            "save_all_review" => {
                nuun_media::save_all::review(&self.home(), backend(platform)?, params)
            }
            "save_all_apply" => {
                nuun_media::save_all::apply(&self.home(), write_backend(platform)?, params)
            }
            // Python: summary(catalog): the recipe saved in the catalog (or the template) and the confirmed devices.
            "recipe" => {
                let catalog = self.catalog()?;
                let registry = catalog.get("device-registry")?;
                let confirmed = registry
                    .as_ref()
                    .and_then(Value::as_object)
                    .map_or(0, |r| r.len());
                let saved = catalog.get("recipe")?;
                Ok(nuun_recipe::summary(
                    &nuun_recipe::effective(saved.as_ref()),
                    confirmed,
                ))
            }
            "volumes" => nuun_media::volumes::volumes(&self.catalog()?, backend(platform)?),
            // Pieces this platform cannot read yet are named in `errors`, as Python reports a failing piece.
            "snapshot" => nuun_media::snapshot::snapshot(&self.catalog()?, platform, params),
            "source_cards" => {
                nuun_media::volumes::source_cards(&self.catalog()?, backend(platform)?, None)
            }
            // Cards only: units, bytes, evidence, category, camera, device and camera time (`mission` in `not_ported`).
            "discover" => nuun_media::scan::discover(Some(&self.catalog()?), platform, params),
            "finals_status" => {
                nuun_media::projects::finals_status(&self.catalog()?, backend(platform)?, params)
            }
            "captures" => nuun_media::captures::captures_method(&self.catalog()?, params),
            "capture_discard_check" => {
                nuun_media::captures::discard_check(&self.catalog()?, params)
            }
            "publications" => nuun_media::reads::publications(&self.catalog()?),
            "handoff" => nuun_media::handoff::handoff(&self.catalog()?, params),
            "sources" => nuun_media::sources::sources(&self.catalog()?, backend(platform)?, params),
            "retitle_operations" => nuun_media::reads::retitle_operations(&self.catalog()?),
            "collection_release_plans" => {
                nuun_media::reads::collection_release_plans(&self.catalog()?)
            }
            "resolve" => nuun_media::reads::resolve(&self.catalog()?, backend(platform)?, params),
            "register_volume" => {
                nuun_media::volumes::register_volume(&self.home(), write_backend(platform)?, params)
            }
            // A reviewed plan for a card (Python `build_plan`); `plan_document` is its canonical text, which the app
            // returns untouched to `apply`.
            "plan" | "plan_document" => {
                let plan = nuun_media::planner::build_plan(
                    &self.home(),
                    write_backend(platform)?,
                    params,
                )?;
                if method == "plan" {
                    Ok(plan)
                } else {
                    Ok(json!(
                        String::from_utf8(nu9ve_json::canonical(&plan)).unwrap_or_default()
                    ))
                }
            }
            // Copy, verify and (with `cleanup`) release the card for a reviewed card plan (Python `_apply_ingest`).
            "apply" => {
                let digest = params["digest"]
                    .as_str()
                    .ok_or_else(|| Error::new("internal_error", "'digest'"))?;
                let plan = params
                    .get("plan")
                    .ok_or_else(|| Error::new("internal_error", "'plan'"))?;
                nuun_media::transfer::apply_reviewed(
                    &self.home(),
                    write_backend(platform)?,
                    plan,
                    digest,
                    params["progress_id"].as_str(),
                )
            }
            "resume" => {
                let job = params["job_id"]
                    .as_str()
                    .ok_or_else(|| Error::new("internal_error", "'job_id'"))?;
                nuun_media::transfer::resume(
                    &self.home(),
                    write_backend(platform)?,
                    job,
                    params["progress_id"].as_str(),
                )
            }
            #[cfg(unix)]
            "retitle_capture" => {
                nuun_media::retitle::retitle(&self.home(), write_backend(platform)?, params)
            }
            #[cfg(unix)]
            "publish_revision" => {
                nuun_media::revisions::publish(&self.home(), write_backend(platform)?, params)
            }
            #[cfg(unix)]
            "relayout" => {
                nuun_media::relayout::apply(&self.home(), write_backend(platform)?, params)
            }
            "transcripts_backup" => {
                nuun_media::transcripts::backup(&self.home(), write_backend(platform)?, params)
            }
            "backup_metadata" => {
                let archive = params["archive"]
                    .as_str()
                    .ok_or_else(|| Error::new("internal_error", "'archive'"))?;
                nuun_media::units_map::backup_metadata(
                    &self.home(),
                    write_backend(platform)?,
                    archive,
                )
            }
            "jobs" => Ok(json!(nuun_media::reads::jobs(&self.catalog()?)?)),
            "job" => nuun_media::reads::job(&self.catalog()?, params),
            "library" => Ok(json!(nuun_media::reads::library(&self.catalog()?, params)?)),
            "projects" => Ok(json!(nuun_media::reads::projects(&self.catalog()?)?)),
            "profiles" => Ok(json!(nuun_media::snapshot::profiles(&self.catalog()?)?)),
            "devices" => nuun_media::reads::devices(&self.catalog()?),
            "release_plans" => Ok(json!(nuun_media::reads::release_plans(&self.catalog()?)?)),
            "project_preview" => nuun_media::reads::project_preview(params),
            "closure_overview" => Ok(json!(nuun_media::snapshot::project_closures(
                &self.catalog()?
            )?)),
            "closure_link_candidates" => {
                nuun_media::reads::closure_link_candidates(&self.catalog()?, params)
            }
            "closure_progress" => {
                nuun_media::action::progress_method(&self.catalog()?, params, Some("plan_id"))
            }
            "action_progress" => {
                nuun_media::action::progress_method(&self.catalog()?, params, None)
            }
            "asset_revisions" => nuun_media::reads::asset_revisions(&self.catalog()?, params),
            "living_collections" => nuun_media::reads::living_collections(&self.catalog()?),
            "missions_status" => nuun_media::reads::missions_status(&self.catalog()?, params),
            "integration_capabilities" => nuun_media::integrations::capabilities(&self.catalog()?),
            "suggest" => nuun_media::volumes::suggest(&self.catalog()?, backend(platform)?, params),
            "capture_overview" => {
                nuun_media::projects::capture_overview(&self.catalog()?, backend(platform)?, params)
            }
            "plan_finals" | "plan_finals_document" => {
                let plan = nuun_media::projects::plan_finals(
                    &self.home(),
                    write_backend(platform)?,
                    params,
                )?;
                if method == "plan_finals_document" {
                    Ok(json!(
                        String::from_utf8(nu9ve_json::canonical(&plan)).unwrap_or_default()
                    ))
                } else {
                    Ok(plan)
                }
            }
            "closure_plan" => {
                nuun_media::closeout::plan(&self.home(), write_backend(platform)?, params)
            }
            "closure_apply" => {
                nuun_media::closeout::apply(&self.home(), write_backend(platform)?, params)
            }
            "apply_release" => {
                nuun_media::release::apply_release(&self.home(), write_backend(platform)?, params)
            }
            "relink_release" => {
                nuun_media::release::relink_release(&self.home(), write_backend(platform)?, params)
            }
            "plan_release" => {
                nuun_media::release::plan_release(&self.home(), write_backend(platform)?, params)
            }
            "review_release" => {
                nuun_media::release::review_release(&self.home(), write_backend(platform)?, params)
            }
            "deliver_work" => {
                nuun_media::delivery::deliver(&self.home(), write_backend(platform)?, params)
            }
            "restore" => {
                nuun_media::restore::restore(&self.home(), write_backend(platform)?, params)
            }
            "recover_catalog" => {
                nuun_media::recovery::recover(&self.home(), write_backend(platform)?, params)
            }
            "confirm_camera_cleanup" => {
                nuun_media::sony::confirm(&self.home(), write_backend(platform)?, params)
            }
            "reconcile_sony_cleanup" => {
                nuun_media::sony::reconcile(&self.home(), write_backend(platform)?, params)
            }
            "eject_card" => {
                nuun_media::volumes::eject_card(&self.home(), write_backend(platform)?, params)
            }
            "coverage" => nuun_media::coverage::report(&self.home(), backend(platform)?, params),
            "storage_add_folder" => {
                nuun_media::storage::add_folder(&self.home(), write_backend(platform)?, params)
            }
            "storage_remove_folder" => nuun_media::storage::remove_folder(&self.home(), params),
            #[cfg(target_os = "macos")]
            "storage_scan" => {
                nuun_media::storage::scan(&self.home(), write_backend(platform)?, params)
            }
            #[cfg(target_os = "macos")]
            "storage_compare" => {
                nuun_media::storage::compare(&self.home(), write_backend(platform)?, params)
            }
            "storage_overview" => nuun_media::storage::overview(
                &self.catalog()?,
                &self.home(),
                backend(platform)?,
                Some(nuun_media::storage::THIS_MAC_ROOT),
            ),
            #[cfg(target_os = "macos")]
            "collection_release_plan" => {
                nuun_media::collection_release::plan(&self.home(), write_backend(platform)?, params)
            }
            #[cfg(target_os = "macos")]
            "collection_release_apply" => nuun_media::collection_release::apply(
                &self.home(),
                write_backend(platform)?,
                params,
            ),
            #[cfg(target_os = "macos")]
            "living_plan" => {
                // A new collection keeps one folder per device; the device is created if this machine has none.
                let params = nuun_media::living::device_folder(
                    &self.home(),
                    params,
                    this_device(isolated)?,
                )?;
                nuun_media::living::plan(&self.home(), write_backend(platform)?, &params)
            }
            "conversations_settings" => nuun_media::conversations::settings(&self.home(), params),
            "suite_suggestions" => nuun_media::suggestions::suggestions(
                &self.home(),
                backend(platform)?,
                &user_home()?,
                &if cfg!(target_os = "macos") {
                    nuun_media::conversations::device_of(isolated)
                } else {
                    Value::Null
                },
                params["conversations_only"] == true,
            ),
            "suggestions_dismiss" => nuun_media::suggestions::dismiss(&self.home(), params),
            "conversations_status" => {
                nuun_media::conversations::status(&self.home(), &user_home()?)
            }
            #[cfg(target_os = "macos")]
            "conversations_backup_plan" => nuun_media::conversations::backup_plan(
                &self.home(),
                write_backend(platform)?,
                &user_home()?,
                this_device(isolated)?,
                params,
            ),
            #[cfg(target_os = "macos")]
            "conversations_backup_apply" => nuun_media::conversations::backup_apply(
                &self.home(),
                write_backend(platform)?,
                isolated,
                &user_home()?,
                params,
            ),
            #[cfg(target_os = "macos")]
            "conversations_run" => nuun_media::conversations::run(
                &self.home(),
                write_backend(platform)?,
                isolated,
                &user_home()?,
                this_device(isolated)?,
                params,
            ),
            #[cfg(target_os = "macos")]
            "conversations_export_working" => nuun_media::conversations::export_working(
                &self.home(),
                write_backend(platform)?,
                isolated,
                &user_home()?,
                params,
            ),
            #[cfg(target_os = "macos")]
            "living_apply" => {
                nuun_media::living::apply(&self.home(), write_backend(platform)?, params)
            }
            #[cfg(target_os = "macos")]
            "dng_plan" => nuun_media::dng::plan(&self.home(), write_backend(platform)?, params),
            #[cfg(target_os = "macos")]
            "dng_apply" => nuun_media::dng::apply(&self.home(), write_backend(platform)?, params),
            #[cfg(target_os = "macos")]
            "missions_incorporate" => nuun_media::mission_review::incorporate(
                &self.home(),
                write_backend(platform)?,
                params,
            ),
            "missions_review" => {
                nuun_media::mission_review::review(&self.catalog()?, platform, params)
            }
            "storage_progress" => nuun_media::storage::progress_method(&self.home()),
            "storage_collections" => nuun_media::agent_storage::call(params),
            "storage_tree" => nuun_media::storage::tree(
                &self.catalog()?,
                &self.home(),
                params,
                Some(nuun_media::storage::THIS_MAC_ROOT),
            ),
            "closure_candidates" => nuun_media::projects::closure_candidates(
                &self.catalog()?,
                backend(platform)?,
                params,
            ),
            "closure_deliveries" => nuun_media::projects::closure_deliveries(
                &self.catalog()?,
                backend(platform)?,
                params,
            ),
            "capture_libraries" => {
                nuun_media::captures::capture_libraries(&self.catalog()?, params)
            }
            // Python answers these two without params only (`READ_WHEN_EMPTY`); with params they are writes.
            "defaults" if params.as_object().is_none_or(|p| p.is_empty()) => {
                nuun_media::snapshot::defaults(&self.catalog()?)
            }
            "integration_settings" if params.as_object().is_none_or(|p| p.is_empty()) => {
                nuun_media::snapshot::integration_settings(&self.catalog()?)
            }
            // With parameters these two are writes (`rpc` only), as in Python.
            "defaults" => nuun_media::settings::defaults(&self.home(), params),
            "integration_settings" => {
                nuun_media::settings::integration_settings(&self.home(), params)
            }
            "dismiss_job" => nuun_media::settings::dismiss_job(&self.home(), params),
            "touch_project" => nuun_media::settings::touch_project(&self.home(), params),
            "set_project_state" => nuun_media::settings::set_project_state(&self.home(), params),
            "rename_project" => nuun_media::settings::rename_project(&self.home(), params),
            "set_work_state" => nuun_media::settings::set_work_state(&self.home(), params),
            "set_publication" => nuun_media::settings::set_publication(&self.home(), params),
            "save_capture_tag" => {
                nuun_media::settings::save_capture_tags(&self.home(), params, true)
            }
            "save_capture_tags" => {
                nuun_media::settings::save_capture_tags(&self.home(), params, false)
            }
            "set_finals_folder" => nuun_media::settings::set_finals_folder(&self.home(), params),
            "cancel_plan" => nuun_media::settings::cancel_plan(&self.home(), params),
            "capture_discarded" => nuun_media::settings::capture_discarded(&self.home(), params),
            "closure_link" => nuun_media::settings::closure_link(&self.home(), params),
            "closure_save" => nuun_media::settings::closure_save(&self.home(), platform, params),
            "save_profile" => nuun_media::settings::save_profile(&self.home(), params),
            "confirm_device" => nuun_media::settings::confirm_device(&self.home(), params),
            _ => Err(Error::new(
                "unknown_method",
                format!("Unknown media service method: {method}"),
            )),
        }
    }

    fn info<'a>(
        &self,
        ported: &'a [&'a str],
        native: &'a [&'a str],
        platform: Option<&dyn Backend>,
    ) -> EngineInfo<'a> {
        // These extra fields describe the platform and one-shot operations.
        EngineInfo {
            protocol: "nuun-media/1",
            version: env!("CARGO_PKG_VERSION"),
            capabilities: &["action_progress_v1"],
            ported,
            native,
            extra: serde_json::Map::from_iter([
                ("writes".into(), json!(ported_writes(platform))),
                ("volume_backend".into(), json!(platform_backend_name())),
                ("once".into(), json!(ONCE_READS)),
                (
                    "state_dir".into(),
                    json!(self.state_dir.as_ref().map(|p| p.display().to_string())),
                ),
            ]),
        }
    }

    fn dispatch(&self, method: &str, params: &Value, mode: Mode) -> RpcResult<Value> {
        require(
            method == "engine_info" || ORACLE_METHODS.contains(&method) || NATIVE.contains(&method),
            "unknown_method",
            format!("Unknown media service method: {method}"),
        )?;
        let empty = params.as_object().is_some_and(|p| p.is_empty());
        if method == "engine_info" {
            require(
                empty,
                "invalid_params",
                "This method accepts an empty params object.",
            )?;
            let platform = platform_backend(&self.home());
            let platform = platform.as_deref();
            return Ok(self
                .info(&ported(), &native(platform), platform)
                .into_data());
        }
        let readable =
            READ_METHODS.contains(&method) || (READ_WHEN_EMPTY.contains(&method) && empty);
        if mode == Mode::Serve {
            require(
                readable,
                "read_only",
                format!("rpc-serve only answers reads: {method}"),
            )?;
        }
        if readable && !PORTABLE.contains(&method) && !VOLUME_READS.contains(&method) {
            return Err(RpcFailure::not_ported(
                "method_unavailable",
                format!("Method not ported to the Rust engine yet: {method}"),
            ));
        }
        if !readable
            && !ported_writes(platform_backend(&self.home()).as_deref()).contains(&method)
            && !ONCE_READS.contains(&method)
        {
            let reason = if writes_for(true, true, true, true).contains(&method) {
                "platform_unavailable"
            } else {
                "method_unavailable"
            };
            return Err(RpcFailure::not_ported(
                reason,
                format!("Write not ported to the Rust engine yet: {method}"),
            ));
        }
        // nu9ve-action-v1: every action this call opens is closed with its answer.
        Ok(nuun_media::action::within(|| self.call(method, params))?)
    }

    fn reply_once(&self, wire: &[u8]) -> Reply {
        if wire.len() > MAX_ONCE_BYTES {
            return Reply::invalid_request("request_too_large", "Request exceeds 16 MiB");
        }
        let mut id = RawValue::NULL;
        let result = (|| -> RpcResult<Value> {
            let text = std::str::from_utf8(wire)
                .map_err(|_| Error::new("invalid_request", "Expected valid UTF-8 JSON"))?;
            require(
                text.trim_start_matches([' ', '\t', '\n', '\r'])
                    .starts_with('{'),
                "invalid_request",
                "Expected an id/method/params JSON object",
            )?;
            let request: HashMap<String, &RawValue> = serde_json::from_str(text)
                .map_err(|e| Error::new("invalid_request", format!("Request is not JSON: {e}")))?;
            let raw_id = request.get("id").copied().unwrap_or(RawValue::NULL);
            let valid_id = match raw_id.get().as_bytes().first() {
                Some(b'"') => serde_json::from_str::<String>(raw_id.get()).is_ok(),
                Some(b'-' | b'0'..=b'9') => !raw_id.get().contains(['.', 'e', 'E']),
                _ => raw_id.get() == "null",
            };
            require(
                valid_id,
                "invalid_request",
                "id must be an integer, a string or null",
            )?;
            id = raw_id;
            let method: String = request
                .get("method")
                .and_then(|raw| serde_json::from_str(raw.get()).ok())
                .ok_or_else(|| Error::new("invalid_request", "method must be a string"))?;
            let params = match request.get("params") {
                None => json!({}),
                Some(raw) if raw.get().starts_with('{') => serde_json::from_str(raw.get())
                    .map_err(|e| {
                        Error::new("invalid_request", format!("Request is not JSON: {e}"))
                    })?,
                Some(_) => {
                    return Err(Error::new("invalid_params", "params must be an object").into());
                }
            };
            self.dispatch(&method, &params, Mode::Once)
        })();
        let envelope = match result {
            Ok(data) => ok_envelope(&Value::Null, data),
            Err(failure) => {
                let mut envelope = error_envelope(&Value::Null, &failure.error);
                if let Some(reason) = failure.reason {
                    envelope["error"]["reason"] = json!(reason);
                }
                envelope
            }
        };
        Reply {
            id: id.get().to_owned(),
            envelope,
        }
    }

    fn serve(&self, input: impl BufRead, output: impl Write) -> io::Result<()> {
        let platform = platform_backend(&self.home());
        let platform = platform.as_deref();
        let ported = ported();
        let native = native(platform);
        serve_lines(
            input,
            output,
            self.info(&ported, &native, platform),
            |request: Request<'_>| {
                self.dispatch(
                    request.method,
                    &Value::Object(request.params.clone()),
                    Mode::Serve,
                )
                .map(Into::into)
                .map_err(Into::into)
            },
        )
    }

    #[cfg(test)]
    fn answer(&self, line: &str, mode: Mode) -> Value {
        match mode {
            Mode::Once => serde_json::from_str(&self.reply_once(line.as_bytes()).line()).unwrap(),
            Mode::Serve => {
                let mut output = Vec::new();
                self.serve(line.as_bytes(), &mut output).unwrap();
                serde_json::from_slice(&output).unwrap()
            }
        }
    }
}

type RpcResult<T> = std::result::Result<T, RpcFailure>;

struct RpcFailure {
    error: Error,
    reason: Option<&'static str>,
}

impl RpcFailure {
    fn not_ported(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            error: Error::new("not_ported", message),
            reason: Some(reason),
        }
    }
}

impl From<Error> for RpcFailure {
    fn from(error: Error) -> Self {
        // Domain-level refusals are platform restrictions, except for closeout plans
        // submitted to the simple release methods.
        let reason = (error.code == "not_ported").then_some(
            if error.message == "Use the project closeout operation" {
                "closeout_plan"
            } else {
                "platform_unavailable"
            },
        );
        Self { error, reason }
    }
}

/// An answer: the envelope, and its `id` as the JSON text to write.
struct Reply {
    id: String,
    envelope: Value,
}

impl Reply {
    fn invalid_request(code: &str, message: &str) -> Self {
        Self {
            id: "null".to_owned(),
            envelope: error_envelope(&Value::Null, &Error::new(code, message)),
        }
    }

    /// `{"id":…,"ok":…,"data"|"error":…}` on one line, with the id exactly as it arrived.
    fn line(&self) -> String {
        let ok = self.envelope["ok"] == json!(true);
        let (key, payload) = if ok {
            ("data", &self.envelope["data"])
        } else {
            ("error", &self.envelope["error"])
        };
        format!(r#"{{"id":{},"ok":{ok},"{key}":{payload}}}"#, self.id)
    }
}

impl From<RpcFailure> for RpcError {
    fn from(failure: RpcFailure) -> Self {
        match failure.reason {
            Some(reason) => Self::not_ported(reason, failure.error.message),
            None => Self::new(&failure.error.code, failure.error.message),
        }
    }
}

/// F5 (b): the read orders other apps send to `nu9ve.py media` ([callers](../../docs/platforms/media-cli-callers.md)),
/// as the method and params Python's `media/cli.py` builds from them. `Err` is a usage error (Python's argparse
/// exits 2 for it).
fn media_order(args: &[String]) -> std::result::Result<(&'static str, Value), String> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    // `--name value` pairs after the positionals, each at most once.
    let options =
        |rest: &[&str], allowed: &[&str]| -> std::result::Result<Map<String, Value>, String> {
            let mut found = Map::new();
            let mut rest = rest.iter();
            while let Some(flag) = rest.next() {
                let name = flag
                    .strip_prefix("--")
                    .filter(|name| allowed.contains(name))
                    .ok_or_else(|| format!("unrecognized argument: {flag}"))?;
                let value = rest
                    .next()
                    .ok_or_else(|| format!("{flag} expects a value"))?;
                found.insert(name.to_owned(), json!(value));
            }
            Ok(found)
        };
    match words.as_slice() {
        ["projects"] => Ok(("projects", json!({}))),
        ["volumes", "list"] => Ok(("volumes", json!({}))),
        ["integration-capabilities"] => Ok(("integration_capabilities", json!({}))),
        ["retitle-operations"] => Ok(("retitle_operations", json!({}))),
        ["captures", root] if !root.starts_with("--") => Ok(("captures", json!({"root": root}))),
        ["sources", rest @ ..] => {
            let found = options(rest, &["project", "purpose"])?;
            let project = found
                .get("project")
                .ok_or("the following arguments are required: --project")?;
            let purpose = found.get("purpose").cloned().unwrap_or(json!("edit"));
            if purpose != "edit" && purpose != "archive" {
                return Err(format!("argument --purpose: invalid choice: {purpose}"));
            }
            Ok(("sources", json!({"project": project, "purpose": purpose})))
        }
        ["resolve", asset, rest @ ..] if !asset.starts_with("--") => {
            let found = options(rest, &["revision"])?;
            Ok((
                "resolve",
                json!({"asset_id": asset, "revision": found.get("revision")}),
            ))
        }
        _ => Err(format!(
            "unknown order: {}",
            if words.is_empty() {
                "(none)".to_owned()
            } else {
                words.join(" ")
            }
        )),
    }
}

/// `nuun-engine [--state-dir DIR] media [--state-dir DIR] <order> …`: Python's CLI envelope with the same values,
/// `{"ok": true, "command", "data"}` indented by 2, or `{"ok": false, "error"}` on one line with exit 1. Keys come
/// in this engine's order (sorted), not Python's; the callers parse the JSON.
fn media_cli(mut state_dir: Option<PathBuf>, args: &[String]) -> i32 {
    let mut rest = args;
    while let [flag, value, tail @ ..] = rest
        && flag == "--state-dir"
    {
        state_dir = Some(PathBuf::from(value));
        rest = tail;
    }
    // F5 (d): `media mcp`, the read-only MCP stdio server (mcp.rs).
    if rest == ["mcp"] {
        let engine = Engine { state_dir };
        return mcp::serve(&engine, io::stdin().lock(), io::stdout().lock());
    }
    let (method, params) = match media_order(rest) {
        Ok(order) => order,
        Err(usage) => {
            eprintln!(
                "usage: nuun-engine media [--state-dir DIR] projects | sources --project P [--purpose edit|archive] | volumes list | resolve ASSET [--revision R] | captures ROOT | integration-capabilities | retitle-operations | mcp\nnuun-engine media: error: {usage}"
            );
            return 2;
        }
    };
    let engine = Engine { state_dir };
    match engine.dispatch(method, &params, Mode::Once) {
        Ok(data) => {
            let envelope = json!({"ok": true, "command": method, "data": data});
            println!(
                "{}",
                serde_json::to_string_pretty(&envelope).unwrap_or_default()
            );
            0
        }
        Err(failure) => {
            let error = json!({"code": failure.error.code, "message": failure.error.message});
            println!("{}", json!({"ok": false, "error": error}));
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut state_dir = None;
    let mut command = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--state-dir" if i + 1 < args.len() => {
                state_dir = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--version" => {
                println!("nuun-engine {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "media" => std::process::exit(media_cli(state_dir, &args[i + 1..])),
            other => command = Some(other.to_owned()),
        }
        i += 1;
    }
    let engine = Engine { state_dir };
    let code = match command.as_deref() {
        Some("rpc-serve") => {
            // While the app reads, the suite finds nüün on its own socket (data_catalog, data_export).
            let served = suite::start(engine.home(), engine.isolated());
            let result = engine.serve(io::stdin().lock(), io::stdout().lock());
            // `exit` skips destructors: the record goes first, only if it is still this engine's.
            if let Some(served) = served {
                served.stop();
            }
            match result {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("rpc-serve: {error}");
                    1
                }
            }
        }
        // The suite methods in frames on stdio, for the core's example checks (`--server`).
        Some("suite-frames") => match suite::serve_stdio(&engine.home(), engine.isolated()) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("suite-frames: {error}");
                1
            }
        },
        Some("rpc") => {
            let mut raw = Vec::new();
            let reply = match io::stdin()
                .take((MAX_ONCE_BYTES + 1) as u64)
                .read_to_end(&mut raw)
            {
                Ok(_) => engine.reply_once(&raw),
                Err(error) => Reply::invalid_request("invalid_request", &error.to_string()),
            };
            println!("{}", reply.line());
            i32::from(reply.envelope["ok"] != json!(true))
        }
        _ => {
            eprintln!(
                "usage: nuun-engine [--state-dir DIR] rpc-serve | rpc | suite-frames | media … | --version"
            );
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state folder that does not exist: an empty catalog, and nothing is ever created (reads never write).
    fn engine() -> Engine {
        Engine {
            state_dir: Some(
                std::env::temp_dir()
                    .join(format!("nuun-engine-unit-{}-missing", std::process::id())),
            ),
        }
    }

    /// Phase L: every catalog read in the golden file, on Python's own fixture catalog, through `rpc-serve`.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn catalog_reads_match_python() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let reads = &golden["reads"];
        let home = tempfile::tempdir().unwrap();
        rusqlite::Connection::open(home.path().join("catalog.sqlite3"))
            .unwrap()
            .execute_batch(reads["sql"].as_str().unwrap())
            .unwrap();
        let e = Engine {
            state_dir: Some(home.path().to_path_buf()),
        };
        let cases = reads["cases"].as_array().unwrap();
        assert!(!cases.is_empty());
        for (index, case) in cases.iter().enumerate() {
            let line = json!({"id": index, "method": case["method"], "params": case["params"]})
                .to_string();
            // One-shot reads go through `rpc`, as Python answers them.
            let once = ONCE_READS.contains(&case["method"].as_str().unwrap_or_default());
            let reply = e.answer(&line, if once { Mode::Once } else { Mode::Serve });
            let expected = &case["answer"];
            let label = format!("{} {}", case["method"], case["params"]);
            if expected["ok"] == true {
                assert_eq!(reply["ok"], true, "{label}: {reply}");
                assert_eq!(reply["data"], expected["data"], "{label}");
            } else {
                assert_eq!(reply["ok"], false, "{label}: {reply}");
                assert_eq!(reply["error"]["code"], expected["code"], "{label}");
            }
        }
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn handoff_matches_python() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let case = &golden["handoff"];
        let home = tempfile::tempdir().unwrap();
        rusqlite::Connection::open(home.path().join("catalog.sqlite3"))
            .unwrap()
            .execute_batch(case["sql"].as_str().unwrap())
            .unwrap();
        let engine = Engine {
            state_dir: Some(home.path().to_path_buf()),
        };
        for (params, expected) in case["requests"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["answers"].as_array().unwrap())
        {
            let mut reply = engine.answer(
                &json!({"id": 1, "method": "handoff", "params": params}).to_string(),
                Mode::Once,
            );
            if let Some(data) = reply["data"].as_object_mut() {
                data.remove("handoff_id");
                data.remove("created_at");
                assert_eq!(reply["data"], expected["data"], "{params}");
            } else {
                assert_eq!(
                    reply["error"]["code"], expected["error"],
                    "{params}: {reply}"
                );
            }
        }
    }

    /// E1: the same base catalog and the same 43 writes as Python's, the same answers and the same catalog after.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn catalog_writes_match_python() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let case = &golden["writes"];
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap().join("state");
        std::fs::create_dir_all(&home).unwrap();
        let root = home.parent().unwrap().display().to_string();
        let db = rusqlite::Connection::open(home.join("catalog.sqlite3")).unwrap();
        db.execute_batch(&case["sql"].as_str().unwrap().replace("<ROOT>", &root))
            .unwrap();
        drop(db);
        let e = Engine {
            state_dir: Some(home.clone()),
        };
        let mut answers = Vec::new();
        for request in case["requests"].as_array().unwrap() {
            let line = json!({"method": request[0], "params": request[1]}).to_string();
            let reply = e.answer(&line, Mode::Once);
            answers.push(if reply["ok"] == true {
                json!({"ok": true, "data": reply["data"]})
            } else {
                json!({"ok": false, "code": reply["error"]["code"]})
            });
        }
        let db = rusqlite::Connection::open(home.join("catalog.sqlite3")).unwrap();
        let mut tables = serde_json::Map::new();
        for (name, order) in case["tables_order"].as_object().unwrap() {
            let sql = format!("SELECT * FROM {name} ORDER BY {}", order.as_str().unwrap());
            let mut statement = db.prepare(&sql).unwrap();
            let columns = statement.column_count();
            let rows: Vec<Value> = statement
                .query_map([], |row| {
                    let mut values = Vec::new();
                    for i in 0..columns {
                        values.push(match row.get_ref(i)? {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(n) => json!(n),
                            rusqlite::types::ValueRef::Real(x) => json!(x),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            rusqlite::types::ValueRef::Blob(b) => json!(String::from_utf8_lossy(b)),
                        });
                    }
                    Ok(Value::Array(values))
                })
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            tables.insert(name.clone(), Value::Array(rows));
        }
        let now = regex::Regex::new(case["now"].as_str().unwrap()).unwrap();
        let text = json!({"answers": answers, "tables": tables}).to_string();
        let text = now.replace_all(&text, "<NOW>").replace(&root, "<ROOT>");
        let mine: Value = serde_json::from_str(&text).unwrap();
        for (i, (a, b)) in mine["answers"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["answers"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(a, b, "write {i}: {}", case["requests"][i]);
        }
        for name in case["tables_order"].as_object().unwrap().keys() {
            assert_eq!(mine["tables"][name], case["tables"][name], "table {name}");
        }
    }

    #[test]
    fn empty_catalog_reads_answer_without_creating_anything() {
        let e = engine();
        let reply = e.answer(r#"{"id": 5, "method": "recipe"}"#, Mode::Serve);
        assert_eq!(reply["data"]["confirmed_devices"], 0);
        let volumes = e.answer(r#"{"id": 6, "method": "volumes"}"#, Mode::Serve);
        if platform_backend_name().is_some() {
            assert_eq!(volumes["data"], json!([]));
        } else {
            assert_eq!(volumes["error"]["code"], "not_ported");
        }
        assert!(!e.state_dir.as_ref().unwrap().exists());
    }

    #[test]
    fn recipe_answers_with_the_python_shape() {
        let reply = engine().answer(
            r#"{"id": 7, "method": "recipe", "params": {}}"#,
            Mode::Serve,
        );
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["data"]["cards"][1]["family"], "sony");
    }

    #[test]
    fn unported_reads_mutations_and_garbage_have_stable_codes() {
        let e = engine();
        assert_eq!(
            e.answer(r#"{"id": 1, "method": "storage_tree"}"#, Mode::Serve)["error"]["code"],
            "internal_error",
            "Python: KeyError on a missing target"
        );
        assert_eq!(
            e.answer(r#"{"id": 1, "method": "job"}"#, Mode::Serve)["error"]["code"],
            "internal_error",
            "Python: KeyError on a missing job_id"
        );
        assert_eq!(
            e.answer(r#"{"id": 2, "method": "apply", "params": {}}"#, Mode::Serve)["error"]["code"],
            "read_only"
        );
        assert_eq!(
            e.answer(
                r#"{"id": 3, "method": "defaults", "params": {"x": 1}}"#,
                Mode::Serve
            )["error"]["code"],
            "read_only"
        );
        assert_eq!(
            e.answer("not json", Mode::Serve)["error"]["code"],
            "invalid_request"
        );
        assert_eq!(
            e.answer(r#"{"id": 5, "method": "frobnicate"}"#, Mode::Once)["error"]["code"],
            "unknown_method"
        );
        assert_eq!(
            e.answer(r#"[1, 2]"#, Mode::Serve)["error"]["code"],
            "invalid_request"
        );
        let long = format!(
            r#"{{"id": 4, "method": "recipe", "params": {{"pad": "{}"}}}}"#,
            "x".repeat(nu9ve_engine::MAX_MESSAGE_BYTES)
        );
        assert_eq!(
            e.answer(&long, Mode::Serve)["error"]["code"],
            "request_too_large"
        );
    }

    #[test]
    fn not_ported_errors_explain_why() {
        let Err(error) = backend(None) else {
            panic!("expected a missing platform backend");
        };
        let failure = RpcFailure::from(error);
        assert_eq!(failure.error.code, "not_ported");
        assert_eq!(failure.reason, Some("platform_unavailable"));
    }

    #[test]
    fn discover_answers_only_through_the_one_shot_rpc() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("DCIM/100GOPRO")).unwrap();
        std::fs::write(temp.path().join("DCIM/100GOPRO/GX010001.MP4"), b"video").unwrap();
        let request = json!({"id": 3, "method": "discover", "params": {"source": temp.path().canonicalize().unwrap().display().to_string()}}).to_string();
        let e = engine();
        assert_eq!(
            e.answer(&request, Mode::Serve)["error"]["code"],
            "read_only"
        );
        let reply = e.answer(&request, Mode::Once);
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(reply["data"]["units"][0]["id"], "DCIM/100GOPRO/GX010001");
        assert_eq!(reply["data"]["units"][0]["category"], "video");
        assert_eq!(reply["data"]["bytes"], 5);
    }

    #[test]
    fn writes_go_only_through_the_one_shot_rpc() {
        let temp = tempfile::tempdir().unwrap();
        let e = Engine {
            state_dir: Some(temp.path().join("state")),
        };
        let target = temp.path().join("destino");
        std::fs::create_dir_all(&target).unwrap();
        let request = json!({"id": 9, "method": "register_volume",
            "params": {"name": "LOCAL", "role": "work", "root": target.display().to_string(), "allow_local": true}})
        .to_string();
        assert_eq!(
            e.answer(&request, Mode::Serve)["error"]["code"],
            "read_only"
        );
        assert!(
            !temp.path().join("state").exists(),
            "a refused write leaves nothing behind"
        );
        assert_eq!(
            e.answer(
                r#"{"id": 10, "method": "storage_scan", "params": {}}"#,
                Mode::Once
            )["error"]["code"],
            if cfg!(target_os = "macos") {
                "internal_error"
            } else {
                "not_ported"
            }
        );
        let reply = e.answer(&request, Mode::Once);
        let writes = platform_backend(&e.home()).is_some_and(|b| b.writes());
        match platform_backend_name().filter(|_| writes) {
            Some(mode) => {
                assert_eq!(reply["ok"], true, "{reply}");
                assert_eq!(reply["data"]["identity"]["mode"], mode);
                let listed = e.answer(r#"{"id": 11, "method": "volumes"}"#, Mode::Serve);
                assert_eq!(listed["data"][0]["name"], "LOCAL");
                assert_eq!(listed["data"][0]["online"], true);
            }
            // Without a volume backend that writes, the write is refused before anything is created.
            None => {
                assert_eq!(reply["error"]["code"], "not_ported");
                assert!(!temp.path().join("state").exists());
            }
        }
    }

    #[test]
    fn the_volume_backend_decides_what_is_ported() {
        let info =
            engine().answer(r#"{"id": 1, "method": "engine_info"}"#, Mode::Serve)["data"].clone();
        let has = platform_backend_name().is_some();
        assert_eq!(info["volume_backend"].is_string(), has);
        assert_eq!(platform_backend_name().is_some(), has);
        for method in VOLUME_READS {
            assert_eq!(
                info["ported"].as_array().unwrap().contains(&json!(method)),
                has,
                "{method}"
            );
        }
        let writes = platform_backend(&engine().home()).is_some_and(|b| b.writes());
        for method in VOLUME_WRITES {
            assert_eq!(
                info["writes"].as_array().unwrap().contains(&json!(method)),
                writes,
                "{method}"
            );
            // No backend that writes: every volume write answers `not_ported`, never `unknown_method`.
            if !writes {
                let request = json!({"id": 2, "method": method, "params": {}}).to_string();
                let reply = engine().answer(&request, Mode::Once);
                assert_eq!(reply["error"]["code"], "not_ported");
                assert_eq!(reply["error"]["reason"], "platform_unavailable");
            }
        }
        assert_eq!(cfg!(any(windows, target_os = "macos")), has);
        assert_eq!(cfg!(any(windows, target_os = "macos")), writes);
    }

    /// F5 (b): the orders of `media`, as Python's `media/cli.py` turns them into a method and its params.
    #[test]
    fn media_orders_become_pythons_methods() {
        let order =
            |words: &[&str]| media_order(&words.iter().map(|w| w.to_string()).collect::<Vec<_>>());
        assert_eq!(order(&["projects"]).unwrap(), ("projects", json!({})));
        assert_eq!(order(&["volumes", "list"]).unwrap(), ("volumes", json!({})));
        assert_eq!(
            order(&["sources", "--project", "terra"]).unwrap(),
            ("sources", json!({"project": "terra", "purpose": "edit"}))
        );
        assert_eq!(
            order(&["sources", "--purpose", "archive", "--project", "terra"]).unwrap(),
            ("sources", json!({"project": "terra", "purpose": "archive"}))
        );
        assert_eq!(
            order(&["resolve", "a1"]).unwrap(),
            ("resolve", json!({"asset_id": "a1", "revision": null}))
        );
        assert_eq!(
            order(&["resolve", "a1", "--revision", "r1"]).unwrap(),
            ("resolve", json!({"asset_id": "a1", "revision": "r1"}))
        );
        assert_eq!(
            order(&["captures", "/x"]).unwrap(),
            ("captures", json!({"root": "/x"}))
        );
        for bad in [
            &[][..],
            &["sources"],
            &["sources", "--project"],
            &["sources", "--project", "p", "--purpose", "x"],
            &["sources", "--project", "p", "--other", "y"],
            &["resolve"],
            &["volumes"],
            &["apply", "plan.json"],
        ] {
            assert!(order(bad).is_err(), "{bad:?}");
        }
    }

    /// nu9ve-action-v1 §6: the pinned `action_progress` requests, through `rpc-serve`, on an empty library.
    #[test]
    fn action_examples_v1() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/nu9ve-core/data/action-examples-v1.json");
        let examples: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let cases = examples["requests"].as_array().unwrap();
        assert!(!cases.is_empty());
        let mut input = Vec::new();
        for case in cases {
            writeln!(input, "{}", case["request"].as_str().unwrap()).unwrap();
        }
        let mut output = Vec::new();
        engine().serve(input.as_slice(), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.lines().count(), cases.len());
        for (line, case) in output.lines().zip(cases) {
            let name = case["name"].as_str().unwrap();
            let reply: Value = serde_json::from_str(line).unwrap();
            let request: Value = serde_json::from_str(case["request"].as_str().unwrap()).unwrap();
            assert_eq!(reply["id"], request["id"], "{name}: {line}");
            assert_eq!(reply["ok"], case["ok"], "{name}: {line}");
            if case["ok"] == json!(true) {
                assert_eq!(
                    reply["data"],
                    json!({"contract": "nu9ve-action/1", "state": "unknown",
                        "progress_id": request["params"]["progress_id"]}),
                    "{name}"
                );
            } else {
                assert_eq!(reply["error"]["code"], case["code"], "{name}: {line}");
            }
        }
    }

    /// nu9ve-engine-v1 §11: the 20 engine cases of the pinned examples, line by line as `rpc-serve` reads them.
    #[test]
    fn engine_examples_v1() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/nu9ve-core/data/engine-examples-v1.json");
        let examples: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let cases = examples["server"].as_array().unwrap();
        assert_eq!(cases.len(), 20);
        let mut input = Vec::new();
        let mut expected = Vec::new();
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let mut request = case["request"].as_str().unwrap().to_owned();
            if let Some(pad) = case["pad_bytes"].as_u64() {
                request = request.replace("<pad>", &"x".repeat(pad as usize));
            }
            writeln!(input, "{request}").unwrap();
            if !case["response"].is_null() {
                expected.push((name, &case["response"]));
            }
            if let Some(then) = case.get("then") {
                writeln!(input, "{}", then["request"].as_str().unwrap()).unwrap();
                expected.push((name, &then["response"]));
            }
        }
        let mut output = Vec::new();
        engine().serve(input.as_slice(), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.ends_with('\n'));
        assert_eq!(output.lines().count(), expected.len());
        for (line, (name, expected)) in output.lines().zip(expected) {
            let fields: HashMap<String, &RawValue> = serde_json::from_str(line).unwrap();
            assert_eq!(
                fields["id"].get(),
                expected["id"].as_str().unwrap(),
                "{name}: {line}"
            );
            let reply: Value = serde_json::from_str(line).unwrap();
            assert_eq!(reply["ok"], expected["ok"], "{name}: {line}");
            if expected["ok"] == json!(false) {
                assert_eq!(reply["error"]["code"], expected["code"], "{name}: {line}");
            }
        }
    }

    #[test]
    fn engine_info_follows_the_contract() {
        let info =
            engine().answer(r#"{"id": 1, "method": "engine_info"}"#, Mode::Serve)["data"].clone();
        assert_eq!(info["contract"], "nu9ve-engine/1");
        assert_eq!(info["protocol"], "nuun-media/1");
        assert_eq!(info["capabilities"], json!(["action_progress_v1"]));
        let ported = info["ported"].as_array().unwrap();
        for method in info["native"].as_array().unwrap() {
            assert!(!ported.contains(method), "{method} is ported and native");
            assert!(
                !ORACLE_METHODS.contains(&method.as_str().unwrap()),
                "{method}"
            );
        }
        for method in NATIVE {
            assert!(!ORACLE_METHODS.contains(&method), "{method}");
        }
    }

    #[test]
    fn every_ported_method_is_a_read() {
        for method in ported() {
            assert!(
                READ_METHODS.contains(&method)
                    || READ_WHEN_EMPTY.contains(&method)
                    || method == "engine_info",
                "{method}"
            );
        }
    }
}
