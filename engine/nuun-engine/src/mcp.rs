//! F5 (d), 2026-11-27: `nuun-engine media mcp`, the read-only MCP stdio server of `media/mcp.py` in Rust.
//!
//! The same JSON-RPC lines, the same eight tools and the same answers: `tests/test_media_mcp_rust.py` runs one
//! conversation through both. No HTTP listener and no tool that writes: the eight tools name eight reads, and
//! only those reach [`Engine::dispatch`]. It is [`Mode::Once`] because `resolve` and `job` are one-shot reads
//! that `rpc-serve` does not answer.
use super::{Engine, Mode};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

/// A line longer than this ends the session, as in Python (16 MiB).
const MAX_LINE: usize = 16 * 1024 * 1024;
const PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// `(tool, method, properties, required)`, Python's `TOOLS` in its order.
fn tools() -> Vec<(&'static str, &'static str, Value, Vec<&'static str>)> {
    let text = || json!({"type": "string"});
    vec![
        (
            "media_library_search",
            "library",
            json!({"project": text(), "work_state": {"enum": ["pending", "ready", "skipped"]}}),
            vec![],
        ),
        (
            "media_asset_resolve",
            "resolve",
            json!({"asset_id": text()}),
            vec!["asset_id"],
        ),
        ("media_jobs", "jobs", json!({}), vec![]),
        (
            "media_job_status",
            "job",
            json!({"job_id": text()}),
            vec!["job_id"],
        ),
        ("media_volumes", "volumes", json!({}), vec![]),
        (
            "media_storage_overview",
            "storage_overview",
            json!({}),
            vec![],
        ),
        (
            "media_storage_collections",
            "storage_collections",
            json!({}),
            vec![],
        ),
        (
            "media_storage_tree",
            "storage_tree",
            json!({"target": text(), "path": text()}),
            vec!["target"],
        ),
    ]
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// One message; `None` for a notification. `initialized` is whether an `initialize` came before.
fn handle(engine: &Engine, message: &Value, initialized: bool) -> Option<Value> {
    let (Some(fields), Some(method)) = (
        message
            .as_object()
            .filter(|m| m.get("jsonrpc") == Some(&json!("2.0"))),
        message["method"].as_str(),
    ) else {
        return Some(error(&Value::Null, -32600, "Invalid request"));
    };
    let id = fields.get("id")?.clone();
    let result = |value: Value| Some(json!({"jsonrpc": "2.0", "id": id, "result": value}));
    if method == "initialize" {
        let asked = message["params"]["protocolVersion"].as_str();
        let version = asked
            .filter(|v| PROTOCOLS.contains(v))
            .unwrap_or(PROTOCOLS[0]);
        return result(json!({
            "protocolVersion": version,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "ncreator-media", "version": "0.2.0"},
            "instructions": "Read-only local catalog. Import/cleanup requires the reviewed CLI/native plan flow.",
        }));
    }
    if !initialized {
        return Some(error(&id, -32000, "Initialize first"));
    }
    match method {
        "ping" => result(json!({})),
        "tools/list" => result(
            json!({"tools": tools().into_iter().map(|(name, target, properties, required)| json!({
            "name": name,
            "description": format!("Read local media {target}"),
            "inputSchema": {"type": "object", "properties": properties, "required": required, "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false},
        })).collect::<Vec<_>>()}),
        ),
        "tools/call" => {
            let params = &message["params"];
            let Some((_, target, properties, required)) = tools()
                .into_iter()
                .find(|(name, ..)| params["name"].as_str() == Some(name))
            else {
                return Some(error(&id, -32602, "Unknown tool"));
            };
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let valid = args.as_object().is_some_and(|args| {
                args.iter()
                    .all(|(key, value)| properties.get(key).is_some() && value.is_string())
                    && required.iter().all(|key| args.contains_key(*key))
            });
            if !valid {
                return Some(error(&id, -32602, "Invalid tool arguments"));
            }
            match engine.dispatch(target, &args, Mode::Once) {
                Ok(data) => {
                    // A structured object, even when the domain answer is an array.
                    let structured = json!({"data": data});
                    result(json!({
                        "content": [{"type": "text", "text": structured.to_string()}],
                        "structuredContent": structured,
                        "isError": false,
                    }))
                }
                Err(failure) => result(json!({
                    "content": [{"type": "text", "text": failure.error.message}],
                    "isError": true,
                })),
            }
        }
        _ => Some(error(&id, -32601, "Method not found")),
    }
}

/// Serves until stdin ends (0) or a line is too long (1).
pub(crate) fn serve(engine: &Engine, mut reader: impl BufRead, mut writer: impl Write) -> i32 {
    let mut initialized = false;
    let mut line = Vec::new();
    loop {
        line.clear();
        match std::io::Read::take(&mut reader, (MAX_LINE + 1) as u64).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return 0,
            Ok(_) if line.len() > MAX_LINE => return 1,
            Ok(_) => {}
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(message) => {
                let response = handle(engine, &message, initialized);
                if message["method"] == "initialize" {
                    initialized = true;
                }
                response
            }
            Err(_) => Some(error(&Value::Null, -32700, "Parse error")),
        };
        if let Some(response) = response
            && writeln!(writer, "{response}")
                .and_then(|()| writer.flush())
                .is_err()
        {
            return 0;
        }
    }
}
