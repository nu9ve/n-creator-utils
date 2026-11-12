//! Python `captures.capture_libraries` / `captures.captures`: finished and pending Stalker sessions of every known
//! library, read from each bundle's `manifest.json`.
//!
//! Python keeps an index per library in the catalog (`capture-index:<root>`) and rewrites it when a manifest
//! changed. This engine opens the catalog read only: it uses that index the same way (an entry whose manifest
//! stamp still matches is not read again) and computes changed entries in memory without saving them, so the
//! answer is the same and the next Python read refreshes the index as before.

use crate::catalog::Catalog;
use crate::rules::relative;
use crate::tree;
use nu9ve_json::{Error, Result, digest, require};
use nu9ve_volumes::{EntryKind, stamp};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const VIDEO: [&str; 8] = [
    ".mov", ".mp4", ".m4v", ".mts", ".m2ts", ".avi", ".mkv", ".webm",
];
const INDEX_KEYS: [&str; 4] = [
    "project_hint",
    "preview_path",
    "duration_seconds",
    "created_at_local",
];

/// Where Stalker libraries are looked for (Python's defaults; tests pass their own).
pub struct Places {
    pub registry: PathBuf,
    pub common: Vec<PathBuf>,
    pub fallback: PathBuf,
}

impl Places {
    pub fn user() -> Self {
        let home = home();
        Self {
            registry: home.join("Library/Application Support/Stalker/library.json"),
            common: vec![home.join("Movies/Stalker"), home.join("Downloads")],
            fallback: home.join("Movies/Stalker"),
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// `Path(value).expanduser()`.
fn expanduser(value: &str) -> PathBuf {
    if value == "~" {
        home()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(value)
    }
}

/// `str(Path(value).expanduser().absolute())`: joined to the working folder, `.` parts and trailing `/` dropped.
fn absolute(value: &str) -> String {
    let path = expanduser(value);
    let path = std::path::absolute(&path).unwrap_or(path);
    let mut text = path.display().to_string();
    while text.len() > 1 && text.ends_with('/') {
        text.pop();
    }
    text
}

/// Python `PurePath.suffix` of the last component.
fn suffix(name: &str) -> String {
    let last = name.rsplit('/').next().unwrap_or(name);
    match last.rfind('.') {
        Some(i) if i > 0 && i < last.len() - 1 => last[i..].to_owned(),
        _ => String::new(),
    }
}

/// Python `str(uuid.UUID(value))`: hex with optional `urn:`, `uuid:`, braces and dashes; lowercase 8-4-4-4-12.
fn python_uuid(value: &str) -> Option<String> {
    let hex: String = value
        .replace("urn:", "")
        .replace("uuid:", "")
        .trim_matches(|c| c == '{' || c == '}')
        .replace('-', "");
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let h = hex.to_ascii_lowercase();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    ))
}

fn python_str(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `value == n` with Python's numeric equality (`1 == 1.0 == True`).
fn equals_int(value: &Value, n: i64) -> bool {
    match value {
        Value::Bool(b) => i64::from(*b) == n,
        Value::Number(x) => x.as_f64() == Some(n as f64),
        _ => false,
    }
}

/// Python `project_hint(manifest)`.
fn project_hint(manifest: &Value) -> Value {
    let project = &manifest["project"];
    if !project.is_object() || !equals_int(&project["version"], 1) {
        return json!({});
    }
    let mut hint = Map::new();
    for key in ["name", "country"] {
        if let Some(text) = project[key].as_str() {
            hint.insert(key.into(), json!(text));
        }
    }
    Value::Object(hint)
}

/// Python `preview_track_path(manifest)`: a safe, bundle-relative video track, screen recordings first.
fn preview_track_path(manifest: &Value) -> String {
    let Some(tracks) = manifest["tracks"].as_array() else {
        return String::new();
    };
    let mut candidates: Vec<(u8, String)> = Vec::new();
    for track in tracks.iter().filter(|t| t.is_object()) {
        let Some(raw) = track["relativePath"].as_str() else {
            continue;
        };
        let Ok(safe) = relative(raw) else { continue };
        if !VIDEO.contains(&suffix(safe).to_lowercase().as_str()) {
            continue;
        }
        let mut label = vec![raw.to_owned()];
        for key in ["name", "title", "role", "kind", "type", "trackId", "id"] {
            label.push(track.get(key).map(python_str).unwrap_or_default());
        }
        let label = label.join(" ").to_lowercase();
        let priority = if label.contains("screen") || label.contains("display") {
            0
        } else {
            1
        };
        candidates.push((priority, safe.to_owned()));
    }
    candidates
        .into_iter()
        .min()
        .map(|c| c.1)
        .unwrap_or_default()
}

/// `len(manifest.get('tracks') or [])`; `None` where Python raises `TypeError`.
fn track_count(tracks: &Value) -> Option<usize> {
    match tracks {
        Value::Null | Value::Bool(false) => Some(0),
        Value::Array(a) => Some(a.len()),
        Value::Object(o) => Some(o.len()),
        Value::String(s) => Some(s.chars().count()),
        Value::Number(n) if n.as_f64() == Some(0.0) => Some(0),
        _ => None,
    }
}

/// `layout.stalker_facts`: the session start in the host zone, as `isoformat(timespec='minutes')`, and seconds.
fn stalker_facts(manifest: &Value, fallback_ns: i64) -> (String, Value) {
    let created = manifest["createdAt"]
        .as_str()
        .and_then(|text| nu9ve_time::DateTime::fromisoformat(&text.replace('Z', "+00:00")))
        .map(|dt| dt.astimezone())
        .unwrap_or_else(|| {
            nu9ve_time::DateTime::fromtimestamp(fallback_ns as f64 / 1e9).astimezone()
        });
    let iso = created.isoformat();
    // `YYYY-MM-DDTHH:MM` then the offset, without seconds or microseconds.
    let offset = iso[19..]
        .find(['+', '-'])
        .map(|i| iso[19 + i..].to_owned())
        .unwrap_or_default();
    let minutes = format!("{}{offset}", &iso[..16]);
    let duration = &manifest["duration"];
    let seconds = match (duration["value"].as_f64(), duration["timescale"].as_f64()) {
        (Some(value), Some(scale)) if scale != 0.0 && !duration["value"].is_boolean() => {
            json!(value / scale)
        }
        _ => Value::Null,
    };
    (minutes, seconds)
}

/// One bundle read again: the index entry Python would save.
fn index_entry(raw: &str, folder: &Path, stamp_value: &Value, mtime_ns: i64) -> Option<Value> {
    let bytes = tree::read_bytes(folder, "manifest.json", 1024 * 1024).ok()??;
    let manifest: Value = serde_json::from_slice(&bytes).ok()?;
    let version = &manifest["schemaVersion"];
    if manifest["producer"] != "stalker" || ![1, 2, 3].iter().any(|n| equals_int(version, *n)) {
        return None;
    }
    let session = python_uuid(manifest["sessionID"].as_str()?)?;
    let (created_local, seconds) = stalker_facts(&manifest, mtime_ns);
    let count = track_count(manifest.get("tracks").unwrap_or(&Value::Null))?;
    let mut preview = preview_track_path(&manifest);
    if !preview.is_empty() {
        let mut candidate = folder.to_path_buf();
        for component in preview.split('/') {
            candidate.push(component);
            if stamp(&candidate).is_ok_and(|s| s.kind == EntryKind::Link) {
                preview.clear();
                break;
            }
        }
        if !preview.is_empty() && !candidate.is_file() {
            preview.clear();
        }
    }
    Some(json!({
        "path": raw, "session_id": session, "status": manifest.get("status").cloned().unwrap_or(Value::Null),
        "manifest_digest": digest(&manifest), "stamp": stamp_value, "project_hint": project_hint(&manifest),
        "created_at": manifest.get("createdAt").cloned().unwrap_or(Value::Null),
        "created_at_local": created_local, "track_count": count, "duration_seconds": seconds,
        "preview_path": preview,
    }))
}

/// Python `captures(catalog, root)` without `changed_paths`: every session of one library.
pub fn captures(catalog: &Catalog, root: &str) -> Result<Vec<Value>> {
    let root = absolute(root);
    let root_path = Path::new(&root);
    let is_link = stamp(root_path).is_ok_and(|s| s.kind == EntryKind::Link);
    require(
        root_path.is_dir() && !is_link,
        "capture_root_unavailable",
        "Capture library is unavailable",
    )?;
    let mut candidates = BTreeSet::new();
    let entries =
        std::fs::read_dir(root_path).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".stalker") && entry.file_type().is_ok_and(|t| t.is_dir())
        {
            candidates.insert(format!("{root}/{name}"));
        }
    }
    if suffix(&root).to_lowercase() == ".stalker" {
        candidates.insert(root.clone());
    }
    let saved = catalog
        .get(&format!("capture-index:{root}"))?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let mut index: BTreeMap<String, Value> = BTreeMap::new();
    for raw in &candidates {
        let folder = Path::new(raw);
        let link = stamp(folder).is_ok_and(|s| s.kind == EntryKind::Link);
        if !folder.is_dir() || link {
            continue;
        }
        let review = json!({"path": raw, "status": "needs_review"});
        let Ok(current) = stamp(&folder.join("manifest.json")) else {
            index.insert(raw.clone(), review);
            continue;
        };
        let stamp_value = tree::stamp_json(&current);
        if let Some(previous) = saved.get(raw)
            && previous["stamp"] == stamp_value
            && INDEX_KEYS.iter().all(|k| previous.get(*k).is_some())
        {
            index.insert(raw.clone(), previous.clone());
            continue;
        }
        let entry = index_entry(raw, folder, &stamp_value, current.mtime_ns).unwrap_or(review);
        index.insert(raw.clone(), entry);
    }
    // Plans keep the manifest's spelling of sessionID (Stalker writes uppercase); compare as UUIDs.
    let mut archived = BTreeSet::new();
    if let Some(db) = catalog.db() {
        let mut statement = db
            .prepare("SELECT session_id,manifest_digest FROM sessions")
            .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
        for row in rows.flatten() {
            archived.insert((row.0.to_lowercase(), row.1));
        }
    }
    let mut sessions = Vec::new();
    for mut value in index.into_values() {
        let key = (
            value["session_id"].as_str().map(str::to_owned),
            value["manifest_digest"].as_str().map(str::to_owned),
        );
        let is_archived =
            matches!(&key, (Some(s), Some(d)) if archived.contains(&(s.clone(), d.clone())));
        let session = value["session_id"].as_str().unwrap_or_default().to_owned();
        let tag = catalog
            .get(&format!("capture-tag:{session}"))?
            .unwrap_or_else(|| json!({}));
        if let Some(object) = value.as_object_mut() {
            object.insert("archived".into(), json!(is_archived));
            object.insert("tag".into(), tag);
        }
        sessions.push(value);
    }
    Ok(sessions)
}

/// Python `has_sessions(root)`: a usual folder counts only with a `.stalker` folder directly inside.
fn has_sessions(root: &Path) -> bool {
    std::fs::read_dir(root).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(".stalker")
                && e.file_type().is_ok_and(|t| t.is_dir())
        })
    })
}

/// Python `capture_libraries(catalog, override)` with the user's usual places.
pub fn capture_libraries(catalog: &Catalog, params: &Value) -> Result<Value> {
    let chosen = match params.get("override") {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        _ => catalog
            .get("capture-override")?
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
    };
    libraries(catalog, &chosen, &Places::user())
}

pub fn libraries(catalog: &Catalog, chosen: &str, places: &Places) -> Result<Value> {
    let mut roots: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    if !chosen.is_empty() {
        roots.push(absolute(chosen));
    }
    if places.registry.exists() {
        let parent = places.registry.parent().unwrap_or(Path::new("/"));
        let name = places
            .registry
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let read = tree::read_bytes(parent, &name, 1024 * 1024).and_then(|bytes| {
            let bytes = bytes.ok_or_else(|| Error::new("operation_failed", "No such file"))?;
            serde_json::from_slice::<Value>(&bytes)
                .map_err(|_| Error::new("invalid_json", "Invalid JSON document"))
        });
        match read {
            Ok(data) if data["schema"] == "stalker-library/1" => {
                let previous = data["previous"].as_array().cloned().unwrap_or_default();
                for value in std::iter::once(&data["path"]).chain(previous.iter()) {
                    let path = if value.is_object() {
                        &value["path"]
                    } else {
                        value
                    };
                    if let Some(text) = path.as_str()
                        && expanduser(text).is_absolute()
                    {
                        roots.push(absolute(text));
                    }
                }
            }
            Ok(_) => errors.push("Unsupported Stalker library registry".into()),
            Err(error) => errors.push(error.message),
        }
    }
    // Stalker's save folder can change without telling anyone: its usual places count only with sessions inside.
    for path in &places.common {
        let text = path.display().to_string();
        if !roots.contains(&text) && has_sessions(path) {
            roots.push(text);
        }
    }
    if roots.is_empty() {
        roots.push(places.fallback.display().to_string());
    }
    let mut unique: Vec<String> = Vec::new();
    for root in roots {
        if !unique.contains(&root) {
            unique.push(root);
        }
    }
    let mut available = Vec::new();
    let mut found: BTreeMap<String, Value> = BTreeMap::new();
    for root in unique {
        match captures(catalog, &root) {
            Ok(sessions) => {
                available.push(root.clone());
                for mut session in sessions {
                    let path = session["path"].as_str().unwrap_or_default().to_owned();
                    session["root"] = json!(root);
                    found.insert(path, session);
                }
            }
            Err(error) => errors.push(format!("{root}: {}", error.message)),
        }
    }
    let mut values: Vec<Value> = found.into_values().collect();
    let key = |s: &Value| {
        (
            s["created_at"].as_str().unwrap_or_default().to_owned(),
            s["path"].as_str().unwrap_or_default().to_owned(),
        )
    };
    values.sort_by_key(|s| std::cmp::Reverse(key(s)));
    let pending: Vec<Value> = values
        .iter()
        .filter(|s| s["status"] == "completed" && s["archived"] == false)
        .cloned()
        .collect();
    Ok(json!({"roots": available, "sessions": values, "pending": pending, "errors": errors}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn without_stamps(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("stamp");
                map.values_mut().for_each(without_stamps);
            }
            Value::Array(items) => items.iter_mut().for_each(without_stamps),
            _ => {}
        }
    }

    /// Phase L: the same library files, registry, archived session and tag as Python's, the same answer.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python, TZ set)"]
    fn libraries_match_python() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let zone = golden["plan_tz"].as_str().unwrap();
        assert_eq!(
            std::env::var("TZ").as_deref(),
            Ok(zone),
            "run with TZ={zone}"
        );
        let case = &golden["captures"];
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let text_root = root.display().to_string();
        let mtime = std::time::UNIX_EPOCH
            + std::time::Duration::from_nanos(case["mtime_ns"].as_u64().unwrap());
        for (rel, text) in case["files"].as_object().unwrap() {
            let file = root.join(rel);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, text.as_str().unwrap()).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(mtime)
                .unwrap();
        }
        let registry = case["registry"].to_string().replace("<ROOT>", &text_root);
        std::fs::write(root.join("registry.json"), registry).unwrap();
        let state = root.join("state");
        std::fs::create_dir_all(&state).unwrap();
        rusqlite::Connection::open(state.join("catalog.sqlite3"))
            .unwrap()
            .execute_batch(case["sql"].as_str().unwrap())
            .unwrap();
        let catalog = Catalog::open(&state).unwrap();
        let places = Places {
            registry: root.join("registry.json"),
            common: Vec::new(),
            fallback: root.join("fallback"),
        };
        let found = libraries(&catalog, "", &places).unwrap();
        let mut mine: Value =
            serde_json::from_str(&found.to_string().replace(&text_root, "<ROOT>")).unwrap();
        let mut theirs = case["result"].clone();
        without_stamps(&mut mine);
        without_stamps(&mut theirs);
        assert_eq!(mine["roots"], theirs["roots"]);
        assert_eq!(mine["errors"], theirs["errors"]);
        let sessions = theirs["sessions"].as_array().unwrap();
        assert_eq!(mine["sessions"].as_array().unwrap().len(), sessions.len());
        for (a, b) in mine["sessions"].as_array().unwrap().iter().zip(sessions) {
            assert_eq!(a, b, "{}", b["path"]);
        }
        assert_eq!(mine["pending"], theirs["pending"]);
    }

    #[test]
    fn uuids_previews_and_hints_follow_python() {
        assert_eq!(
            python_uuid("123E4567E89B12D3A456426614174000").as_deref(),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
        assert_eq!(
            python_uuid("{urn:uuid:123e4567-e89b-12d3-a456-426614174000}").as_deref(),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
        assert_eq!(python_uuid("nope"), None);
        let manifest = json!({"tracks": [
            {"relativePath": "cam.mov", "kind": "camera"},
            {"relativePath": "../x.mov"},
            {"relativePath": "Screen/display.mp4"},
            {"relativePath": "audio.wav"}]});
        assert_eq!(preview_track_path(&manifest), "Screen/display.mp4");
        assert_eq!(suffix("a/.mov"), "");
        assert_eq!(
            project_hint(&json!({"project": {"version": 1.0, "name": "terra", "country": 5}})),
            json!({"name": "terra"})
        );
        assert_eq!(project_hint(&json!({"project": {"version": 2}})), json!({}));
    }
}
