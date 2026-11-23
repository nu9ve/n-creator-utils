//! nu9ve-device-v1 in Rust against data/device-examples-v1.json, written by the Python reference.

use nu9ve_device::{
    Outcome, default_name, file_for, file_problem, host_check, load_or_create, rename, state,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/device-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn files_have_the_same_problems() {
    for case in examples()["files"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        assert_eq!(
            file_problem(text.as_bytes()),
            case["problem"].as_str(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn host_checks_and_states_match() {
    let examples = examples();
    for case in examples["host_check"].as_array().unwrap() {
        assert_eq!(
            host_check(
                case["device_id"].as_str().unwrap(),
                case["platform_id"].as_str().unwrap()
            ),
            case["host_check"]
        );
    }
    for case in examples["states"].as_array().unwrap() {
        let text = case["text"].as_str().map(str::as_bytes);
        assert_eq!(
            state(text, case["platform_id"].as_str()).id(),
            case["state"],
            "{}",
            case["name"]
        );
    }
    for case in examples["default_names"].as_array().unwrap() {
        assert_eq!(
            default_name(
                case["os"].as_str().unwrap(),
                case["device_id"].as_str().unwrap()
            ),
            case["name"]
        );
    }
}

#[test]
fn locations_match() {
    for case in examples()["locations"].as_array().unwrap() {
        let env = case["env"].as_object().unwrap().clone();
        let got = file_for(case["platform"].as_str().unwrap(), &|k| {
            env.get(k).and_then(Value::as_str).map(str::to_owned)
        });
        assert_eq!(got, case["file"].as_str().map(PathBuf::from), "{:?}", case);
    }
}

#[test]
fn a_device_is_created_once_renamed_and_replaced_when_copied() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nu9ve/device.json");
    let (first, outcome) = load_or_create(&path, Some("PLATFORM-A")).unwrap();
    assert_eq!(outcome, Outcome::Created);
    assert_eq!(file_problem(&std::fs::read(&path).unwrap()), None);
    let (again, outcome) = load_or_create(&path, Some("PLATFORM-A")).unwrap();
    assert_eq!(
        (outcome, &again["device_id"]),
        (Outcome::Existing, &first["device_id"])
    );
    let renamed = rename(&path, "M4").unwrap();
    assert_eq!(
        (renamed["name"].as_str(), &renamed["device_id"]),
        (Some("M4"), &first["device_id"])
    );
    // The same file on another machine: a new device that remembers where it came from.
    let (copy, outcome) = load_or_create(&path, Some("PLATFORM-B")).unwrap();
    assert_eq!(outcome, Outcome::ReplacedCopy);
    assert_ne!(copy["device_id"], first["device_id"]);
    assert_eq!(copy["previous_device_id"], first["device_id"]);
    assert_eq!(copy["name"], "M4");
    // An invalid file is never overwritten.
    std::fs::write(&path, b"{").unwrap();
    assert!(load_or_create(&path, Some("PLATFORM-B")).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{");
}
