//! The socket client refuses an endpoint that is not private before sending anything.
#![cfg(any(target_os = "macos", target_os = "linux"))]

use nu9ve_ipc::{EngineRef, Record, Transport, request};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::time::Duration;

fn record(path: std::path::PathBuf) -> Record {
    Record::for_current_process(
        "alpha",
        "1.0.0",
        Transport::Unix { path },
        EngineRef {
            contract: "nu9ve-engine/1".into(),
            protocol: "test/1".into(),
            engine: "rust".into(),
            version: "0.1.0".into(),
        },
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn a_group_readable_socket_is_unsafe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("engine.sock");
    let _listener = UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).unwrap();
    let error = request(
        &record(path),
        "engine_info",
        None,
        None,
        Duration::from_secs(1),
    )
    .unwrap_err();
    assert_eq!(error.code, "unsafe_endpoint");
}

#[test]
fn no_socket_is_an_unavailable_engine() {
    let dir = tempfile::tempdir().unwrap();
    let error = request(
        &record(dir.path().join("none.sock")),
        "engine_info",
        None,
        None,
        Duration::from_secs(1),
    )
    .unwrap_err();
    assert_eq!(error.code, "engine_unavailable");
}
