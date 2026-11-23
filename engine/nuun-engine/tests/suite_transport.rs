//! nüün on the suite transport (nu9ve-registry-v1, nu9ve-engine-v1 §9): while `rpc-serve` runs, a socket in
//! frames serves `data_catalog` and `data_export`, registered as `nuun`; the record goes when the app closes it.
//! Always QA folders: the registry is `NU9VE_ENGINES_DIR` and `HOME` is temporary.
#![cfg(unix)]
use serde_json::json;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn serve(state: &Path, home: &Path, engines: Option<&Path>) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nuun-engine"));
    command
        .args(["--state-dir", state.to_str().unwrap(), "rpc-serve"])
        .env("HOME", home)
        .env_remove("NU9VE_ENGINES_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(engines) = engines {
        command.env("NU9VE_ENGINES_DIR", engines);
    }
    command.spawn().unwrap()
}

fn wait_for(what: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn rpc_serve_registers_its_socket_and_answers_the_suite() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (state, home, engines) = (root.join("state"), root.join("home"), root.join("engines"));
    std::fs::create_dir_all(&home).unwrap();
    let mut child = serve(&state, &home, Some(&engines));
    let live = || {
        matches!(
            nu9ve_ipc::read_entry(&engines, "nuun"),
            Ok(Some(nu9ve_ipc::Entry {
                state: nu9ve_ipc::State::Live(_),
                ..
            }))
        )
    };
    assert!(wait_for(live), "no live nuun record");
    let Ok(Some(nu9ve_ipc::Entry {
        state: nu9ve_ipc::State::Live(record),
        ..
    })) = nu9ve_ipc::read_entry(&engines, "nuun")
    else {
        unreachable!()
    };
    assert_eq!(record.pid, child.id());
    assert!(record.has_capability("data_catalog_v1") && record.has_capability("data_export_v1"));
    let timeout = nu9ve_ipc::DEFAULT_TIMEOUT;
    let info = nu9ve_ipc::request(&record, "engine_info", None, None, timeout).unwrap();
    assert_eq!(info["protocol"], record.engine.protocol);
    let catalog =
        nu9ve_ipc::request(&record, "data_catalog", Some(json!({})), None, timeout).unwrap();
    assert_eq!(
        (catalog["contract"].as_str(), catalog["app"].as_str()),
        (Some("nu9ve-data/1"), Some("nuun"))
    );
    let export = nu9ve_ipc::request(
        &record,
        "data_export",
        Some(json!({"dataset": "group:none"})),
        None,
        timeout,
    )
    .unwrap();
    assert_eq!(export["status"], "not_found");
    // Only the two suite methods: the app's reads and writes stay on its own stdio.
    let refused =
        nu9ve_ipc::request(&record, "snapshot", Some(json!({})), None, timeout).unwrap_err();
    assert_eq!(refused.code, "unknown_method");
    // A second engine of the same app does not take the record from the live one (registry §5).
    let mut second = serve(&root.join("state2"), &home, Some(&engines));
    std::thread::sleep(Duration::from_millis(500));
    assert!(matches!(nu9ve_ipc::read_entry(&engines, "nuun"),
        Ok(Some(nu9ve_ipc::Entry { state: nu9ve_ipc::State::Live(ref r), .. })) if r.pid == child.id()));
    drop(second.stdin.take());
    second.wait().unwrap();
    // Closing stdin (the app quitting) removes the record that is still this engine's.
    drop(child.stdin.take());
    child.wait().unwrap();
    assert!(nu9ve_ipc::read_entry(&engines, "nuun").unwrap().is_none());
}

#[test]
fn under_qa_without_its_own_registry_nothing_is_registered() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut child = serve(&root.join("state"), &home, None);
    std::thread::sleep(Duration::from_millis(500));
    // A --state-dir never writes the person's folder, here the temporary HOME's.
    assert!(
        !home
            .join("Library/Application Support/nu9ve/engines/nuun.json")
            .exists()
    );
    drop(child.stdin.take());
    child.wait().unwrap();
}
