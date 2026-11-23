//! nu9ve-export-v1 in Rust against data/export-examples-v1.json, written by the Python reference.

use nu9ve_mcp::export::{read_status, reply_problem};
use serde_json::Value;
use std::path::Path;

const MAX: u64 = 64 * 1024 * 1024;

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/export-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("examples")).expect("JSON")
}

fn parse(text: &Value) -> Value {
    serde_json::from_str(text.as_str().expect("text")).expect("JSON")
}

#[test]
fn valid_replies_read_as_the_reference() {
    for case in examples()["valid"].as_array().unwrap() {
        let dataset = case["dataset"].as_str().unwrap();
        let reply = parse(&case["text"]);
        assert_eq!(
            reply_problem(dataset, &reply, MAX),
            None,
            "{}",
            case["name"]
        );
        assert_eq!(
            read_status(dataset, &reply, MAX),
            case["reads_as"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn invalid_replies_name_the_same_problem_and_wait() {
    let examples = examples();
    let cases = examples["invalid"].as_array().unwrap();
    assert!(cases.len() >= 15);
    for case in cases {
        let dataset = case["dataset"].as_str().unwrap();
        let reply = parse(&case["text"]);
        assert_eq!(
            reply_problem(dataset, &reply, MAX),
            case["problem"].as_str(),
            "{}",
            case["name"]
        );
        assert_eq!(
            read_status(dataset, &reply, MAX),
            "pending",
            "{}",
            case["name"]
        );
    }
}
