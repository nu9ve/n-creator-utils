//! nu9ve-profile-v1 in Rust against data/profile-examples-v1.json, written by the Python reference.

use nu9ve_profiles::{
    Error, apply, change_at, create_profile_change, delete_profile_change, file_for, file_problem,
    for_app, new_document, read, remove_app_change, upsert_app_change,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/profile-examples-v1.json");
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
fn sequences_give_the_same_documents_and_refusals() {
    for case in examples()["sequences"].as_array().unwrap() {
        let mut doc = match case["start"].as_str() {
            Some(text) => serde_json::from_str(text).unwrap(),
            None => new_document(case["device_id"].as_str(), "2026-11-25T20:00:00Z"),
        };
        for step in case["steps"].as_array().unwrap() {
            match apply(&doc, &step["change"], step["now"].as_str().unwrap()) {
                Ok(next) => {
                    assert!(step["error"].is_null(), "{}: {}", case["name"], step);
                    doc = next;
                }
                Err(code) => assert_eq!(step["error"], code, "{}: {}", case["name"], step),
            }
        }
        assert_eq!(doc, case["result"], "{}", case["name"]);
    }
}

#[test]
fn listings_match() {
    for case in examples()["listings"].as_array().unwrap() {
        let doc: Value = serde_json::from_str(case["text"].as_str().unwrap()).unwrap();
        let ids: Vec<String> = for_app(Some(&doc), case["app"].as_str().unwrap())
            .into_iter()
            .map(|p| p.profile_id)
            .collect();
        assert_eq!(json!(ids), case["profiles"]);
    }
}

#[test]
fn locations_match() {
    for case in examples()["locations"].as_array().unwrap() {
        let env = case["env"].as_object().unwrap().clone();
        let got = file_for(case["platform"].as_str().unwrap(), &|k| {
            env.get(k).and_then(Value::as_str).map(str::to_owned)
        });
        assert_eq!(got, case["file"].as_str().map(PathBuf::from), "{case:?}");
    }
}

const DEVICE: &str = "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f";

#[test]
fn the_file_is_created_private_changed_and_never_overwritten_when_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nu9ve/profiles.json");
    assert!(read(&path).unwrap().is_none());
    let now = "2026-11-25T20:00:00Z";
    change_at(
        &path,
        &create_profile_change("e64", "Erik", "person"),
        Some(DEVICE),
        now,
    )
    .unwrap();
    let doc = change_at(
        &path,
        &upsert_app_change("e64", "alpha", "/qa/alpha/engine", true, Some("full")),
        None,
        now,
    )
    .unwrap();
    assert_eq!(doc["device_id"], DEVICE);
    assert_eq!(read(&path).unwrap(), Some(doc));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }
    // Refused: nothing is written.
    let before = std::fs::read(&path).unwrap();
    let refused = change_at(&path, &delete_profile_change("e64"), None, now).unwrap_err();
    assert_eq!(refused.code(), "profile_in_use");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    change_at(&path, &remove_app_change("e64", "alpha"), None, now).unwrap();
    change_at(&path, &delete_profile_change("e64"), None, now).unwrap();
    let again = change_at(
        &path,
        &create_profile_change("e64", "Otro", "person"),
        None,
        now,
    );
    assert_eq!(again.unwrap_err().code(), "profile_retired");
    // An invalid file is left as it is.
    std::fs::write(&path, b"{").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(matches!(read(&path), Err(Error::Invalid("not_json"))));
    let refused = change_at(&path, &create_profile_change("a", "A", "team"), None, now);
    assert_eq!(refused.unwrap_err().code(), "invalid");
    assert_eq!(std::fs::read(&path).unwrap(), b"{");
}

#[cfg(unix)]
#[test]
fn a_file_others_can_read_is_not_trusted() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.json");
    let doc = new_document(None, "2026-11-25T20:00:00Z");
    std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        read(&path),
        Err(Error::Invalid("unsafe_permissions"))
    ));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(read(&path).unwrap(), Some(doc));
}

#[test]
fn concurrent_writers_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nu9ve/profiles.json");
    let now = "2026-11-25T20:00:00Z";
    change_at(
        &path,
        &create_profile_change("e64", "Erik", "person"),
        Some(DEVICE),
        now,
    )
    .unwrap();
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let path = path.clone();
            std::thread::spawn(move || {
                // Each thread is one app with its own key, plus a profile of its own.
                let app = format!("app{i}");
                change_at(
                    &path,
                    &upsert_app_change("e64", &app, &format!("/qa/{app}"), false, None),
                    None,
                    now,
                )
                .unwrap();
                change_at(
                    &path,
                    &create_profile_change(&format!("p{i}"), "P", "team"),
                    None,
                    now,
                )
                .unwrap();
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let doc = read(&path).unwrap().unwrap();
    assert_eq!(doc["profiles"].as_array().unwrap().len(), 9);
    assert_eq!(doc["profiles"][0]["apps"].as_object().unwrap().len(), 8);
    let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n != "profiles.json" && n != "profiles.json.lock")
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}
