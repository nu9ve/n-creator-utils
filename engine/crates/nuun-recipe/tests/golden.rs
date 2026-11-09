//! Differential test against the Python oracle (rust-engine.md §7, phase 0).
//!
//! `tests/fixtures/engine-golden/recipe.json` is written by `scripts/engine_golden.py` on a machine with Python
//! (the Mac today). Ignored by default so a machine without the file does not pass silently:
//! `cargo test --offline -- --ignored` runs it and fails if the file is missing.

use serde_json::Value;
use std::path::PathBuf;

fn golden() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/engine-golden/recipe.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "Missing {}: run `python3 scripts/engine_golden.py` first",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("golden file is JSON")
}

#[test]
#[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
fn canonical_json_matches_python_byte_for_byte() {
    for case in golden()["canonical"].as_array().unwrap() {
        let bytes = nu9ve_json::canonical(&case["value"]);
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            case["canonical"].as_str().unwrap()
        );
        assert_eq!(
            nu9ve_json::digest(&case["value"]),
            case["digest"].as_str().unwrap()
        );
    }
}

#[test]
#[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
fn template_and_recipe_rpc_match_python() {
    let golden = golden();
    // Same digest means template.json is an exact transcription of media/recipes.py TEMPLATE.
    assert_eq!(
        nuun_recipe::recipe_ref(&nuun_recipe::template()),
        golden["recipe_ref"]
    );
    assert_eq!(
        nuun_recipe::summary(&nuun_recipe::template(), 0),
        golden["recipe_summary"]
    );
}

#[test]
#[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
fn card_families_match_python_on_the_same_trees() {
    let golden = golden();
    let cases = golden["cards"]
        .as_array()
        .expect("cards in the golden file");
    assert!(!cases.is_empty());
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        for (folder, filled) in case["tree"].as_object().unwrap() {
            let path = root.path().join(folder);
            std::fs::create_dir_all(&path).unwrap();
            if filled.as_bool().unwrap() {
                std::fs::write(path.join("x"), b"1").unwrap();
            }
        }
        let recipe = nuun_recipe::template();
        let rule = nuun_recipe::card_rule(root.path(), &recipe).unwrap();
        assert_eq!(rule["family"], case["family"], "{}", case["tree"]);
        assert_eq!(
            rule["recommendation"], case["recommendation"],
            "{}",
            case["tree"]
        );
    }
}
