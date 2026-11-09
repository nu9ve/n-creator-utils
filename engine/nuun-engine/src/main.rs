//! `nuun-engine [--state-dir DIR] rpc-serve | rpc | --version`
//!
//! The same boundary as `python -S nu9ve.py media [--state-dir DIR] rpc-serve` (media/cli.py `serve_reads`):
//! one JSON request per line `{id, method, params}`, one line back `{id, ok: true, data}` or
//! `{id, ok: false, error: {code, message}}`; lines up to 1 MiB; only read methods. Python stays the oracle
//! until each method reaches parity (rust-engine.md §7): a read method that is not ported yet answers
//! `not_ported`, so a client can ask the Python engine (or the Windows maqueta) instead.

use nu9ve_json::{Error, Result, error_envelope, ok_envelope, require};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;

/// media/cli.py `READ_METHODS` (2026-11-09) and `READ_WHEN_EMPTY`.
const READ_METHODS: [&str; 28] = [
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

/// Methods this engine answers. Everything else in READ_METHODS is `not_ported`. Volume methods need a backend
/// for the platform: Windows is in Rust; macOS is still Python (diskutil), so there they stay `not_ported`.
#[cfg(windows)]
const PORTED: [&str; 4] = ["recipe", "volumes", "source_cards", "engine_info"];
#[cfg(not(windows))]
const PORTED: [&str; 2] = ["recipe", "engine_info"];

/// Writes this engine performs, only through the one-shot `rpc` command (as Python: `rpc-serve` is read only).
/// `register_volume` changes configuration (the catalog), never media, under the writer lock.
#[cfg(windows)]
const PORTED_WRITES: [&str; 1] = ["register_volume"];
#[cfg(not(windows))]
const PORTED_WRITES: [&str; 0] = [];

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
            #[cfg(windows)]
            "volumes" => {
                nuun_media::volumes::volumes(&self.catalog()?, &nuun_media::volumes::WindowsBackend)
            }
            #[cfg(windows)]
            "source_cards" => nuun_media::volumes::source_cards(
                &self.catalog()?,
                &nuun_media::volumes::WindowsBackend,
                None,
            ),
            #[cfg(windows)]
            "register_volume" => nuun_media::volumes::register_volume(
                &self.home(),
                &nuun_media::volumes::WindowsBackend,
                params,
            ),
            // Not in Python: lets a client know which engine answered and what it can do.
            "engine_info" => Ok(json!({
                "engine": "rust",
                "version": env!("CARGO_PKG_VERSION"),
                "ported": PORTED,
                "writes": PORTED_WRITES,
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
            } else if !readable && !PORTED_WRITES.contains(&method) {
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

    #[test]
    fn empty_catalog_reads_answer_without_creating_anything() {
        let e = engine();
        let reply = e.answer(r#"{"id": 5, "method": "recipe"}"#, Mode::Serve);
        assert_eq!(reply["data"]["confirmed_devices"], 0);
        #[cfg(windows)]
        assert_eq!(
            e.answer(r#"{"id": 6, "method": "volumes"}"#, Mode::Serve)["data"],
            json!([])
        );
        #[cfg(not(windows))]
        assert_eq!(
            e.answer(r#"{"id": 6, "method": "volumes"}"#, Mode::Serve)["error"]["code"],
            "not_ported"
        );
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
            e.answer(r#"{"id": 1, "method": "snapshot"}"#, Mode::Serve)["error"]["code"],
            "not_ported"
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
            e.answer(r#"{"id": 10, "method": "apply", "params": {}}"#, Mode::Once)["error"]["code"],
            "not_ported"
        );
        #[cfg(windows)]
        {
            let reply = e.answer(&request, Mode::Once);
            assert_eq!(reply["ok"], true, "{reply}");
            assert_eq!(reply["data"]["identity"]["mode"], "windows");
            let listed = e.answer(r#"{"id": 11, "method": "volumes"}"#, Mode::Serve);
            assert_eq!(listed["data"][0]["name"], "LOCAL");
            assert_eq!(listed["data"][0]["online"], true);
        }
    }

    #[test]
    fn every_ported_method_is_a_read() {
        for method in PORTED {
            assert!(
                READ_METHODS.contains(&method) || method == "engine_info",
                "{method}"
            );
        }
    }
}
