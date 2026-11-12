//! Python `integrations.capabilities` (with `clipper.executable`/`invoke`/`dependencies` and `retitle.producer`):
//! which optional consumers are installed, by asking them. Failure stays local to each one.
//!
//! The catalog settings are read first ([`Integrations::read`]), so the two executables can then run on their own
//! threads, at the same time, and in parallel with the rest of `snapshot`: Python runs them one after the other.
//! Difference on purpose: Python records `clipper-known` when it finds the executable; this engine opens the
//! catalog read only and leaves that write to Python, but answers `known` as Python would right after it.

use crate::catalog::Catalog;
use nu9ve_json::{Error, Result, require};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// retitle.py `CAPABILITY`.
pub const STALKER_CAPABILITY: &str = "stalker-coordinated-retitle/1";
const CLIPPER_CAPABILITY: &str = "media-release-lock/1";
/// integrations.py: both consumers are asked with `timeout=3`.
const TIMEOUT: Duration = Duration::from_secs(3);

fn truthy(value: &Option<Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// The catalog settings both discoveries need, read before any thread starts.
#[derive(Clone, Debug)]
pub struct Integrations {
    clipper_configured: String,
    clipper_known: bool,
    /// `CLIPPER_PROJECTS_ROOT` or `~/Downloads/clipper-projects`: its queue files also mean Clipper was used.
    pub(crate) clipper_base: PathBuf,
    stalker: Option<Value>,
}

impl Integrations {
    pub fn read(catalog: &Catalog) -> Result<Self> {
        let configured = catalog.get("clipper-executable")?;
        let clipper_known = truthy(&catalog.get("clipper-known")?) || truthy(&configured);
        Ok(Self {
            clipper_configured: configured
                .as_ref()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            clipper_known,
            clipper_base: std::env::var_os("CLIPPER_PROJECTS_ROOT")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .or_else(|| home().map(|h| h.join("Downloads/clipper-projects")))
                .unwrap_or_default(),
            stalker: catalog.get("stalker-executable")?,
        })
    }

    /// A configured Clipper and projects folder, without Stalker (fixture tests).
    #[cfg(test)]
    pub(crate) fn fixture(clipper: &str, base: PathBuf) -> Self {
        Self {
            clipper_configured: clipper.into(),
            clipper_known: true,
            clipper_base: base,
            stalker: None,
        }
    }

    /// Integrations that find nothing and start nothing, for tests of the reads that include them.
    #[cfg(test)]
    pub(crate) fn isolated() -> Self {
        Self {
            clipper_configured: "/nonexistent/clipper".into(),
            clipper_known: false,
            clipper_base: PathBuf::from("/nonexistent/clipper-projects"),
            stalker: None,
        }
    }

    /// Python `capabilities(catalog)`: Clipper and Stalker asked at the same time.
    pub fn capabilities(&self) -> Value {
        let (clipper, stalker) = std::thread::scope(|scope| {
            let stalker = scope.spawn(|| self.stalker());
            let clipper = self.clipper();
            (
                clipper,
                stalker
                    .join()
                    .unwrap_or_else(|_| failed("stalker panicked")),
            )
        });
        json!({"clipper": clipper, "stalker": stalker})
    }

    fn clipper(&self) -> Value {
        match self.dependencies() {
            Ok((data, found)) => {
                let capabilities = data.get("capabilities").cloned().unwrap_or(json!([]));
                let available = data["integration"] != "never_configured"
                    && contains(&capabilities, CLIPPER_CAPABILITY);
                // Python sets `clipper-known` as soon as it finds the executable, then reads it back.
                json!({"available": available, "capabilities": capabilities,
                       "known": self.clipper_known || found})
            }
            Err(error) => failed(&error.message),
        }
    }

    /// Python `clipper.executable`: the configured path, else the usual places; absolute, an executable file.
    fn clipper_executable(&self) -> Result<String> {
        let candidates: Vec<Option<String>> = if self.clipper_configured.is_empty() {
            vec![
                which("clipper"),
                home().map(|h| h.join(".local/bin/clipper").display().to_string()),
                Some("/opt/homebrew/bin/clipper".into()),
            ]
        } else {
            vec![Some(self.clipper_configured.clone())]
        };
        candidates
            .into_iter()
            .flatten()
            .find(|p| {
                let path = Path::new(p);
                path.is_absolute() && executable(path) && path.is_file()
            })
            .ok_or_else(|| {
                Error::new(
                    "clipper_unavailable",
                    "Configura el ejecutable de Clipper en Ajustes → Capturas antes de liberar espacio",
                )
            })
    }

    /// Python `clipper.dependencies(timeout=3)`, and whether the executable was found (Python then records it).
    fn dependencies(&self) -> Result<(Value, bool)> {
        self.dependencies_within(TIMEOUT)
    }

    /// Python `clipper.dependencies(catalog, timeout)`: Clipper's linked-library report, and whether its executable
    /// was found. `inspect` waits up to 120 s, as Python.
    pub(crate) fn dependencies_within(&self, timeout: Duration) -> Result<(Value, bool)> {
        let command = match self.clipper_executable() {
            Ok(command) => command,
            Err(error) => {
                let base = &self.clipper_base;
                let known = self.clipper_known
                    || base.join(".linked-projects.json").exists()
                    || base.join(".export-queue/queue.json").exists();
                if !known {
                    // Only executable absence, never a failed/invalid installed consumer.
                    return Ok((
                        json!({"schema": "clipper-library-deps/1", "capabilities": [], "projects": [],
                               "offline_projects": [], "integration": "never_configured"}),
                        false,
                    ));
                }
                return Err(error);
            }
        };
        let argv = [command.as_str(), "library", "deps", "--json"];
        let unavailable = |m: String| {
            Error::new(
                "clipper_unavailable",
                format!("No se pudo consultar Clipper: {m}"),
            )
        };
        let output = run(&argv, None, &[], timeout).map_err(unavailable)?;
        require(
            output.success,
            "clipper_failed",
            "Clipper no pudo comprobar/actualizar sus dependencias; no se libera el origen",
        )?;
        require(
            output.stdout.len() <= 16 * 1024 * 1024,
            "clipper_invalid",
            "Clipper response too large",
        )?;
        let data: Value =
            serde_json::from_slice(&output.stdout).map_err(|e| unavailable(e.to_string()))?;
        validate_dependencies(&data)?;
        Ok((data, true))
    }

    /// Python `retitle.producer(catalog, 'capabilities', {}, timeout=3)`.
    fn stalker(&self) -> Value {
        match self.stalker_capabilities() {
            Ok(data) => {
                let capabilities = data.get("capabilities").cloned().unwrap_or(json!([]));
                let available = data["schema"] == "stalker-capabilities/1"
                    && contains(&capabilities, STALKER_CAPABILITY);
                json!({"available": available, "capabilities": capabilities})
            }
            Err(error) => failed(&error.message),
        }
    }

    fn stalker_capabilities(&self) -> Result<Value> {
        let path = self
            .stalker
            .as_ref()
            .and_then(Value::as_str)
            .filter(|p| Path::new(p).is_absolute() && executable(Path::new(p)))
            .ok_or_else(|| {
                Error::new(
                    "stalker_upgrade_required",
                    format!(
                        "Configure stalker_executable implementing {STALKER_CAPABILITY}; archived:false is insufficient. See docs/contracts/stalker-retitle-v1.md"
                    ),
                )
            })?;
        let argv = [path, "nuun-retitle", "capabilities", "--json"];
        // Python passes the coordinator descriptors and the release token, both empty outside an operation.
        let env = [
            ("NUUN_RETITLE_LOCK_FDS", ""),
            ("CLIPPER_MEDIA_RELEASE_TOKEN", ""),
        ];
        let output = run(&argv, Some(b"{}"), &env, TIMEOUT).map_err(|m| {
            Error::new(
                "stalker_failed",
                format!("Stalker unavailable; recover with the same operation_id: {m}"),
            )
        })?;
        if !output.success || output.stdout.len() > 1024 * 1024 {
            let stderr: String = String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2000)
                .collect();
            return Err(Error::new(
                "stalker_upgrade_required",
                format!(
                    "Stalker must implement {STALKER_CAPABILITY} (docs/contracts/stalker-retitle-v1.md); retry the same operation_id to reconcile: {stderr}"
                ),
            ));
        }
        let data: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            Error::new(
                "stalker_failed",
                format!("Stalker unavailable; recover with the same operation_id: {e}"),
            )
        })?;
        require(
            data.is_object(),
            "stalker_invalid",
            "Invalid Stalker response",
        )?;
        Ok(data)
    }
}

/// Python `capabilities(catalog)` for the `integration_capabilities` method.
pub fn capabilities(catalog: &Catalog) -> Result<Value> {
    Ok(Integrations::read(catalog)?.capabilities())
}

fn failed(message: &str) -> Value {
    json!({"available": false, "error": message})
}

fn contains(list: &Value, wanted: &str) -> bool {
    match list {
        Value::Array(items) => items.iter().any(|v| v == wanted),
        Value::String(text) => text.contains(wanted),
        _ => false,
    }
}

/// clipper.py `dependencies`: missing fields must never turn an unknown dependency into an unused path.
fn validate_dependencies(data: &Value) -> Result<()> {
    require(
        data["schema"] == "clipper-library-deps/1"
            && data["projects"].is_array()
            && data["offline_projects"].is_array(),
        "clipper_invalid",
        "Unsupported Clipper dependencies",
    )?;
    for project in data["projects"].as_array().into_iter().flatten() {
        require(
            project["clipper_root"].is_string()
                && project["assets"].is_array()
                && project["relinks"].is_array(),
            "clipper_invalid",
            "Incomplete Clipper project dependencies",
        )?;
        for asset in project["assets"].as_array().into_iter().flatten() {
            let path = asset["path"].as_str().unwrap_or_default();
            require(
                !path.is_empty()
                    && Path::new(path).is_absolute()
                    && asset["in_use"].is_boolean()
                    && asset["queued_exports"].is_array()
                    && asset["items"].is_array()
                    && asset["asset_id"].is_string(),
                "clipper_invalid",
                "Incomplete Clipper asset dependencies",
            )?;
        }
    }
    Ok(())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// `shutil.which`: the first executable file named `name` on PATH.
fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file() && executable(candidate))
        .map(|p| p.display().to_string())
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    path.is_file()
}

struct Output {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// `subprocess.run(argv, input=…, capture_output=True, timeout=…)`; on timeout the child is killed, and the error
/// reads like Python's `TimeoutExpired`.
fn run(
    argv: &[&str],
    input: Option<&[u8]>,
    env: &[(&str, &str)],
    timeout: Duration,
) -> std::result::Result<Output, String> {
    let mut command = Command::new(argv[0]);
    command
        .args(&argv[1..])
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(bytes);
    }
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_end(&mut buffer);
        }
        buffer
    });
    let err_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut buffer);
        }
        buffer
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let shown: Vec<String> = argv.iter().map(|a| format!("'{a}'")).collect();
                return Err(format!(
                    "Command '[{}]' timed out after {} seconds",
                    shown.join(", "),
                    timeout.as_secs()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    Ok(Output {
        success: status.success(),
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, name: &str, body: &str) -> String {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.display().to_string()
    }

    fn with(clipper: &str, known: bool, stalker: Option<&str>) -> Integrations {
        Integrations {
            clipper_configured: clipper.into(),
            clipper_known: known,
            clipper_base: PathBuf::from("/nonexistent/clipper-projects"),
            stalker: stalker.map(|s| json!(s)),
        }
    }

    #[test]
    fn both_consumers_answer_like_python() {
        let dir = tempfile::tempdir().unwrap();
        let clipper = script(
            dir.path(),
            "clipper",
            r#"echo '{"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],"projects":[{"clipper_root":"/x","assets":[{"path":"/a.mov","in_use":true,"queued_exports":[],"items":[],"asset_id":"a"}],"relinks":[]}],"offline_projects":[]}'"#,
        );
        let stalker = script(
            dir.path(),
            "stalker",
            r#"read input; [ "$input" = "{}" ] && echo '{"schema":"stalker-capabilities/1","capabilities":["stalker-coordinated-retitle/1"]}'"#,
        );
        let found = with(&clipper, false, Some(&stalker)).capabilities();
        assert_eq!(
            found,
            json!({"clipper": {"available": true, "capabilities": ["media-release-lock/1"], "known": true},
                   "stalker": {"available": true, "capabilities": ["stalker-coordinated-retitle/1"]}})
        );
    }

    #[test]
    fn failures_stay_local_and_carry_pythons_messages() {
        let dir = tempfile::tempdir().unwrap();
        let broken = script(dir.path(), "clipper", "exit 3");
        let invalid = script(dir.path(), "stalker", "echo '[1]'");
        let found = with(&broken, true, Some(&invalid)).capabilities();
        assert_eq!(found["clipper"]["available"], false);
        assert_eq!(
            found["clipper"]["error"],
            "Clipper no pudo comprobar/actualizar sus dependencias; no se libera el origen"
        );
        assert_eq!(found["stalker"]["error"], "Invalid Stalker response");
        let unset = with("", true, None).capabilities();
        assert!(
            unset["stalker"]["error"]
                .as_str()
                .unwrap()
                .starts_with("Configure stalker_executable")
        );
    }

    #[test]
    fn an_absent_clipper_never_configured_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none").display().to_string();
        let found = with(&missing, false, None).dependencies().unwrap();
        assert_eq!(found.0["integration"], "never_configured");
        assert_eq!(
            with(&missing, true, None).dependencies().unwrap_err().code,
            "clipper_unavailable"
        );
    }

    #[test]
    fn a_slow_consumer_is_stopped_at_the_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let slow = script(dir.path(), "slow", "sleep 5");
        let started = Instant::now();
        let error = run(&[&slow, "x"], None, &[], Duration::from_millis(200))
            .err()
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(error.contains("timed out after 0 seconds"), "{error}");
    }
}
