//! nu9ve-agent-v1 with nu9ve-device-v1: once the suite agent is installed, `device.json` is the agent's. The
//! engine reads it and never creates it. Always QA folders (`NU9VE_AGENT_DIR`, `NU9VE_DEVICE_FILE`): nothing
//! here reaches this machine's agent or device.
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn call(state: &Path, qa: &Path, method: &str, params: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nuun-engine"))
        .args(["--state-dir", state.to_str().unwrap(), "rpc"])
        .env("NU9VE_AGENT_DIR", qa.join("agent"))
        .env("NU9VE_DEVICE_FILE", qa.join("device.json"))
        .env("NU9VE_ENGINES_DIR", qa.join("engines"))
        .env("NU9VE_VOLUMES_DIR", qa.join("volumes"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"method": method, "params": params}).to_string();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn with_an_installed_agent_the_engine_never_creates_device_json() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (state, qa) = (root.join("state"), root.join("qa"));
    std::fs::create_dir_all(qa.join("agent")).unwrap();
    // Without the agent nüün still creates it (device §9.2).
    let created = call(&state, &qa, "suite_device", json!({}));
    assert_eq!(created["data"]["state"], "ok", "{created}");
    let id = created["data"]["device"]["device_id"].clone();
    std::fs::remove_file(qa.join("device.json")).unwrap();
    std::fs::write(
        qa.join("agent/agent.json"),
        br#"{"contract":"nu9ve-agent/1","version":"0.2.2","build":"public","installed_at":"2026-11-22T22:00:00Z","installed_by":"nuun","binary":"nu9ve"}"#,
    )
    .unwrap();
    let refused = call(&state, &qa, "suite_device", json!({}));
    assert_eq!(refused["data"]["state"], "none", "{refused}");
    assert_eq!(refused["data"]["reason"], "agent_device");
    assert!(!qa.join("device.json").exists());
    // The agent's file (the same format): nüün uses it and can rename it.
    let mut device = created["data"]["device"].clone();
    device["name"] = json!("M1");
    std::fs::write(qa.join("device.json"), device.to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            qa.join("device.json"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let seen = call(&state, &qa, "suite_device", json!({"name": "M4"}));
    assert_eq!(seen["data"]["device"]["device_id"], id, "{seen}");
    assert_eq!(seen["data"]["device"]["name"], "M4");
}
