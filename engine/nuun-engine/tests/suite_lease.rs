//! «Trabajar aquí» and «Dejar de trabajar aquí» (exchange §7.1): `suite_lease` asks the agent's `suite` engine
//! for `lease_take`, `lease_release` and `leases`. The agent is the real `nu9ve` binary from the pinned core,
//! serving in QA folders only (`NU9VE_AGENT_DIR`, `NU9VE_DEVICE_FILE`, `NU9VE_ENGINES_DIR`, `NU9VE_VOLUMES_DIR`,
//! a temporary `HOME`): no keychain, no LaunchAgent, nothing of this machine's agent.
//! Build it first: `cargo build -p nu9ve-mcp --bin nu9ve`.
#![cfg(unix)]
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn agent_binary() -> PathBuf {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target");
    ["debug", "release"]
        .iter()
        .map(|p| target.join(p).join("nu9ve"))
        .find(|p| p.is_file())
        .expect("build the agent first: cargo build -p nu9ve-mcp --bin nu9ve")
}

fn qa(command: &mut Command, qa: &Path) {
    command
        .env("HOME", qa.join("home"))
        .env("NU9VE_AGENT_DIR", qa.join("agent"))
        .env("NU9VE_DEVICE_FILE", qa.join("device.json"))
        .env("NU9VE_ENGINES_DIR", qa.join("engines"))
        .env("NU9VE_VOLUMES_DIR", qa.join("volumes"));
}

fn agent(dir: &Path, args: &[&str]) -> Value {
    let mut command = Command::new(agent_binary());
    command.args(["agent"]).args(args);
    qa(&mut command, dir);
    let out = command.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn lease(dir: &Path, params: Value) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nuun-engine"));
    command
        .args(["--state-dir", dir.join("state").to_str().unwrap(), "rpc"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    qa(&mut command, dir);
    let mut child = command.spawn().unwrap();
    let request = json!({"method": "suite_lease", "params": params}).to_string();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.as_bytes())
        .unwrap();
    serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap()
}

#[test]
fn work_here_takes_and_releases_the_project_through_the_agent() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().canonicalize().unwrap();
    for sub in ["home", "exchange"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    // Without an agent nothing changes, and nüün says so.
    let none = lease(&dir, json!({"action": "take", "project_key": "baches"}));
    assert_eq!(none["data"]["agent"], "absent", "{none}");
    agent(&dir, &["init"]);
    agent(&dir, &["exchange", dir.join("exchange").to_str().unwrap()]);
    let mut serve = Command::new(agent_binary());
    serve
        .args(["agent", "serve"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    qa(&mut serve, &dir);
    let mut serve = serve.spawn().unwrap();
    let start = Instant::now();
    while !matches!(
        nu9ve_ipc::read_entry(&dir.join("engines"), "suite"),
        Ok(Some(nu9ve_ipc::Entry {
            state: nu9ve_ipc::State::Live(_),
            ..
        }))
    ) {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "the agent did not register"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let taken = lease(&dir, json!({"action": "take", "project_key": "baches"}));
    assert_eq!(taken["ok"], true, "{taken}");
    assert_eq!(
        (
            taken["data"]["agent"].as_str(),
            taken["data"]["here"].as_bool()
        ),
        (Some("running"), Some(true))
    );
    assert_eq!(taken["data"]["changed"]["taken"], true);
    let state = lease(&dir, json!({"action": "state", "project_key": "baches"}));
    assert_eq!(
        state["data"]["holders"].as_array().unwrap().len(),
        1,
        "{state}"
    );
    // Without a key: every nüün project some machine works on, in one call.
    let all = lease(&dir, json!({"action": "state"}));
    assert_eq!(all["data"]["holders"][0]["project_key"], "baches", "{all}");
    // Another project of nüün is not this one.
    assert_eq!(
        lease(&dir, json!({"action": "state", "project_key": "otro"}))["data"]["here"],
        false
    );
    let released = lease(&dir, json!({"action": "release", "project_key": "baches"}));
    assert_eq!(
        (
            released["data"]["here"].as_bool(),
            released["data"]["changed"]["released"].as_bool()
        ),
        (Some(false), Some(true)),
        "{released}"
    );
    let bad = lease(&dir, json!({"action": "steal", "project_key": "baches"}));
    assert_eq!(bad["error"]["code"], "invalid_params");
    // SAFETY: SIGTERM to the agent this test started.
    unsafe { libc::kill(serve.id() as i32, libc::SIGTERM) };
    serve.wait().unwrap();
}
