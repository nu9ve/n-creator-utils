//! Installing, updating and removing the agent (nu9ve-agent-v1 §1 to §4), and what an app sees (§5).
//!
//! An app asks with `nu9ve agent state` (its own bundled binary, which knows its version) or, in Rust,
//! with [`state_of`]. Both use the same rules as `install`, so no app repeats them.

use super::serve;
use super::{agent_dir, other, qa};
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

pub const CONTRACT: &str = "nu9ve-agent/1";
/// §2: `public` (no cloud) for every build of this repository. The cloud build comes from inner-core (F2).
pub const BUILD: &str = "public";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// §1: the LaunchAgent label, and its file in `~/Library/LaunchAgents/`.
pub const LABEL: &str = "xyz.nu9ve.agent";
/// §1: the scheduled task on Windows.
pub const TASK: &str = "nu9ve agent";
/// §2: `agent.json` is at most 4 KiB.
pub const MAX_AGENT_JSON: usize = 4096;

const STOP_WAIT: Duration = Duration::from_secs(10);

/// The executable's name in the agent folder (§2 `binary`).
pub fn binary_name() -> &'static str {
    if cfg!(windows) { "nu9ve.exe" } else { "nu9ve" }
}

/// §2: `MAYOR.MENOR.PARCHE`, integers without leading zeroes.
pub fn version_key(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let mut next = || {
        let part = parts.next()?;
        let ok = !part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'));
        ok.then(|| part.parse().ok()).flatten()
    };
    let key = (next()?, next()?, next()?);
    parts.next().is_none().then_some(key)
}

fn utc_time(text: &str) -> bool {
    let b = text.as_bytes();
    let digit = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    let fixed = b.len() >= 20
        && (0..4).all(digit)
        && b[4] == b'-'
        && (5..7).all(digit)
        && b[7] == b'-'
        && (8..10).all(digit)
        && b[10] == b'T'
        && (11..13).all(digit)
        && b[13] == b':'
        && (14..16).all(digit)
        && b[16] == b':'
        && (17..19).all(digit);
    fixed
        && match &b[19..] {
            [b'Z'] => true,
            [b'.', rest @ .., b'Z'] => {
                (1..=9).contains(&rest.len()) && rest.iter().all(u8::is_ascii_digit)
            }
            _ => false,
        }
}

fn app_name(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// §2: `None` when `agent.json` is valid, otherwise why not (the tokens of the examples).
pub fn agent_problem(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > MAX_AGENT_JSON {
        return Some("too_large");
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Some("not_json");
    };
    let Some(map) = value.as_object() else {
        return Some("not_object");
    };
    for field in [
        "contract",
        "version",
        "build",
        "installed_at",
        "installed_by",
        "binary",
    ] {
        match map.get(field) {
            None => return Some("missing_field"),
            Some(v) if !v.is_string() => return Some("wrong_type"),
            _ => {}
        }
    }
    let text = |f: &str| map[f].as_str().unwrap_or_default();
    if text("contract") != CONTRACT {
        return Some("unknown_contract");
    }
    if version_key(text("version")).is_none() {
        return Some("bad_version");
    }
    if !matches!(text("build"), "public" | "cloud") {
        return Some("bad_build");
    }
    if !utc_time(text("installed_at")) {
        return Some("bad_time");
    }
    if text("installed_by") != "installer" && !app_name(text("installed_by")) {
        return Some("bad_installed_by");
    }
    if !matches!(text("binary"), "nu9ve" | "nu9ve.exe") {
        return Some("bad_binary");
    }
    None
}

/// §3: what an installer does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Install,
    OfferUpdate,
    Keep,
}

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::OfferUpdate => "offer_update",
            Action::Keep => "keep",
        }
    }
}

/// §3: carrying `bundled` (version, build) over `installed`. Versions must be valid (§2).
pub fn action(bundled: (&str, &str), installed: Option<(&str, &str)>) -> Action {
    let Some((i, build_i)) = installed else {
        return Action::Install;
    };
    let (v, build) = bundled;
    if build == "public" && build_i == "cloud" {
        return Action::Keep;
    }
    match (version_key(v), version_key(i)) {
        (Some(v), Some(i)) if v > i => Action::OfferUpdate,
        (Some(v), Some(i)) if v == i && build == "cloud" && build_i == "public" => {
            Action::OfferUpdate
        }
        _ => Action::Keep,
    }
}

/// §5: what an app sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppState {
    Absent,
    Installed,
    Running,
    Outdated,
}

impl AppState {
    pub fn id(self) -> &'static str {
        match self {
            AppState::Absent => "absent",
            AppState::Installed => "installed",
            AppState::Running => "running",
            AppState::Outdated => "outdated",
        }
    }
}

/// §5 from its inputs: the bytes of `agent.json` (or none), whether the `suite` record is live, and
/// what the app carries (or nothing).
pub fn app_state(
    agent_json: Option<&[u8]>,
    registry_live: bool,
    bundled: Option<(&str, &str)>,
) -> AppState {
    let Some(bytes) = agent_json.filter(|b| agent_problem(b).is_none()) else {
        return AppState::Absent;
    };
    let value: Value = serde_json::from_slice(bytes).unwrap_or_default();
    let installed = (
        value["version"].as_str().unwrap_or_default(),
        value["build"].as_str().unwrap_or_default(),
    );
    if bundled.is_some_and(|b| action(b, Some(installed)) == Action::OfferUpdate) {
        return AppState::Outdated;
    }
    if registry_live {
        AppState::Running
    } else {
        AppState::Installed
    }
}

/// `agent.json` of the agent folder, if it can be read (bounded); validity is [`agent_problem`]'s.
fn read_agent_json(dir: &Path) -> Option<Vec<u8>> {
    let file = fs::File::open(dir.join("agent.json")).ok()?;
    let mut bytes = Vec::new();
    io::Read::read_to_end(
        &mut io::Read::take(file, MAX_AGENT_JSON as u64 + 1),
        &mut bytes,
    )
    .ok()?;
    Some(bytes)
}

fn installed(dir: &Path) -> Option<Value> {
    let bytes = read_agent_json(dir)?;
    agent_problem(&bytes)
        .is_none()
        .then(|| serde_json::from_slice(&bytes).ok())?
}

/// The `suite` record is live in the engine registry (registry §6).
fn registry_live() -> bool {
    nu9ve_ipc::default_dir()
        .and_then(|d| nu9ve_ipc::read_entry(&d, "suite").ok().flatten())
        .is_some_and(|e| matches!(e.state, nu9ve_ipc::State::Live(_)))
}

/// §5 for this machine, seen by an app that carries `bundled` (or nothing): the function an app in Rust
/// calls. `nu9ve agent state` prints it, with this binary as what the app carries.
pub fn state_of(bundled: Option<(&str, &str)>) -> io::Result<Value> {
    let dir = agent_dir().ok_or_else(|| other("no agent folder"))?;
    let bytes = read_agent_json(&dir);
    let live = registry_live();
    let state = app_state(bytes.as_deref(), live, bundled);
    let found = installed(&dir);
    let action = bundled.map(|b| {
        action(
            b,
            found.as_ref().map(|v| {
                (
                    v["version"].as_str().unwrap_or_default(),
                    v["build"].as_str().unwrap_or_default(),
                )
            }),
        )
        .id()
    });
    Ok(
        json!({"state": state.id(), "installed": found.map(|v| json!({
        "version": v["version"], "build": v["build"], "installed_by": v["installed_by"], "installed_at": v["installed_at"]})),
        "bundled": bundled.map(|(v, b)| json!({"version": v, "build": b})), "action": action}),
    )
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> io::Result<()> {
    Ok(())
}

/// §4: asks the running agent to stop, and waits. In QA (and when it was started by hand) by a signal
/// to the pid holding the serve lock; outside QA the system startup is stopped first (see `startup`).
fn stop_running(dir: &Path) -> io::Result<bool> {
    let Some(pid) = serve::running_pid(dir) else {
        return Ok(false);
    };
    // The suite engine's `engine_shutdown` first, if it answers; a signal otherwise.
    if !super::suite::ask_to_stop(dir) {
        signal_stop(pid)?;
    }
    let deadline = Instant::now() + STOP_WAIT;
    while serve::running_pid(dir).is_some() {
        if Instant::now() > deadline {
            return Err(other(format!(
                "the running agent (pid {pid}) did not stop; nothing was changed"
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(true)
}

#[cfg(unix)]
fn signal_stop(pid: u32) -> io::Result<()> {
    let pid = i32::try_from(pid).map_err(|_| other("pid out of range"))?;
    // SAFETY: kill with a pid read from the lock its holder wrote; no memory is involved.
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn signal_stop(pid: u32) -> io::Result<()> {
    // Not verified: Windows has no SIGTERM; the task is ended, then the process.
    let _ = std::process::Command::new("schtasks")
        .args(["/End", "/TN", TASK])
        .status();
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .status();
    Ok(())
}

/// §1: the system startup of the agent. Never touched in QA.
mod startup {
    use super::*;

    /// The LaunchAgent: at login, kept alive if it crashes, not after an orderly stop.
    pub fn plist(binary: &Path, dir: &Path) -> String {
        let escape = |p: &Path| {
            p.to_string_lossy()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>agent</string>
		<string>serve</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>ProcessType</key>
	<string>Background</string>
	<key>StandardErrorPath</key>
	<string>{}</string>
</dict>
</plist>
"#,
            escape(binary),
            escape(&dir.join("agent.log")),
        )
    }

    #[cfg(target_os = "macos")]
    fn plist_path() -> io::Result<std::path::PathBuf> {
        let home = std::env::var("HOME").map_err(|_| other("no HOME"))?;
        Ok(std::path::PathBuf::from(home).join(format!("Library/LaunchAgents/{LABEL}.plist")))
    }

    #[cfg(target_os = "macos")]
    fn domain() -> String {
        // SAFETY: getuid has no preconditions.
        format!("gui/{}", unsafe { libc::getuid() })
    }

    #[cfg(target_os = "macos")]
    fn launchctl(args: &[&str]) -> io::Result<bool> {
        Ok(std::process::Command::new("/bin/launchctl")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?
            .success())
    }

    /// Stops and unloads the agent's startup, if it is loaded. Not verified on a real LaunchAgent.
    pub fn remove(unregister: bool) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            launchctl(&["bootout", &format!("{}/{LABEL}", domain())])?;
            if unregister {
                match fs::remove_file(plist_path()?) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                    _ => {}
                }
            }
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("schtasks")
                .args(["/End", "/TN", TASK])
                .status();
            if unregister {
                let _ = std::process::Command::new("schtasks")
                    .args(["/Delete", "/TN", TASK, "/F"])
                    .status();
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        let _ = unregister;
        Ok(())
    }

    /// Writes and loads the startup, which starts the agent. Not verified on a real LaunchAgent or task.
    pub fn add(binary: &Path, dir: &Path) -> io::Result<&'static str> {
        #[cfg(target_os = "macos")]
        {
            let path = plist_path()?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            nu9ve_fs::atomic_write_with(
                &path,
                plist(binary, dir).as_bytes(),
                nu9ve_fs::SyncMode::Plain,
            )?;
            set_mode(&path, 0o644)?;
            if !launchctl(&["bootstrap", &domain(), &path.to_string_lossy()])? {
                launchctl(&["kickstart", "-k", &format!("{}/{LABEL}", domain())])?;
            }
            Ok("launchd")
        }
        #[cfg(windows)]
        {
            let _ = dir;
            let run = format!("\"{}\" agent serve", binary.display());
            let created = std::process::Command::new("schtasks")
                .args([
                    "/Create", "/TN", TASK, "/SC", "ONLOGON", "/RL", "LIMITED", "/F", "/TR", &run,
                ])
                .status()?
                .success();
            if !created {
                return Err(other("schtasks could not create the task"));
            }
            let _ = std::process::Command::new("schtasks")
                .args(["/Run", "/TN", TASK])
                .status();
            Ok("task")
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = (binary, dir);
            Err(other("this system has no agent startup in v1"))
        }
    }
}

pub use startup::plist;

/// The first line `<binary> --version` prints.
fn reported_version(binary: &Path) -> Option<String> {
    let out = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .map(str::to_owned)
        })
        .flatten()
}

/// §3 and §4: installs or updates the agent from this binary, if §3 says so. Run by the agent's own
/// installer, or by an app after the person chose «Instalar el agente» or «Actualizar» in its settings.
pub fn install(installed_by: &str) -> io::Result<Value> {
    if installed_by != "installer" && !app_name(installed_by) {
        return Err(other(
            "installed_by is «installer» or an app name (^[a-z][a-z0-9-]{0,31}$)",
        ));
    }
    let dir = agent_dir().ok_or_else(|| other("no agent folder"))?;
    let found = installed(&dir);
    let todo = action(
        (VERSION, BUILD),
        found.as_ref().map(|v| {
            (
                v["version"].as_str().unwrap_or_default(),
                v["build"].as_str().unwrap_or_default(),
            )
        }),
    );
    if todo == Action::Keep {
        return Ok(
            json!({"action": "keep", "installed": found.map(|v| json!({"version": v["version"], "build": v["build"]})),
            "bundled": {"version": VERSION, "build": BUILD}}),
        );
    }
    fs::create_dir_all(&dir)?;
    set_mode(&dir, 0o700)?;

    // A temporary copy inside the folder, checked before anything else changes.
    let source = std::env::current_exe()?;
    let target = dir.join(binary_name());
    let temp = dir.join(format!(".{}.new-{}", binary_name(), std::process::id()));
    let _ = fs::remove_file(&temp);
    fs::copy(&source, &temp)?;
    set_mode(&temp, 0o700)?;
    let expected = format!("nu9ve {VERSION}");
    if reported_version(&temp).as_deref() != Some(expected.as_str()) {
        let _ = fs::remove_file(&temp);
        return Err(other(format!(
            "the copy does not report «{expected}»; the installed agent is left as it was"
        )));
    }

    // No two agents at once: the startup goes first (it would start it again), then the running one.
    if !qa()
        && let Err(e) = startup::remove(false)
    {
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    let stopped = match stop_running(&dir) {
        Ok(stopped) => stopped,
        Err(e) => {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
    };
    if let Err(e) = fs::rename(&temp, &target) {
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    let record = json!({"contract": CONTRACT, "version": VERSION, "build": BUILD,
        "installed_at": nu9ve_device::now_utc(), "installed_by": installed_by, "binary": binary_name()});
    let path = dir.join("agent.json");
    nu9ve_fs::atomic_write_with(
        &path,
        &serde_json::to_vec(&record).map_err(io::Error::other)?,
        nu9ve_fs::SyncMode::Plain,
    )?;
    set_mode(&path, 0o600)?;
    // QA never registers the startup in the system (§1).
    let startup = if qa() {
        "none"
    } else {
        startup::add(&target, &dir)?
    };
    Ok(
        json!({"action": if found.is_some() { "update" } else { "install" },
        "version": VERSION, "build": BUILD, "installed_by": installed_by, "binary": target,
        "startup": startup, "stopped_running": stopped}),
    )
}

/// §4: removes the startup, the binary and `agent.json`. Never `device.json`, the device key or the
/// exchange folder, nor the agent's own state (`seq`, pairings): another agent installed later uses them.
pub fn uninstall() -> io::Result<Value> {
    let dir = agent_dir().ok_or_else(|| other("no agent folder"))?;
    if !qa() {
        startup::remove(true)?;
    }
    let stopped = stop_running(&dir)?;
    let mut removed = Vec::new();
    for name in [binary_name(), "agent.json"] {
        match fs::remove_file(dir.join(name)) {
            Ok(()) => removed.push(name),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(json!({"removed": removed, "stopped_running": stopped, "startup_removed": !qa()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn examples() -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/agent-examples-v1.json");
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    fn pair(v: &Value) -> Option<(&str, &str)> {
        v.as_array()
            .map(|a| (a[0].as_str().unwrap(), a[1].as_str().unwrap()))
    }

    #[test]
    fn agent_json_files_of_the_examples() {
        for case in examples()["files"].as_array().unwrap() {
            let got = agent_problem(case["text"].as_str().unwrap().as_bytes());
            assert_eq!(got, case["problem"].as_str(), "{}", case["name"]);
        }
    }

    #[test]
    fn version_order_of_the_examples() {
        let order: Vec<(u64, u64, u64)> = examples()["version_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| version_key(v.as_str().unwrap()).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]));
        for bad in ["0.03.0", "0.3", "1.2.3.4", "a.b.c", "", "1..2"] {
            assert!(version_key(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn actions_and_states_of_the_examples() {
        let examples = examples();
        for case in examples["actions"].as_array().unwrap() {
            let got = action(pair(&case["bundled"]).unwrap(), pair(&case["installed"]));
            assert_eq!(got.id(), case["action"], "{}", case["name"]);
        }
        for case in examples["states"].as_array().unwrap() {
            let got = app_state(
                case["agent_json"].as_str().map(str::as_bytes),
                case["registry_live"].as_bool().unwrap(),
                pair(&case["bundled"]),
            );
            assert_eq!(got.id(), case["state"], "{}", case["name"]);
        }
    }

    #[test]
    fn the_launch_agent_runs_serve_and_escapes_paths() {
        let text = plist(Path::new("/A & B/nu9ve"), Path::new("/A & B"));
        assert!(text.contains("<string>xyz.nu9ve.agent</string>"));
        assert!(text.contains("<string>/A &amp; B/nu9ve</string>"));
        assert!(text.contains("<string>serve</string>"));
        assert!(text.contains("<string>/A &amp; B/agent.log</string>"));
    }
}
