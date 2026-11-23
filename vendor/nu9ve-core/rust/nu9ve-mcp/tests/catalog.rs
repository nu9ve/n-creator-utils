//! nu9ve-data-v1 in Rust against data/data-examples-v1.json, written by the Python reference.

use nu9ve_mcp::catalog::{Reply, aggregate, catalog_problem, mcp_view};
use serde_json::Value;
use std::path::Path;

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/data-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("examples")).expect("JSON")
}

fn parse(text: &Value) -> Value {
    serde_json::from_str(text.as_str().expect("text")).expect("JSON")
}

#[test]
fn valid_catalogs_have_no_problem() {
    let examples = examples();
    for case in examples["valid"].as_array().unwrap() {
        let app = case["app"].as_str().unwrap();
        assert_eq!(
            catalog_problem(app, &parse(&case["text"])),
            None,
            "{}",
            case["name"]
        );
    }
}

#[test]
fn invalid_catalogs_name_the_same_problem() {
    let examples = examples();
    let cases = examples["invalid"].as_array().unwrap();
    assert!(cases.len() >= 25);
    for case in cases {
        let app = case["app"].as_str().unwrap();
        assert_eq!(
            catalog_problem(app, &parse(&case["text"])),
            case["problem"].as_str(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn aggregation_matches_the_reference() {
    let examples = examples();
    let replies: Vec<(String, Reply)> = examples["aggregate"]["replies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let app = r["app"].as_str().unwrap().to_owned();
            let reply = if r["text"].is_null() {
                Reply::NotOpen
            } else {
                Reply::Catalog(parse(&r["text"]))
            };
            (app, reply)
        })
        .collect();
    assert_eq!(aggregate(&replies), examples["aggregate"]["expected"]);
}

#[test]
fn what_an_agent_sees_matches_the_reference() {
    let examples = examples();
    for case in examples["mcp"].as_array().unwrap() {
        let app = case["app"].as_str().unwrap();
        assert_eq!(mcp_view(app, &parse(&case["dataset"])), parse(&case["mcp"]));
    }
}
