//! Differential test of `discover` against the Python oracle (rust-engine.md §7).
//! `tests/fixtures/engine-golden/recipe.json` comes from `scripts/engine_golden.py` (Python, on the Mac today).
//! Ignored by default so a machine without the file does not pass silently.

use serde_json::Value;
use std::path::PathBuf;

#[test]
#[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
fn discover_matches_python_on_the_same_card_trees() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/engine-golden/recipe.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing {}: run scripts/engine_golden.py", path.display()));
    let golden: Value = serde_json::from_str(&text).unwrap();
    let cases = golden["discover"]
        .as_array()
        .expect("discover cases in the golden file");
    assert!(!cases.is_empty());
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        for (rel, content) in case["tree"].as_object().unwrap() {
            let full = root.path().join(rel);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content.as_str().unwrap()).unwrap();
        }
        let source = root.path().display().to_string();
        let mut found =
            nuun_media::scan::discover(None, &serde_json::json!({ "source": source })).unwrap();
        // Rust names what it cannot look up yet (`mission`, only with an archive); these cases have none.
        found.as_object_mut().unwrap().remove("not_ported");
        assert_eq!(found, case["discover"], "{}", case["tree"]);
    }
}
