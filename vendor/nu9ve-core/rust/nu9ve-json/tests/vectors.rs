//! nu9ve-json-v1 vectors (`data/json-vectors-v1.json`), written by CPython's json module with
//! `scripts/json-vectors.py`, so this test checks the Rust reader and writer against the oracle without Python.
//!
//! The path is the same in nu9ve/core and in an app's pinned copy (`vendor/nu9ve-core/`): `../../data`.

use serde_json::Value;
use std::path::PathBuf;

fn vectors() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/json-vectors-v1.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "Missing {}: an app pins `data` with `rust/nu9ve-json` (sync-core.sh)",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("vectors are JSON")
}

fn cases<'a>(vectors: &'a Value, key: &str) -> &'a Vec<Value> {
    let list = vectors[key].as_array().expect(key);
    assert!(!list.is_empty(), "{key} is empty");
    list
}

#[test]
fn canonical_bytes_and_digests_match_the_oracle() {
    let vectors = vectors();
    assert_eq!(vectors["contract"], "nu9ve-json-v1");
    for case in cases(&vectors, "accepted") {
        let name = case["name"].as_str().unwrap();
        let value: Value = serde_json::from_str(case["input"].as_str().unwrap())
            .unwrap_or_else(|e| panic!("{name}: the reader rejected an accepted input: {e}"));
        let bytes = nu9ve_json::canonical(&value);
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            case["canonical"].as_str().unwrap(),
            "{name}"
        );
        // Canonical text reads back to the same bytes (nu9ve-json-v1 §8).
        let again: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(nu9ve_json::canonical(&again), bytes, "{name} read back");
        assert_eq!(
            nu9ve_json::digest(&value),
            case["digest"].as_str().unwrap(),
            "{name}"
        );
    }
}

#[test]
fn rejected_inputs_never_reach_canonical_bytes() {
    let vectors = vectors();
    for case in cases(&vectors, "rejected") {
        let name = case["name"].as_str().unwrap();
        let read = serde_json::from_str::<Value>(case["input"].as_str().unwrap());
        assert!(read.is_err(), "{name}: read as {:?}", read.ok());
    }
}

#[test]
fn known_limits_still_differ_from_the_oracle() {
    // nu9ve-json-v1 §8: producers never write these. If one starts to match, move it to `accepted`.
    let vectors = vectors();
    for case in cases(&vectors, "limits") {
        let name = case["name"].as_str().unwrap();
        let value: Value = serde_json::from_str(case["input"].as_str().unwrap()).unwrap();
        let bytes = nu9ve_json::canonical(&value);
        assert_ne!(
            std::str::from_utf8(&bytes).unwrap(),
            case["canonical"].as_str().unwrap(),
            "{name} now matches the oracle: move it to accepted in nu9ve/core"
        );
    }
}

fn namespace(text: &str) -> [u8; 16] {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    let mut bytes = [0u8; 16];
    for (i, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    bytes
}

#[test]
fn name_based_ids_match_the_oracle() {
    let vectors = vectors();
    for case in cases(&vectors, "uuid5") {
        let ns = namespace(case["namespace"].as_str().unwrap());
        let name = case["name"].as_str().unwrap();
        assert_eq!(
            nu9ve_json::uuid5(&ns, name),
            case["uuid"].as_str().unwrap(),
            "{name:?}"
        );
    }
    assert_eq!(
        namespace("6ba7b811-9dad-11d1-80b4-00c04fd430c8"),
        nu9ve_json::NAMESPACE_URL
    );
    let content = &vectors["content_id"];
    let digest = nu9ve_json::digest(&content["value"]);
    assert_eq!(digest, content["digest"].as_str().unwrap());
    assert_eq!(
        nu9ve_json::uuid5(&nu9ve_json::NAMESPACE_URL, &digest),
        content["uuid"].as_str().unwrap()
    );
}

#[test]
fn envelopes_are_the_two_documented_shapes() {
    let vectors = vectors();
    let find = |name: &str| {
        cases(&vectors, "accepted")
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()["canonical"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let ok = nu9ve_json::ok_envelope(
        &serde_json::json!(7),
        serde_json::json!({"items": [], "total": 0}),
    );
    assert_eq!(nu9ve_json::canonical(&ok), find("envelope_ok").into_bytes());
    let error = nu9ve_json::error_envelope(
        &Value::Null,
        &nu9ve_json::Error::new("read_only", "No se puede escribir."),
    );
    assert_eq!(
        nu9ve_json::canonical(&error),
        find("envelope_error").into_bytes()
    );
}
