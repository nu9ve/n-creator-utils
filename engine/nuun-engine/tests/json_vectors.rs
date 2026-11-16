//! nüün uses its pinned nu9ve-json as nu9ve/core tests it: the canonical bytes, digests and content ids of
//! `vendor/nu9ve-core/data/json-vectors-v1.json` (contracts/nu9ve-json-v1.md), built with this workspace's
//! own feature set. A serde_json feature turned on here (`float_roundtrip`, `arbitrary_precision`) would change
//! digests of reviewed plans; this test is where that shows.

use serde_json::Value;
use std::path::PathBuf;

fn vectors() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/nu9ve-core/data/json-vectors-v1.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "Missing {}: pin nu9ve/core with vendor/nu9ve-core/scripts/sync-core.sh",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("vectors are JSON")
}

#[test]
fn pinned_nu9ve_json_matches_the_shared_vectors() {
    let vectors = vectors();
    assert_eq!(vectors["contract"], "nu9ve-json-v1");
    let accepted = vectors["accepted"].as_array().unwrap();
    assert!(!accepted.is_empty());
    for case in accepted {
        let name = case["name"].as_str().unwrap();
        let value: Value = serde_json::from_str(case["input"].as_str().unwrap()).unwrap();
        assert_eq!(
            std::str::from_utf8(&nu9ve_json::canonical(&value)).unwrap(),
            case["canonical"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(nu9ve_json::digest(&value), case["digest"], "{name}");
    }
    for case in vectors["rejected"].as_array().unwrap() {
        let read = serde_json::from_str::<Value>(case["input"].as_str().unwrap());
        assert!(read.is_err(), "{}", case["name"]);
    }
    let content = &vectors["content_id"];
    let digest = nu9ve_json::digest(&content["value"]);
    assert_eq!(digest, content["digest"].as_str().unwrap());
    assert_eq!(
        nu9ve_json::uuid5(&nu9ve_json::NAMESPACE_URL, &digest),
        content["uuid"].as_str().unwrap()
    );
}
