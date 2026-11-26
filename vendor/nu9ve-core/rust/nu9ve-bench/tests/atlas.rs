//! nu9ve_bench::atlas against scripts/bench-from-atlas.py: the same runs on data/bench-fixtures/atlas (the
//! script's output is the `atlas` section of data/bench-examples-v1.json), and the edge cases of a made-up repo.

use nu9ve_bench::atlas::translate;
use nu9ve_bench::{Comparison, compare, run_problem};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn core_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn script_runs() -> Vec<Value> {
    let path = core_root().join("data/bench-examples-v1.json");
    let examples: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    examples["atlas"]["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| serde_json::from_str(line.as_str().unwrap()).unwrap())
        .collect()
}

/// Numbers by value (Python writes `1260000`, Rust may write `1260000.0`) and quality in metric order (the
/// flattening follows each parser's key order).
fn normalized(value: &Value) -> Value {
    match value {
        Value::Number(n) => json!(n.as_f64()),
        Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map {
                let mut item = normalized(item);
                if key == "quality"
                    && let Some(list) = item.as_array_mut()
                {
                    list.sort_by(|a, b| a["metric"].to_string().cmp(&b["metric"].to_string()));
                }
                out.insert(key.clone(), item);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

#[test]
fn the_fixture_translates_like_the_script() {
    let root = core_root().join("data/bench-fixtures/atlas");
    let got = translate(&root);
    assert_eq!(got.problems, Vec::<(String, String)>::new());
    let want = script_runs();
    let ids = |runs: &[Value]| -> Vec<String> {
        runs.iter()
            .map(|r| r["run_id"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(ids(&got.runs), ids(&want));
    for (a, b) in got.runs.iter().zip(&want) {
        assert_eq!(normalized(a), normalized(b), "{}", b["source"]);
        assert_eq!(run_problem(a), None);
    }
    // Atlas's own log wins over the translation of the same origin, and the milestones are quality-only runs.
    assert_eq!(got.runs[0]["source"]["kind"], "imported");
    assert_eq!(got.runs[0]["cache"], "cold");
    let milestones: Vec<&Value> = got
        .runs
        .iter()
        .filter(|r| r["suite"] == "atlas-quality")
        .collect();
    assert_eq!(milestones.len(), 7);
    assert!(milestones.iter().all(|r| r.get("metrics").is_none()));
    assert!(
        milestones
            .iter()
            .any(|r| r["case"] == "gravity/auto-v6-plus" && r["at"] == "2026-11-22")
    );
    assert!(
        milestones
            .iter()
            .any(|r| r["case"] == "gravity/gr-v3" && r["at"] == "2026-11-23")
    );
    // Every file read is listed, for lineage.
    assert!(got.files.iter().any(|f| f.ends_with("evolucion.json")));
    assert!(got.files.iter().any(|f| f.ends_with("atlas.ndjson")));
    assert!(got.files.iter().any(|f| f.ends_with("2026-11.md")));
    // Translating twice gives the same ids.
    let again = translate(&root);
    assert_eq!(ids(&again.runs), ids(&got.runs));
}

#[test]
fn two_milestones_compare_without_a_speedup() {
    let runs = translate(&core_root().join("data/bench-fixtures/atlas")).runs;
    let regla = runs.iter().find(|r| r["case"] == "evidence/regla").unwrap();
    let mut later = regla.clone();
    later["run_id"] = json!("0".repeat(32));
    assert_eq!(
        compare(regla, &later),
        Comparison::Comparable {
            speedup: None,
            flags: vec!["dirty"]
        }
    );
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push((path.clone(), fs::read(path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_broken_repo_reports_and_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "config/bench.json",
        "{\"baselines\": [7, {\"stage\": \"poses\"}]}",
    );
    write(root, "derived/bench/poses/a/broken.json", "{not json");
    write(
        root,
        "derived/bench/poses/a/ok.json",
        r#"{"stage":"poses","dataset":"a","engine":"odm","date":"2026-11-01","time_s":12.5,"status":"oom"}"#,
    );
    write(
        root,
        "derived/bench/atlas.ndjson",
        "{\"contract\":\"nu9ve-bench/1\"}\n\n{\"half\":",
    );
    write(
        root,
        "analysis/history/evolucion.json",
        r#"{"evidence":[{"key":"x","iteration":1,"auc":null},{"key":"y","iteration":2,"auc":0.6}],"gravity":"no"}"#,
    );
    let before = tree(root);
    let got = translate(root);
    assert_eq!(tree(root), before, "the translator never writes");

    let problems: Vec<(&str, &str)> = got
        .problems
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    assert!(problems.contains(&("derived/bench/atlas.ndjson:1", "missing_field")));
    assert!(problems.contains(&("derived/bench/poses/a/broken.json", "not_json")));
    assert!(problems.contains(&("config/bench.json#baselines/0", "not_object")));
    assert!(problems.contains(&("config/bench.json#baselines/1", "missing_field")));

    // A failed Atlas run keeps its status; the milestone with every metric null is skipped; the other one is
    // dated by the file, since there is no log.
    let failed = got
        .runs
        .iter()
        .find(|r| r["suite"] == "atlas-pipeline")
        .unwrap();
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["error"]["code"], "atlas_oom");
    let milestones: Vec<&Value> = got
        .runs
        .iter()
        .filter(|r| r["suite"] == "atlas-quality")
        .collect();
    assert_eq!(milestones.len(), 1);
    let at = milestones[0]["at"].as_str().unwrap();
    assert_eq!(at.len(), 10);
    assert!(at.starts_with("20"));
}

#[test]
fn an_empty_folder_has_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let got = translate(dir.path());
    assert!(got.runs.is_empty() && got.problems.is_empty() && got.files.is_empty());
}
