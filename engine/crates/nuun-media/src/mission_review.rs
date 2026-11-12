//! Python `missions.review` (with `validate`, `summary`, `empty_state`, `load_state`, `merge`, `coverage`,
//! `_candidates`, `_read_export` and `_archive_state`): what each flight-app export would bring to the archive's
//! mission registry, and the digest that authorizes exactly that incorporation. Read only: the merge runs on a
//! copy in memory. `incorporate` (the write) stays in Python.
//!
//! One known difference: when an export cannot be read or is not valid JSON, `error` carries this engine's message
//! instead of Python's `OSError`/`json` text; the state (`invalid`) and the digest are the same.

use crate::catalog::Catalog;
use crate::missions::parse_date;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, Result, digest, require};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const HISTORY_SCHEMA: &str = "nu9ve-mission-history/1";
const MAX_EXPORT: u64 = 64 * 1024 * 1024;
const TOP_FIELDS: [&str; 5] = [
    "schemaVersion",
    "exportedAt",
    "exportedAtLocal",
    "timeZoneIdentifier",
    "cameraCapabilities",
];

type Instant = Option<String>;

/// A comparable instant of `parse_date` (UTC wall time, ISO text sorts as time); `None` is Python's `EPOCH`.
fn instant(value: &Value) -> Instant {
    parse_date(value).map(|d| {
        let mut utc = d.to_utc();
        utc.offset = None;
        utc.isoformat()
    })
}

fn truthy(value: &Value) -> bool {
    crate::volumes::truthy(value)
}

/// Python `str(value)` of a record id.
fn key_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        other => other.to_string(),
    }
}

/// `x in list` with Python's numeric equality.
fn contains(list: &Value, item: &Value) -> bool {
    list.as_array().is_some_and(|items| {
        items.iter().any(|v| {
            v == item || matches!((v.as_f64(), item.as_f64()), (Some(a), Some(b)) if a == b && !v.is_boolean() && !item.is_boolean())
        })
    })
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new("invalid_export", message)
}

/// missions.py `validate`.
fn validate(data: &Value, flow: &Value) -> Result<()> {
    require(data.is_object(), "invalid_export", "Not a JSON object")?;
    let version = data.get("schemaVersion").cloned().unwrap_or(Value::Null);
    if !contains(&flow["schema_versions"], &version) {
        let shown = match &version {
            Value::Null => "None".to_owned(),
            other => key_of(other),
        };
        return Err(invalid(format!("Unsupported schemaVersion: {shown}")));
    }
    require(
        parse_date(&data["exportedAt"]).is_some(),
        "invalid_export",
        "exportedAt missing or invalid",
    )?;
    require(
        data["plans"].is_array(),
        "invalid_export",
        "plans list missing",
    )?;
    for (key, field) in flow["merge_keys"].as_object().into_iter().flatten() {
        let Some(records) = data.get(key).filter(|r| !r.is_null()) else {
            continue;
        };
        let list = records
            .as_array()
            .filter(|l| l.iter().all(Value::is_object));
        let Some(list) = list else {
            return Err(invalid(format!("{key} must be a list of objects")));
        };
        let field = field.as_str().unwrap_or_default();
        if !list.iter().all(|r| {
            r[field].is_string() || r[field].is_i64() || r[field].is_u64() || r[field].is_boolean()
        }) {
            return Err(invalid(format!("{key} records need {field}")));
        }
    }
    for (_, field) in flow["tombstones"].as_object().into_iter().flatten() {
        let field = field.as_str().unwrap_or_default();
        require(
            data[field].is_null() || data[field].is_array(),
            "invalid_export",
            format!("{field} must be a list"),
        )?;
    }
    Ok(())
}

/// missions.py `window_kind`.
fn window_kind(export: &Value) -> Value {
    let kind = &export["exportWindow"]["kind"];
    if truthy(kind) {
        return kind.clone();
    }
    let plans = export["plans"].as_array().map(Vec::len).unwrap_or(0);
    json!(if plans == 1 { "plan" } else { "all" })
}

/// missions.py `summary`.
fn summary(data: &Value) -> Map<String, Value> {
    let log = [&data["captureLog"], &data["captures"]]
        .into_iter()
        .find(|v| truthy(v))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut dates: Vec<&str> = log
        .iter()
        .filter_map(|r| r["capturedAt"].as_str())
        .collect();
    dates.sort();
    let mut out = Map::new();
    out.insert("exportedAt".into(), data["exportedAt"].clone());
    out.insert("kind".into(), window_kind(data));
    out.insert("schemaVersion".into(), data["schemaVersion"].clone());
    out.insert(
        "plans".into(),
        json!(data["plans"].as_array().map(Vec::len).unwrap_or(0)),
    );
    out.insert("photos".into(), json!(log.len()));
    out.insert("first".into(), json!(dates.first()));
    out.insert("last".into(), json!(dates.last()));
    out
}

/// The registry as missions.py keeps it in memory: top fields, lists by id and the `nuun` block.
#[derive(Clone)]
struct State {
    top: Map<String, Value>,
    lists: Map<String, Value>,
    nuun: Value,
}

fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// missions.py `empty_state`.
fn empty_state(flow: &Value) -> State {
    let merge = keys(&flow["merge_keys"]);
    let newest = flow["schema_versions"]
        .as_array()
        .into_iter()
        .flatten()
        .max_by(|a, b| {
            a.as_f64()
                .partial_cmp(&b.as_f64())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
        .unwrap_or(Value::Null);
    let empty: Map<String, Value> = merge.iter().map(|k| (k.clone(), json!({}))).collect();
    let tombs: Map<String, Value> = keys(&flow["tombstones"])
        .into_iter()
        .map(|k| (k, json!({})))
        .collect();
    State {
        top: Map::from_iter([("schemaVersion".to_owned(), newest)]),
        lists: empty.clone(),
        nuun: json!({"schema": HISTORY_SCHEMA, "flow": flow["id"], "sources": {}, "records": empty,
                     "tombstones": tombs, "changes_count": 0}),
    }
}

/// missions.py `load_state`.
fn load_state(raw: Option<Vec<u8>>, flow: &Value) -> Result<State> {
    let Some(raw) = raw else {
        return Ok(empty_state(flow));
    };
    let history: Value =
        serde_json::from_slice(&raw).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    let nuun = if truthy(&history["nuun"]) {
        &history["nuun"]
    } else {
        &Value::Null
    };
    require(
        nuun["schema"] == HISTORY_SCHEMA && nuun["flow"] == flow["id"],
        "invalid_history",
        "The mission history on the archive is not a nüün registry for this flow",
    )?;
    let mut state = empty_state(flow);
    for field in TOP_FIELDS {
        if let Some(v) = history.get(field) {
            state.top.insert(field.into(), v.clone());
        }
    }
    state.top.retain(|k, _| history.get(k).is_some());
    for (key, field) in flow["merge_keys"].as_object().into_iter().flatten() {
        let field = field.as_str().unwrap_or_default();
        let mut list = Map::new();
        for record in history[key.as_str()].as_array().into_iter().flatten() {
            list.insert(key_of(&record[field]), record.clone());
        }
        state.lists.insert(key.clone(), Value::Object(list));
        let records = nuun["records"][key.as_str()].clone();
        state.nuun["records"][key.as_str()] = if truthy(&records) { records } else { json!({}) };
    }
    for key in keys(&state.nuun["tombstones"]) {
        let tomb = nuun["tombstones"][key.as_str()].clone();
        state.nuun["tombstones"][key.as_str()] = if truthy(&tomb) { tomb } else { json!({}) };
    }
    let sources = nuun["sources"].clone();
    state.nuun["sources"] = if truthy(&sources) { sources } else { json!({}) };
    state.nuun["changes_count"] = json!(nuun["changes_count"].as_i64().unwrap_or(0));
    Ok(state)
}

/// missions.py `merge` on the in-memory copy: the change counts of one export. Never lets an older export win.
fn merge(
    state: &mut State,
    data: &Value,
    sha: &str,
    name: &str,
    flow: &Value,
) -> Map<String, Value> {
    let incoming = instant(&data["exportedAt"]);
    let mut counts: [usize; 3] = [0, 0, 0];
    let stored_at = |nuun: &Value, key: &str, rid: &str| -> Instant {
        let source = nuun["records"][key][rid]
            .as_str()
            .map(|s| &nuun["sources"][s]);
        source
            .filter(|s| !s.is_null())
            .and_then(|s| instant(&s["exportedAt"]))
    };
    for (key, field) in flow["merge_keys"].as_object().into_iter().flatten() {
        let field = field.as_str().unwrap_or_default();
        for record in data[key.as_str()].as_array().into_iter().flatten() {
            let rid = key_of(&record[field]);
            let tomb = &state.nuun["tombstones"][key.as_str()][rid.as_str()];
            if truthy(tomb) && instant(&tomb["exportedAt"]) >= incoming {
                continue; // deleted by a newer export: an older one never brings it back
            }
            let stored = state.lists[key.as_str()].get(rid.as_str()).cloned();
            match stored {
                None => {
                    state.lists[key.as_str()][rid.as_str()] = record.clone();
                    state.nuun["records"][key.as_str()][rid.as_str()] = json!(sha);
                    counts[0] += 1;
                }
                Some(stored) => {
                    if incoming >= stored_at(&state.nuun, key, &rid) && stored != *record {
                        state.lists[key.as_str()][rid.as_str()] = record.clone();
                        state.nuun["records"][key.as_str()][rid.as_str()] = json!(sha);
                        counts[1] += 1;
                    }
                }
            }
        }
    }
    for (key, field) in flow["tombstones"].as_object().into_iter().flatten() {
        let field = field.as_str().unwrap_or_default();
        for rid in data[field].as_array().into_iter().flatten() {
            let rid = key_of(rid);
            if state.lists[key.as_str()].get(rid.as_str()).is_some()
                && incoming >= stored_at(&state.nuun, key, &rid)
            {
                if let Some(list) = state.lists[key.as_str()].as_object_mut() {
                    list.remove(&rid);
                }
                if let Some(records) = state.nuun["records"][key.as_str()].as_object_mut() {
                    records.remove(&rid);
                }
                counts[2] += 1;
            }
            let previous = &state.nuun["tombstones"][key.as_str()][rid.as_str()];
            if !truthy(previous) || instant(&previous["exportedAt"]) < incoming {
                state.nuun["tombstones"][key.as_str()][rid.as_str()] =
                    json!({"export_sha256": sha, "exportedAt": data["exportedAt"]});
            }
        }
    }
    let newest = state.nuun["sources"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, s)| instant(&s["exportedAt"]))
        .max()
        .unwrap_or(None);
    if incoming >= newest {
        state.top = TOP_FIELDS
            .iter()
            .filter_map(|k| data.get(*k).map(|v| ((*k).to_owned(), v.clone())))
            .collect();
    }
    let changes = json!({"new": counts[0], "updated": counts[1], "deleted": counts[2]});
    let mut source = summary(data);
    source.insert("file".into(), json!(name));
    source.insert("sha256".into(), json!(sha));
    source.insert("raw".into(), Value::Null);
    source.insert("changes".into(), changes.clone());
    state.nuun["sources"][sha] = Value::Object(source);
    changes.as_object().cloned().unwrap_or_default()
}

/// `max(values, key=parse_date or EPOCH)`: the first of the latest, as Python's `max`.
fn latest(values: impl Iterator<Item = Value>) -> Value {
    let mut best: Option<(Instant, Value)> = None;
    for value in values {
        let at = instant(&value);
        if best.as_ref().is_none_or(|(b, _)| at > *b) {
            best = Some((at, value));
        }
    }
    best.map(|b| b.1).unwrap_or(Value::Null)
}

/// missions.py `coverage`.
fn coverage(state: &State) -> Value {
    let records: Vec<&Value> = ["captureLog", "captures"]
        .iter()
        .flat_map(|k| {
            state
                .lists
                .get(*k)
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
        })
        .map(|(_, r)| r)
        .collect();
    let captured = records
        .iter()
        .filter(|r| r["capturedAt"].is_string())
        .map(|r| r["capturedAt"].clone());
    let media = records
        .iter()
        .filter_map(|r| r["mediaCreatedAt"].as_str())
        .max()
        .map(|m| json!(m))
        .unwrap_or(Value::Null);
    let sources = state.nuun["sources"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let counts: Map<String, Value> = state
        .lists
        .iter()
        .map(|(k, v)| (k.clone(), json!(v.as_object().map(Map::len).unwrap_or(0))))
        .collect();
    json!({
        "exported_until": latest(sources.values().map(|s| s["exportedAt"].clone())),
        "last_capture_at": latest(captured),
        "last_media_created_at": media,
        "records": counts,
        "sources": sources.len(),
    })
}

/// `fnmatch.fnmatch` on POSIX (case-sensitive): `*`, `?` and `[…]`/`[!…]`.
fn fnmatch(name: &str, pattern: &str) -> bool {
    fn go(n: &[char], p: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|i| go(&n[i..], &p[1..])),
            Some('?') => !n.is_empty() && go(&n[1..], &p[1..]),
            Some('[') => {
                let Some(end) = p.iter().skip(1).position(|c| *c == ']').map(|i| i + 1) else {
                    return n.first() == Some(&'[') && go(&n[1..], &p[1..]);
                };
                let Some(c) = n.first() else { return false };
                let (negate, set) = match p[1] {
                    '!' => (true, &p[2..end]),
                    _ => (false, &p[1..end]),
                };
                let mut hit = false;
                let mut i = 0;
                while i < set.len() {
                    if i + 2 < set.len() && set[i + 1] == '-' {
                        hit |= set[i] <= *c && *c <= set[i + 2];
                        i += 3;
                    } else {
                        hit |= set[i] == *c;
                        i += 1;
                    }
                }
                hit != negate && go(&n[1..], &p[end + 1..])
            }
            Some(literal) => n.first() == Some(literal) && go(&n[1..], &p[1..]),
        }
    }
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    go(&n, &p)
}

fn expand(value: &str) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let path = if value == "~" {
        home
    } else if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(value)
    };
    std::path::absolute(&path).unwrap_or(path)
}

/// missions.py `_candidates`: the given paths, or the inbox's files matching the flow's patterns, sorted.
fn candidates(flow: &Value, paths: &Value) -> Result<(PathBuf, Vec<PathBuf>)> {
    let inbox = expand(flow["inbox"].as_str().unwrap_or_default());
    if !paths.is_null() {
        let list = paths
            .as_array()
            .filter(|l| l.iter().all(Value::is_string))
            .ok_or_else(|| Error::new("invalid_request", "paths must be a list"))?;
        return Ok((
            inbox,
            list.iter()
                .map(|p| expand(p.as_str().unwrap_or_default()))
                .collect(),
        ));
    }
    let mut found: Vec<PathBuf> = match std::fs::read_dir(&inbox) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                flow["patterns"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|p| fnmatch(&name, p))
            })
            .map(|e| e.path())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(Error::new("operation_failed", e.to_string())),
    };
    found.sort();
    Ok((inbox, found))
}

/// missions.py `_read_export`: the export's bytes (at most 64 MiB) and its stamp, without following links.
fn read_export(path: &Path) -> Result<(Vec<u8>, Value)> {
    let parent = path.parent().unwrap_or(Path::new("/"));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stamp =
        nu9ve_volumes::stamp(path).map_err(|e| Error::new("operation_failed", e.message))?;
    require(
        stamp.size <= MAX_EXPORT,
        "file_too_large",
        format!("Export larger than 64 MiB: {name}"),
    )?;
    let raw = crate::tree::read_bytes(parent, &name, MAX_EXPORT as usize)?
        .ok_or_else(|| Error::new("operation_failed", format!("No such file: {name}")))?;
    Ok((raw, crate::tree::stamp_json(&stamp)))
}

/// The `missions_review` method.
pub fn review(catalog: &Catalog, backend: Option<&dyn Backend>, params: &Value) -> Result<Value> {
    if let Some(unexpected) = params.as_object().and_then(|p| {
        p.keys()
            .find(|k| !["archive", "paths", "flow_id"].contains(&k.as_str()))
    }) {
        return Err(Error::new(
            "operation_failed",
            format!("review() got an unexpected keyword argument '{unexpected}'"),
        ));
    }
    let recipe = nuun_recipe::effective(catalog.get("recipe")?.as_ref());
    let flows = recipe["missions"].as_array().cloned().unwrap_or_default();
    require(
        !flows.is_empty(),
        "no_mission_flow",
        "This recipe has no mission registry",
    )?;
    let flow = match &params["flow_id"] {
        Value::Null => flows[0].clone(),
        wanted => flows
            .iter()
            .find(|f| &f["id"] == wanted)
            .cloned()
            .ok_or_else(|| {
                Error::new(
                    "no_mission_flow",
                    format!("Unknown mission registry: {}", key_of(wanted)),
                )
            })?,
    };
    let (inbox, paths) = candidates(&flow, &params["paths"])?;
    let archive = params.get("archive").cloned().unwrap_or(Value::Null);
    let mut state = None;
    if !archive.is_null() {
        let record = catalog.volume(archive.as_str().unwrap_or_default())?;
        require(
            record["role"] == "archive",
            "invalid_role",
            "The mission registry lives on the archive",
        )?;
        let backend = backend
            .ok_or_else(|| Error::new("not_ported", "No volume backend on this platform"))?;
        volumes::check(backend, &record)?;
        let area = flow["archive_area"]
            .as_str()
            .unwrap_or_default()
            .trim_matches('/');
        let root = Path::new(record["root"].as_str().unwrap_or_default());
        state = Some(load_state(
            crate::tree::read_bytes(root, &format!("{area}/history.json"), 1 << 40)?,
            &flow,
        )?);
    }
    let mut files: Vec<Value> = Vec::new();
    let mut loaded: Vec<(usize, Value)> = Vec::new();
    for path in paths {
        let mut entry = json!({"path": path.display().to_string(),
            "name": path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            "in_inbox": path.parent() == Some(inbox.as_path())});
        let read = read_export(&path).and_then(|(raw, stamp)| {
            let data: Value = serde_json::from_slice(&raw)
                .map_err(|e| Error::new("invalid_json", e.to_string()))?;
            validate(&data, &flow)?;
            Ok((raw, stamp, data))
        });
        match read {
            Err(error) => {
                entry["state"] = json!("invalid");
                entry["error"] = json!(error.message);
                files.push(entry);
            }
            Ok((raw, stamp, data)) => {
                entry["sha256"] = json!(hex(&Sha256::digest(&raw)));
                entry["bytes"] = json!(raw.len());
                entry["stamp"] = stamp;
                for (k, v) in summary(&data) {
                    entry[k.as_str()] = v;
                }
                loaded.push((files.len(), data));
                files.push(entry);
            }
        }
    }
    let registry = match state {
        Some(state) => {
            let mut trial = state.clone();
            loaded.sort_by(|a, b| instant(&a.1["exportedAt"]).cmp(&instant(&b.1["exportedAt"])));
            for (i, data) in &loaded {
                let sha = files[*i]["sha256"].as_str().unwrap_or_default().to_owned();
                let name = files[*i]["name"].as_str().unwrap_or_default().to_owned();
                if trial.nuun["sources"].get(&sha).is_some() {
                    files[*i]["state"] = json!("incorporated");
                    files[*i]["changes"] = json!({"new": 0, "updated": 0, "deleted": 0});
                    continue;
                }
                let changes = merge(&mut trial, data, &sha, &name, &flow);
                let any = changes.values().any(|v| v.as_u64().unwrap_or(0) > 0);
                files[*i]["changes"] = Value::Object(changes);
                files[*i]["state"] = json!(if any { "new" } else { "nothing_new" });
            }
            coverage(&trial)
        }
        None => {
            for (i, _) in &loaded {
                files[*i]["state"] = json!("unknown_registry");
            }
            Value::Null
        }
    };
    let digest_files: Vec<Value> = files
        .iter()
        .map(|f| {
            json!([
                f["path"],
                f.get("sha256").cloned().unwrap_or(Value::Null),
                f["state"]
            ])
        })
        .collect();
    let value_digest =
        digest(&json!({"flow": flow["id"], "archive": archive, "files": digest_files}));
    Ok(
        json!({"schema": "nu9ve-missions-review/1", "flow": flow["id"], "label": flow["label"],
              "inbox": inbox.display().to_string(), "archive": archive, "files": files,
              "coverage_after": registry, "digest": value_digest}),
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Online when the registered root exists: answers the identity the catalog saved for it.
    struct Registered(std::collections::HashMap<String, Value>);

    impl crate::volumes::Backend for Registered {
        fn mounted_roots(&self) -> Vec<std::path::PathBuf> {
            Vec::new()
        }
        fn describe(&self, root: &std::path::Path, _: bool) -> nu9ve_json::Result<Value> {
            nu9ve_json::require(root.is_dir(), "volume_offline", "Fixture disk disconnected")?;
            self.0
                .get(&root.display().to_string())
                .cloned()
                .ok_or_else(|| nu9ve_json::Error::new("volume_offline", "Unknown fixture disk"))
        }
        fn usage(&self, _: &std::path::Path) -> Option<(u64, u64)> {
            None
        }
    }

    /// The golden case `name`, its files under a fresh root and its catalog (and, for storage, its index).
    fn golden(name: &str) -> (tempfile::TempDir, std::path::PathBuf, String, Value) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let case = golden[name].clone();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let text_root = root.display().to_string();
        for (rel, text) in case["files"].as_object().unwrap() {
            let file = root.join(rel);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, text.as_str().unwrap()).unwrap();
        }
        let state = root.join("state");
        std::fs::create_dir_all(&state).unwrap();
        rusqlite::Connection::open(state.join("catalog.sqlite3"))
            .unwrap()
            .execute_batch(&case["sql"].as_str().unwrap().replace("<ROOT>", &text_root))
            .unwrap();
        if let Some(index) = case["index_sql"].as_str() {
            let db = rusqlite::Connection::open(state.join("storage-index.sqlite3")).unwrap();
            db.execute_batch(&index.replace("<ROOT>", &text_root))
                .unwrap();
            // `iterdump` leaves out the schema version the index is opened by.
            db.execute_batch("PRAGMA user_version=1").unwrap();
        }
        (temp, state, text_root, case)
    }

    fn backend_for(catalog: &Catalog) -> Registered {
        Registered(
            catalog
                .volumes()
                .unwrap()
                .into_iter()
                .map(|v| {
                    (
                        v["root"].as_str().unwrap().to_owned(),
                        v["identity"].clone(),
                    )
                })
                .collect(),
        )
    }

    fn answer(result: nu9ve_json::Result<Value>, text_root: &str) -> Value {
        match result {
            Ok(data) => {
                let data: Value =
                    serde_json::from_str(&data.to_string().replace(text_root, "<ROOT>")).unwrap();
                json!({"ok": true, "data": data})
            }
            Err(e) => json!({"ok": false, "code": e.code}),
        }
    }

    fn reviewed_digest(data: &Value) -> Value {
        let files: Vec<Value> = data["files"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                json!([
                    f["path"],
                    f.get("sha256").cloned().unwrap_or(Value::Null),
                    f["state"]
                ])
            })
            .collect();
        json!(digest(
            &json!({"flow": data["flow"], "archive": data["archive"], "files": files})
        ))
    }

    /// Phase L: the same inbox, exports and registry on the archive as Python's, the same reviews.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn reviews_match_python() {
        let (_temp, state, root, case) = golden("missions");
        let catalog = Catalog::open(&state).unwrap();
        let backend = backend_for(&catalog);
        let elsewhere = format!("{root}/elsewhere/coverage-x.json");
        let inbox_two = format!("{root}/inbox/coverage-2.json");
        for (name, params) in [
            ("inbox", json!({"archive": "MEDIA"})),
            ("no-archive", json!({})),
            (
                "paths",
                json!({"archive": "MEDIA", "paths": [elsewhere, inbox_two]}),
            ),
            ("unknown-flow", json!({"flow_id": "nope"})),
            ("bad-paths", json!({"paths": "x"})),
        ] {
            let found = review(&catalog, Some(&backend), &params);
            if let Ok(data) = &found {
                assert_eq!(
                    data["digest"],
                    reviewed_digest(data),
                    "{name}: digest of what it shows"
                );
            }
            let mut mine = answer(found, &root);
            let mut theirs = case["results"][name].clone();
            // The digest signs the paths, which hold each run's folder: compare it over `<ROOT>` paths.
            for value in [&mut mine, &mut theirs] {
                if let Some(data) = value.get_mut("data") {
                    data["digest"] = reviewed_digest(data);
                    for file in data["files"].as_array_mut().into_iter().flatten() {
                        file.as_object_mut().unwrap().remove("stamp");
                    }
                }
            }
            assert_eq!(mine, theirs, "{name}");
        }
    }

    #[test]
    fn fnmatch_follows_python() {
        assert!(fnmatch("coverage-2026.json", "coverage-*.json"));
        assert!(!fnmatch("Coverage-2026.json", "coverage-*.json"));
        assert!(fnmatch("JSON-a.json", "JSON-?.json"));
        assert!(fnmatch("a1", "a[0-9]"));
        assert!(!fnmatch("ab", "a[!b]"));
    }
}
