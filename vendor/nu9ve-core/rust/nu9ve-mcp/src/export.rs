//! nu9ve-export-v1: asking the owner engine of a dataset for its bytes, ready in a file (§1, §2). Only the
//! agent calls it, to answer a request of a paired machine; `nu9ve mcp` never does (§4).
//! The Python reference is scripts/export-examples.py; the tests run its examples.

use crate::catalog::{valid_dataset_id, valid_uuid4};
use nu9ve_ipc::{DEFAULT_TIMEOUT, Record};
use serde_json::{Value, json};

pub const CONTRACT: &str = "nu9ve-export/1";
pub const CAPABILITY: &str = "data_export_v1";
const MAX_PATH: usize = 4096;
const MAX_SAFE: u64 = (1 << 53) - 1;

fn absolute(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with('/')
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'\\' | b'/'))
}

fn path_text_ok(path: &str) -> bool {
    path.len() <= MAX_PATH && !path.contains('\0')
}

fn media_type_ok(text: &str) -> bool {
    let part = |p: &str, longest: usize| {
        let b = p.as_bytes();
        (1..=longest).contains(&b.len())
            && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
            && b.iter().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'+' | b'-')
            })
    };
    text.split_once('/')
        .is_some_and(|(kind, sub)| part(kind, 64) && part(sub, 128))
}

fn sha256_ok(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// §2 and exchange §7: `{volume_id, path}`, a disk and a path inside it, or no disk and a path here.
pub fn where_problem(where_: &Value) -> Option<&'static str> {
    let Some(map) = where_.as_object() else {
        return Some("bad_where");
    };
    let (Some(volume), Some(path)) = (
        map.get("volume_id"),
        map.get("path").and_then(Value::as_str),
    ) else {
        return Some("bad_where");
    };
    if map.len() != 2 || !path_text_ok(path) {
        return Some("bad_where");
    }
    match volume {
        Value::Null => (!absolute(path)).then_some("bad_where"),
        Value::String(id) if valid_uuid4(id) => (path.starts_with('/')
            || path.contains('\\')
            || path.split('/').any(|p| matches!(p, "." | "..")))
        .then_some("bad_where"),
        _ => Some("bad_where"),
    }
}

/// §2: `None` when `reply` (the data of a `data_export` reply) is valid for a request of `dataset` with
/// `max_bytes`, otherwise why not. An unknown status is valid: the client reads it as `not_found`.
pub fn reply_problem(dataset: &str, reply: &Value, max_bytes: u64) -> Option<&'static str> {
    if !reply.is_object() {
        return Some("not_an_object");
    }
    if reply["contract"] != CONTRACT {
        return Some("bad_contract");
    }
    if reply["dataset"] != dataset {
        return Some("other_dataset");
    }
    let Some(status) = reply["status"].as_str() else {
        return Some("bad_status");
    };
    match status {
        "ready" => {
            if !reply["path"]
                .as_str()
                .is_some_and(|p| path_text_ok(p) && absolute(p))
            {
                return Some("bad_path");
            }
            let Some(bytes) = reply["bytes"].as_u64() else {
                return Some("bad_bytes");
            };
            if bytes > max_bytes {
                return Some("over_max_bytes");
            }
            if !reply["sha256"].as_str().is_some_and(sha256_ok) {
                return Some("bad_sha256");
            }
            if !reply["media_type"].as_str().is_some_and(media_type_ok) {
                return Some("bad_media_type");
            }
        }
        "too_large" => {
            if reply
                .get("bytes")
                .is_some_and(|b| b.as_u64().is_none_or(|n| n > MAX_SAFE))
            {
                return Some("bad_bytes");
            }
            if let Some(problem) = where_problem(&reply["where"]) {
                return Some(problem);
            }
        }
        _ => {}
    }
    None
}

/// §2: what a client does with a reply: its status, `not_found` for one it does not know, and
/// `pending` for an invalid one.
pub fn read_status(dataset: &str, reply: &Value, max_bytes: u64) -> &'static str {
    if reply_problem(dataset, reply, max_bytes).is_some() {
        return "pending";
    }
    match reply["status"].as_str() {
        Some("ready") => "ready",
        Some("too_large") => "too_large",
        Some("pending") => "pending",
        _ => "not_found",
    }
}

/// Asks a live engine with `data_export_v1` for `dataset` (an id of its catalog), after checking its
/// `engine_info`. An error, or an engine without the capability, is `None`: the request waits (§3).
pub fn fetch(record: &Record, dataset: &str, max_bytes: u64) -> Option<Value> {
    if !record.has_capability(CAPABILITY) || !valid_dataset_id(dataset) {
        return None;
    }
    crate::engines::check(record).ok()?;
    nu9ve_ipc::request(
        record,
        "data_export",
        Some(json!({"dataset": dataset, "max_bytes": max_bytes})),
        None,
        DEFAULT_TIMEOUT,
    )
    .ok()
}
