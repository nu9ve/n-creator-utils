//! nu9ve-bench-v1 in Rust against data/bench-examples-v1.json, written by the Python reference
//! (scripts/bench-examples.py with scripts/nu9ve_bench.py).

use nu9ve_bench::measure::{Measured, RunInfo, measure_fn, new_run};
use nu9ve_bench::{Comparison, append, compare, distribution, line_problem, read_log};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench-examples-v1.json");
    serde_json::from_str(&fs::read_to_string(path).expect("examples")).expect("JSON")
}

fn text(case: &Value, key: &str) -> String {
    case[key].as_str().expect("text").to_owned()
}

#[test]
fn valid_runs_have_no_problem() {
    let examples = examples();
    for case in examples["valid"].as_array().unwrap() {
        assert_eq!(line_problem(&text(case, "text")), None, "{}", case["name"]);
    }
}

#[test]
fn invalid_runs_name_the_same_problem() {
    let examples = examples();
    let cases = examples["invalid"].as_array().unwrap();
    assert!(cases.len() >= 40);
    for case in cases {
        assert_eq!(
            line_problem(&text(case, "text")),
            case["problem"].as_str(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn distributions_match_the_reference() {
    for case in examples()["distributions"].as_array().unwrap() {
        let samples: Vec<f64> = case["samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_f64().unwrap())
            .collect();
        let got = distribution(&samples);
        for key in ["median", "p95", "min", "max"] {
            assert_eq!(
                got[key].as_f64(),
                case[key].as_f64(),
                "{key} of {samples:?}"
            );
        }
    }
}

#[test]
fn logs_read_like_the_reference() {
    for case in examples()["logs"].as_array().unwrap() {
        let log = read_log(&text(case, "text"));
        let ids: Vec<&str> = log
            .runs
            .iter()
            .map(|r| r["run_id"].as_str().unwrap())
            .collect();
        let want: Vec<&str> = case["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        assert_eq!(ids, want, "{}", case["name"]);
        let problems: Vec<Value> = log
            .problems
            .iter()
            .map(|p| json!({"line": p.line, "problem": p.problem}))
            .collect();
        assert_eq!(Value::from(problems), case["problems"], "{}", case["name"]);
        assert_eq!(
            json!(log.duplicates),
            case["duplicates"],
            "{}",
            case["name"]
        );
        assert_eq!(
            json!(log.partial_last_line),
            case["partial_last_line"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn comparisons_match_the_reference() {
    for case in examples()["comparisons"].as_array().unwrap() {
        let parse = |key: &str| serde_json::from_str::<Value>(&text(case, key)).unwrap();
        let got = match compare(&parse("baseline"), &parse("candidate")) {
            Comparison::NotComparable { reason } => json!({"comparable": false, "reason": reason}),
            Comparison::Comparable { speedup, flags } => {
                json!({"comparable": true, "speedup": speedup, "flags": flags})
            }
        };
        assert_eq!(got, case["result"], "{}", case["name"]);
    }
}

#[test]
fn the_atlas_translation_is_valid() {
    let examples = examples();
    let runs = examples["atlas"]["runs"].as_array().unwrap();
    assert!(!runs.is_empty());
    for run in runs {
        assert_eq!(line_problem(run.as_str().unwrap()), None);
    }
}

fn info() -> RunInfo {
    RunInfo {
        suite: "core-selftest".into(),
        case: "rust/sum".into(),
        category: "cpu".into(),
        subject: json!({"app": "core", "engine": "rust", "version": "0.1.0", "commit": null, "dirty": null}),
        machine: None,
        inputs: vec![],
        params: Some(json!({"n": 100000})),
        cache: "warm",
        privacy: "public",
        quality: None,
        note: None,
    }
}

#[test]
fn a_measured_run_is_valid_and_appends() {
    let measured = measure_fn(5, 1, || {
        std::hint::black_box((0..100_000u64).sum::<u64>());
    });
    assert_eq!(measured.status, "ok");
    let run = new_run(info(), measured).expect("valid run");
    assert_eq!(
        run["metrics"]["wall_ms"]["samples"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(run["machine"]["arch"], json!(std::env::consts::ARCH));

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("core-selftest.ndjson");
    append(&log, &run).unwrap();
    append(&log, &run).unwrap();
    let read = read_log(&fs::read_to_string(&log).unwrap());
    assert_eq!(read.runs.len(), 1);
    assert_eq!(read.duplicates.len(), 1);
    assert!(!read.partial_last_line);
}

#[test]
fn an_invalid_run_never_reaches_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("bad.ndjson");
    let mut run = new_run(info(), measure_fn(1, 0, || {})).unwrap();
    run["cache"] = json!("hot");
    assert!(append(&log, &run).is_err());
    assert!(!log.exists());
    let bad = Measured {
        repeats: 0,
        warmup: 0,
        status: "ok",
        metrics: None,
        error: None,
    };
    assert!(new_run(info(), bad).is_err());
}

#[cfg(unix)]
#[test]
fn a_command_is_measured_as_a_process() {
    use nu9ve_bench::measure::measure_command;
    use std::process::Command;

    let ok = measure_command(3, 1, || Command::new("true")).unwrap();
    assert_eq!(ok.status, "ok");
    let metrics = ok.metrics.unwrap();
    assert_eq!(metrics["wall_ms"]["samples"].as_array().unwrap().len(), 3);
    assert_eq!(metrics["cpu_ms"]["samples"].as_array().unwrap().len(), 3);
    assert!(metrics["peak_memory"]["bytes"].as_u64().unwrap() > 0);

    let failed = measure_command(3, 0, || Command::new("false")).unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.repeats, 0);
    assert!(failed.metrics.is_none());
    assert_eq!(failed.error.unwrap()["code"], "exit_status");
}
