//! The registry against data/registry-examples-v1.json and a real folder.

use nu9ve_ipc::{EngineRef, Record, Transport, parse, scan, valid_file_name};
use serde_json::Value;
use std::path::Path;

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/registry-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("examples")).expect("JSON")
}

#[test]
fn records_match_the_examples() {
    let examples = examples();
    let cases = examples["records"].as_array().expect("records");
    assert!(cases.len() >= 20);
    for case in cases {
        if case.get("stat").is_some() {
            continue; // Permissions are checked on real files below.
        }
        let got = parse(
            case["file"].as_str().unwrap(),
            case["text"].as_str().unwrap().as_bytes(),
        )
        .err();
        assert_eq!(got, case["problem"].as_str(), "{}", case["name"]);
    }
}

#[test]
fn file_names_match_the_examples() {
    for case in examples()["names"].as_array().expect("names") {
        assert_eq!(
            valid_file_name(case["file"].as_str().unwrap()),
            case["listed"].as_bool().unwrap(),
            "{}",
            case["file"]
        );
    }
}

fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn record_for_me(app: &str) -> Option<Record> {
    Record::for_current_process(
        app,
        "1.0.0",
        Transport::Unix {
            path: "/tmp/does-not-matter.sock".into(),
        },
        EngineRef {
            contract: "nu9ve-engine/1".into(),
            protocol: "test/1".into(),
            engine: "rust".into(),
            version: "0.1.0".into(),
        },
        vec!["data_catalog_v1".into()],
    )
    .ok()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn live_stale_invalid_and_cleanup() {
    use nu9ve_ipc::{Registration, State};
    let dir = private_dir();
    let me = record_for_me("alpha").expect("record");
    let registration = Registration::register(dir.path(), &me).expect("register");
    // A second engine of the same app does not replace a live one.
    let mut other = me.clone();
    other.process_start_us += 1;
    let refused = Registration::register(dir.path(), &other).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists);

    // Same pid, other start: a reused pid, so stale.
    let mut reused = record_for_me("beta").unwrap();
    reused.process_start_us -= 1;
    std::fs::write(dir.path().join("beta.json"), reused.to_json().to_string()).unwrap();
    // Not a registry name, ignored.
    std::fs::write(dir.path().join(".alpha.json.x.tmp"), "{}").unwrap();
    // Wrong app inside.
    std::fs::write(dir.path().join("gamma.json"), me.to_json().to_string()).unwrap();
    #[cfg(unix)]
    for name in ["beta.json", "gamma.json"] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join(name),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }

    let entries = scan(dir.path()).expect("scan");
    let states: Vec<_> = entries.iter().map(|e| (e.app.as_str(), &e.state)).collect();
    assert_eq!(states.len(), 3);
    assert!(matches!(states[0], ("alpha", State::Live(r)) if *r == me));
    assert!(matches!(states[1], ("beta", State::Stale(_))));
    assert!(matches!(
        states[2],
        ("gamma", State::Invalid("app_mismatch"))
    ));

    // A stale record is replaced by the next engine of that app.
    drop(registration);
    assert!(!dir.path().join("alpha.json").exists());
    let beta = record_for_me("beta").unwrap();
    let _beta = Registration::register(dir.path(), &beta).expect("replaces stale");
    assert!(matches!(&scan(dir.path()).unwrap()[0].state, State::Live(r) if *r == beta));
}

#[cfg(unix)]
#[test]
fn unsafe_files_and_folders_are_not_trusted() {
    use nu9ve_ipc::State;
    use std::os::unix::fs::PermissionsExt;
    let dir = private_dir();
    let Some(me) = record_for_me("alpha") else {
        return;
    };
    let path = dir.path().join("alpha.json");
    std::fs::write(&path, me.to_json().to_string()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(
        scan(dir.path()).unwrap()[0].state,
        State::Invalid("unsafe_permissions")
    );

    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", &path).unwrap();
    assert_eq!(
        scan(dir.path()).unwrap()[0].state,
        State::Invalid("unsafe_permissions")
    );

    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(scan(dir.path()).is_err());
}

#[test]
fn a_missing_folder_is_an_empty_registry() {
    let dir = private_dir();
    assert!(scan(&dir.path().join("nothing")).unwrap().is_empty());
}

#[test]
fn a_written_record_reads_back() {
    let Some(me) = record_for_me("alpha") else {
        return;
    };
    let text = me.to_json().to_string();
    assert_eq!(parse("alpha.json", text.as_bytes()), Ok(me.clone()));
    // Minor version of 2026-11-25: the profile goes and comes back.
    let mut with_profile = me;
    with_profile.profile_id = Some("e64".into());
    let text = with_profile.to_json().to_string();
    assert!(text.contains(r#""profile_id":"e64""#));
    assert_eq!(parse("alpha.json", text.as_bytes()), Ok(with_profile));
}
