//! `nuun-engine [--state-dir DIR] rpc-serve | rpc | --version`
//!
//! The same boundary as `python -S nu9ve.py media [--state-dir DIR] rpc-serve` (media/cli.py `serve_reads`):
//! one JSON request per line `{id, method, params}`, one line back `{id, ok: true, data}` or
//! `{id, ok: false, error: {code, message}}`; lines up to 1 MiB; only read methods. Python stays the oracle
//! until each method reaches parity (rust-engine.md §7): a read method that is not ported yet answers
//! `not_ported`, so a client can ask the Python engine (or the Windows maqueta) instead.

use nu9ve_json::{Error, Result, error_envelope, ok_envelope, require};
use nuun_media::volumes::{Backend, platform_backend, platform_backend_name};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;

/// media/cli.py `READ_METHODS` and `READ_WHEN_EMPTY`; tests/test_engine_boundary.py keeps them equal.
const READ_METHODS: [&str; 29] = [
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
];
const READ_WHEN_EMPTY: [&str; 2] = ["defaults", "integration_settings"];
const MAX_LINE: usize = 1024 * 1024;

/// Methods this engine answers on every platform. Everything else in READ_METHODS is `not_ported`.
const PORTABLE: [&str; 25] = [
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
    let mut writes = CATALOG_WRITES.to_vec();
    if backend.is_some() {
        writes.push("coverage");
    }
    if backend.is_some_and(|b| b.writes()) {
        writes.extend(VOLUME_WRITES);
        if cfg!(target_os = "macos") {
            writes.extend([
                "eject_card",
                "storage_scan",
                "storage_compare",
                "missions_incorporate",
                "dng_plan",
                "dng_apply",
                "living_plan",
                "living_apply",
                "collection_release_plan",
                "collection_release_apply",
            ]);
        }
        if cfg!(unix) {
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

/// This platform's volume backend, or `not_ported` so the client can ask the Python engine instead.
fn backend(backend: Option<&dyn Backend>) -> Result<&dyn Backend> {
    backend.ok_or_else(|| {
        Error::new(
            "not_ported",
            "No volume backend in the Rust engine for this platform yet",
        )
    })
}

/// The backend for a write: only one whose writes are open (a new backend reads until its physical run).
fn write_backend(platform: Option<&dyn Backend>) -> Result<&dyn Backend> {
    let backend = backend(platform)?;
    require(
        backend.writes(),
        "not_ported",
        "This platform's volume backend reads only until its physical run (docs/release.md); ask Python",
    )?;
    Ok(backend)
}

/// Reads that Python answers only through the one-shot `rpc` (not in `READ_METHODS`): `discover` walks a source.
const ONCE_READS: [&str; 10] = [
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

    /// The catalog of `--state-dir`, or of the default home; read only, empty when it does not exist.
    fn catalog(&self) -> Result<nuun_media::catalog::Catalog> {
        nuun_media::catalog::Catalog::open(&self.home())
    }

    fn call(&self, method: &str, params: &Value) -> Result<Value> {
        let platform = platform_backend(&self.home());
        let platform = platform.as_deref();
        match method {
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
                    .ok_or_else(|| Error::new("operation_failed", "'digest'"))?;
                let plan = params
                    .get("plan")
                    .ok_or_else(|| Error::new("operation_failed", "'plan'"))?;
                nuun_media::transfer::apply_reviewed(
                    &self.home(),
                    write_backend(platform)?,
                    plan,
                    digest,
                )
            }
            "resume" => {
                let job = params["job_id"]
                    .as_str()
                    .ok_or_else(|| Error::new("operation_failed", "'job_id'"))?;
                nuun_media::transfer::resume(&self.home(), write_backend(platform)?, job)
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
                    .ok_or_else(|| Error::new("operation_failed", "'archive'"))?;
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
            "closure_progress" => nuun_media::reads::progress(
                &self.catalog()?,
                "closure-progress:",
                params,
                "plan_id",
            ),
            "action_progress" => {
                nuun_media::reads::progress(&self.catalog()?, "action-progress:", params, "id")
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
                nuun_media::living::plan(&self.home(), write_backend(platform)?, params)
            }
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
            // Not in Python: lets a client know which engine answered and what it can do.
            "engine_info" => Ok(json!({
                "engine": "rust",
                "version": env!("CARGO_PKG_VERSION"),
                "ported": ported(),
                "writes": ported_writes(platform),
                // Which real volume backend answers volume methods here (`null`: none yet, ask Python).
                "volume_backend": platform_backend_name(),
                "once": ONCE_READS,
                "state_dir": self.state_dir.as_ref().map(|p| p.display().to_string()),
            })),
            _ if READ_METHODS.contains(&method) || READ_WHEN_EMPTY.contains(&method) => {
                let _ = params;
                Err(Error::new(
                    "not_ported",
                    format!("Method not ported to the Rust engine yet: {method}"),
                ))
            }
            _ => Err(Error::new(
                "unknown_method",
                format!("Unknown media service method: {method}"),
            )),
        }
    }

    /// One line in, one envelope out. Never panics on input.
    fn answer(&self, line: &str, mode: Mode) -> Value {
        let mut id = Value::Null;
        let result = (|| -> Result<Value> {
            require(
                mode == Mode::Once || line.len() <= MAX_LINE,
                "request_too_large",
                "Request line exceeds 1 MiB",
            )?;
            let request: Value = serde_json::from_str(line)
                .map_err(|e| Error::new("operation_failed", e.to_string()))?;
            require(
                request.is_object() && request["method"].is_string(),
                "invalid_request",
                "Expected id/method/params JSON",
            )?;
            id = request["id"].clone();
            let method = request["method"].as_str().unwrap_or_default();
            let params = match &request["params"] {
                Value::Null => json!({}),
                other => other.clone(),
            };
            let empty = params.as_object().is_none_or(|p| p.is_empty());
            let readable = READ_METHODS.contains(&method)
                || method == "engine_info"
                || (READ_WHEN_EMPTY.contains(&method) && empty);
            if mode == Mode::Serve {
                require(
                    readable,
                    "read_only",
                    format!("rpc-serve only answers reads: {method}"),
                )?;
            } else if !readable
                && !ported_writes(platform_backend(&self.home()).as_deref()).contains(&method)
                && !ONCE_READS.contains(&method)
            {
                return Err(Error::new(
                    "not_ported",
                    format!("Write not ported to the Rust engine yet: {method}"),
                ));
            }
            self.call(method, &params)
        })();
        match result {
            Ok(data) => ok_envelope(&id, data),
            Err(error) => error_envelope(&id, &error),
        }
    }

    fn serve(&self) -> i32 {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().lock();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { return 1 };
            if line.trim().is_empty() {
                continue;
            }
            let envelope = self.answer(&line, Mode::Serve);
            if writeln!(stdout, "{envelope}")
                .and_then(|_| stdout.flush())
                .is_err()
            {
                return 0; // the client went away
            }
        }
        0
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
            other => command = Some(other.to_owned()),
        }
        i += 1;
    }
    let engine = Engine { state_dir };
    let code = match command.as_deref() {
        Some("rpc-serve") => engine.serve(),
        Some("rpc") => {
            let mut raw = String::new();
            let read = std::io::stdin()
                .take(16 * 1024 * 1024 + 1)
                .read_to_string(&mut raw);
            let envelope = match read {
                Ok(_) if raw.len() <= 16 * 1024 * 1024 => engine.answer(raw.trim(), Mode::Once),
                _ => error_envelope(
                    &Value::Null,
                    &Error::new("request_too_large", "Request exceeds 16 MiB"),
                ),
            };
            println!("{envelope}");
            i32::from(envelope["ok"] != json!(true))
        }
        _ => {
            eprintln!("usage: nuun-engine [--state-dir DIR] rpc-serve | rpc | --version");
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
            "operation_failed",
            "Python: KeyError on a missing target"
        );
        assert_eq!(
            e.answer(r#"{"id": 1, "method": "job"}"#, Mode::Serve)["error"]["code"],
            "operation_failed",
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
            "operation_failed"
        );
        assert_eq!(
            e.answer(r#"[1, 2]"#, Mode::Serve)["error"]["code"],
            "invalid_request"
        );
        let long = format!(
            r#"{{"id": 4, "method": "recipe", "params": {{"pad": "{}"}}}}"#,
            "x".repeat(MAX_LINE)
        );
        assert_eq!(
            e.answer(&long, Mode::Serve)["error"]["code"],
            "request_too_large"
        );
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
            "not_ported"
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
                assert_eq!(
                    engine().answer(&request, Mode::Once)["error"]["code"],
                    "not_ported"
                );
            }
        }
        assert_eq!(cfg!(any(windows, target_os = "macos")), has);
        // macOS reads first: its writes open after a physical run (docs/release.md).
        assert_eq!(cfg!(windows), writes);
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
