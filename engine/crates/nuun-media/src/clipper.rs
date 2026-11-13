//! Python `clipper.inspect` (with `queue_ids`, `unreported_references` and `overlaps`): whether Clipper uses a path
//! that nüün is about to free, from Clipper's own report and, for projects that report does not cover, from their
//! files. Read only: it never edits a producer's JSON. Release, closeout and discard all ask it before acting.

use crate::catalog::Catalog;
use crate::integrations::Integrations;
use nu9ve_json::{Result, require};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// clipper.py `OFFLINE_PROJECTS`.
pub const OFFLINE_PROJECTS: &str = "Hay proyectos Clipper desconectados; dependencias desconocidas";

/// `os.path.realpath`: links resolved as far as the path exists, the rest joined as written.
pub(crate) fn realpath(value: &str) -> PathBuf {
    let path = std::path::absolute(value).unwrap_or_else(|_| PathBuf::from(value));
    let mut missing = Vec::new();
    let mut current = path.as_path();
    loop {
        if let Ok(found) = std::fs::canonicalize(current) {
            let mut out = found;
            for part in missing.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_owned());
                current = parent;
            }
            _ => return path,
        }
    }
}

/// clipper.py `overlaps`: the same path, or one inside the other (ignoring case on macOS).
pub fn overlaps(a: &str, b: &str) -> bool {
    let (mut a, mut b) = (
        realpath(a).display().to_string(),
        realpath(b).display().to_string(),
    );
    if cfg!(target_os = "macos") {
        a = a.to_lowercase();
        b = b.to_lowercase();
    }
    let (a, b) = (Path::new(&a), Path::new(&b));
    a == b || b.starts_with(a) || a.starts_with(b)
}

fn read_json(path: &Path) -> std::result::Result<Value, String> {
    let parent = path.parent().unwrap_or(Path::new("/"));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = crate::tree::read_bytes(parent, &name, 16 * 1024 * 1024)
        .map_err(|e| e.message)?
        .ok_or_else(|| format!("[Errno 2] No such file or directory: '{}'", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid JSON document".to_owned())
}

/// clipper.py `queue_ids`: the export jobs still pending, or `None` when the queue is missing or unknown.
fn queue_ids(base: &Path) -> Option<Vec<String>> {
    let marker = base.join(".export-queue/queue.json");
    if !marker.exists() {
        return None;
    }
    let data = read_json(&marker).ok()?;
    if data["schema_version"] != 1 || !data["jobs"].is_array() {
        return None;
    }
    let mut ids = Vec::new();
    for job in data["jobs"].as_array().into_iter().flatten() {
        let status = job.get("status")?;
        if status != "completed" {
            ids.push(
                job.get("id")?
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| job["id"].to_string()),
            );
        }
    }
    Some(ids)
}

/// Every string of a legacy project's active sections, and object keys that are absolute paths.
fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|i| strings(i, out)),
        Value::Object(map) => {
            for (key, item) in map {
                if key.starts_with('/') {
                    out.push(key.clone());
                }
                strings(item, out);
            }
        }
        _ => {}
    }
}

/// clipper.py `unreported_references`: projects absent from Clipper's report, and old export snapshots.
fn unreported_references(
    paths: &[(String, Option<String>)],
    data: &Value,
    base: &Path,
    home: &Path,
) -> Vec<String> {
    let mut blockers = Vec::new();
    let mut roots: std::collections::BTreeSet<PathBuf> = Default::default();
    let reported: std::collections::BTreeSet<PathBuf> = data["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["clipper_root"].as_str())
        .map(realpath)
        .collect();
    for parent in [base.to_path_buf(), home.join("Downloads")] {
        if !parent.exists() {
            continue;
        }
        if std::fs::symlink_metadata(&parent).is_ok_and(|m| m.file_type().is_symlink()) {
            blockers.push("No se pudieron revisar proyectos antiguos de Clipper: Clipper projects folder is a symlink".into());
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&parent) {
            for entry in entries.flatten() {
                // `glob('*/…')` leaves hidden folders out.
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let candidate = entry.path();
                if candidate.join("clipper-project.json").exists() {
                    roots.insert(candidate);
                }
            }
        }
    }
    let registry = base.join(".linked-projects.json");
    if registry.exists() {
        match read_json(&registry) {
            Ok(linked)
                if linked["schema"] == "clipper-linked-projects/1"
                    && linked["roots"].is_array() =>
            {
                for root in linked["roots"].as_array().into_iter().flatten() {
                    match root.as_str().filter(|r| Path::new(r).is_absolute()) {
                        None => {
                            blockers.push(
                                "Registro Clipper desconocido: Invalid Clipper project root".into(),
                            );
                            break;
                        }
                        Some(r) if !Path::new(r).join("clipper-project.json").is_file() => {
                            blockers.push(format!("Proyecto Clipper no disponible: {r}"));
                        }
                        Some(r) => {
                            roots.insert(PathBuf::from(r));
                        }
                    }
                }
            }
            Ok(_) => blockers
                .push("Registro Clipper desconocido: Invalid Clipper project registry".into()),
            Err(error) => blockers.push(format!("Registro Clipper desconocido: {error}")),
        }
    }
    for root in &roots {
        if reported.contains(&realpath(&root.display().to_string())) {
            continue;
        }
        match read_json(&root.join("clipper-project.json")) {
            Ok(project) if project.is_object() => {
                let mut values = Vec::new();
                for key in [
                    "items",
                    "capture_sessions",
                    "sync_groups",
                    "assembly_timeline",
                ] {
                    strings(&project[key], &mut values);
                }
                if values
                    .iter()
                    .any(|v| v.starts_with('/') && paths.iter().any(|(p, _)| overlaps(v, p)))
                {
                    blockers.push(format!(
                        "Proyecto antiguo usa esta ruta; adopta/reenlaza la fuente desde Clipper antes de liberar: {}",
                        root.display()
                    ));
                }
            }
            Ok(_) => blockers.push(format!(
                "Proyecto Clipper no revisable: {}: Invalid legacy project",
                root.display()
            )),
            Err(error) => blockers.push(format!(
                "Proyecto Clipper no revisable: {}: {error}",
                root.display()
            )),
        }
    }
    let queue = base.join(".export-queue/queue.json");
    if queue.exists() {
        match read_json(&queue) {
            Ok(snapshot) if snapshot["schema_version"] == 1 && snapshot["jobs"].is_array() => {
                for job in snapshot["jobs"].as_array().into_iter().flatten() {
                    if !job["sources"].is_object() || !job["status"].is_string() {
                        blockers.push("Cola Clipper no revisable: Invalid queue job".into());
                        break;
                    }
                    if job["status"] == "completed" {
                        continue;
                    }
                    let used = job["sources"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .any(|(value, _)| paths.iter().any(|(p, _)| overlaps(value, p)));
                    if used {
                        let id = match &job["id"] {
                            Value::String(s) => s.clone(),
                            Value::Null => "None".into(),
                            other => other.to_string(),
                        };
                        blockers.push(format!(
                            "Snapshot de exportación conserva la ruta original: {id}"
                        ));
                    }
                }
            }
            Ok(_) => blockers.push("Cola Clipper no revisable: Unknown export queue".into()),
            Err(error) => blockers.push(format!("Cola Clipper no revisable: {error}")),
        }
    }
    blockers
}

/// clipper.py `inspect(catalog, paths)`: blockers, relinks Clipper could make, and Clipper's report.
/// `paths` maps a local asset or bundle root to its verified replacement, if any, in the caller's order.
pub fn inspect(catalog: &Catalog, paths: &[(String, Option<String>)]) -> Result<Value> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    inspect_with(&Integrations::read(catalog)?, paths, &home)
}

/// `inspect` with the consumer settings and the home folder given (tests use a fixture Clipper and home).
pub(crate) fn inspect_with(
    integrations: &Integrations,
    paths: &[(String, Option<String>)],
    home: &Path,
) -> Result<Value> {
    let (data, _) = integrations.dependencies_within(Duration::from_secs(120))?;
    inspect_dependencies(integrations, paths, home, data)
}

pub(crate) fn inspect_dependencies(
    integrations: &Integrations,
    paths: &[(String, Option<String>)],
    home: &Path,
    data: Value,
) -> Result<Value> {
    let base = integrations.clipper_base.clone();
    let mut blockers: Vec<String> = Vec::new();
    let mut relinks: Vec<Value> = Vec::new();
    if data["offline_projects"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
    {
        blockers.push(OFFLINE_PROJECTS.into());
    }
    let active_queue = queue_ids(&base);
    let target_of = |path: &str| {
        paths
            .iter()
            .find(|(p, _)| p == path)
            .and_then(|(_, t)| t.clone())
    };
    for project in data["projects"].as_array().into_iter().flatten() {
        for ack in project["relinks"].as_array().into_iter().flatten() {
            require(ack.is_object(), "clipper_invalid", "Invalid relink ACK")?;
            let pins = ack["still_pinned"].as_array().cloned().unwrap_or_default();
            let pinned = !pins.is_empty()
                && active_queue.as_ref().is_none_or(|queue| {
                    pins.iter()
                        .any(|pin| pin.as_str().is_some_and(|p| queue.iter().any(|q| q == p)))
                });
            let from = ack["from"]
                .as_str()
                .filter(|f| !f.is_empty())
                .unwrap_or("/");
            if pinned && paths.iter().any(|(p, _)| overlaps(from, p)) {
                let shown = match &ack["from"] {
                    Value::String(s) => s.clone(),
                    Value::Null => "None".into(),
                    other => other.to_string(),
                };
                blockers.push(format!(
                    "Una exportación aún fija la ruta anterior: {shown}"
                ));
            }
        }
        for asset in project["assets"].as_array().into_iter().flatten() {
            let asset_path = asset["path"].as_str().unwrap_or_default();
            let mut used = vec![asset_path.to_owned()];
            used.extend(
                asset["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned),
            );
            let hits: Vec<&String> = paths
                .iter()
                .map(|(p, _)| p)
                .filter(|p| used.iter().any(|u| overlaps(p, u)))
                .collect();
            if hits.is_empty() {
                continue;
            }
            let queued = asset["queued_exports"]
                .as_array()
                .is_some_and(|q| !q.is_empty());
            if queued {
                blockers.push(format!("Exportación en cola: {asset_path}"));
            }
            let sync = crate::volumes::truthy(&asset["sync_groups"]);
            if asset["in_use"] == true || sync {
                let target =
                    target_of(asset_path).or_else(|| hits.iter().find_map(|h| target_of(h)));
                match target {
                    Some(target) if !queued => relinks.push(json!({
                        "project": project["clipper_root"], "asset_id": asset["asset_id"],
                        "revision": asset.get("revision").cloned().unwrap_or(Value::Null),
                        "from": asset_path, "to": target, "kind": asset.get("kind").cloned().unwrap_or(Value::Null)})),
                    _ => blockers.push(format!("Fuente en uso sin reemplazo editable: {asset_path}")),
                }
            }
        }
    }
    // Historical rows can pin the same logical asset at another revision/path.
    let ids: Vec<Value> = relinks.iter().map(|r| r["asset_id"].clone()).collect();
    for project in data["projects"].as_array().into_iter().flatten() {
        if project["assets"].as_array().into_iter().flatten().any(|a| {
            ids.contains(&a["asset_id"])
                && a["queued_exports"]
                    .as_array()
                    .is_some_and(|q| !q.is_empty())
        }) {
            blockers.push("Una cola conserva miembros/revisiones anteriores del asset".into());
        }
    }
    // `{(project, asset_id): relink}`: the last wins, at the position of the first.
    let mut unique: Vec<Value> = Vec::new();
    for relink in relinks {
        match unique
            .iter_mut()
            .find(|r| r["project"] == relink["project"] && r["asset_id"] == relink["asset_id"])
        {
            Some(slot) => *slot = relink,
            None => unique.push(relink),
        }
    }
    blockers.extend(unreported_references(paths, &data, &base, home));
    let mut seen = std::collections::BTreeSet::new();
    blockers.retain(|b| seen.insert(b.clone()));
    Ok(json!({"blockers": blockers, "relinks": unique, "dependencies": data}))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::captures::{Places, discard_check_with};

    /// E0b: the same fixture Clipper, legacy project, registry, queue and sessions as Python's, the same answers.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn guards_and_discard_checks_match_python() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let case = &golden["clipper"];
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let text_root = root.display().to_string();
        for (rel, text) in case["files"].as_object().unwrap() {
            let file = root.join(rel);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, text.as_str().unwrap().replace("<ROOT>", &text_root)).unwrap();
        }
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join("bin/clipper"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let state = root.join("state");
        std::fs::create_dir_all(&state).unwrap();
        rusqlite::Connection::open(state.join("catalog.sqlite3"))
            .unwrap()
            .execute_batch(&case["sql"].as_str().unwrap().replace("<ROOT>", &text_root))
            .unwrap();
        let catalog = Catalog::open(&state).unwrap();
        let home = root.join("home");
        let integrations = Integrations::fixture(
            &root.join("bin/clipper").display().to_string(),
            root.join("clipper-projects"),
        );
        let back = |result: Result<Value>| match result {
            Ok(data) => json!({"ok": true,
                "data": serde_json::from_str::<Value>(&data.to_string().replace(&text_root, "<ROOT>")).unwrap()}),
            Err(e) => json!({"ok": false, "code": e.code}),
        };
        let stalker = format!("{text_root}/home/Movies/Stalker");
        for (name, paths) in [
            ("S1", vec![(format!("{stalker}/S1.stalker"), None)]),
            (
                "S1-relink",
                vec![(
                    format!("{stalker}/S1.stalker"),
                    Some(format!("{text_root}/new/S1.stalker")),
                )],
            ),
            ("S4", vec![(format!("{stalker}/S4.stalker"), None)]),
            ("S5", vec![(format!("{stalker}/S5.stalker"), None)]),
        ] {
            let mine = back(inspect_with(&integrations, &paths, &home));
            assert_eq!(
                mine,
                case["results"][format!("inspect:{name}")],
                "inspect {name}"
            );
        }
        for name in ["S1", "S2", "S4", "S5", "nope"] {
            let params = json!({"path": format!("{stalker}/{name}.stalker")});
            let mine = back(discard_check_with(
                &catalog,
                &params,
                &Places::under(&home),
                &integrations,
                &home,
            ));
            assert_eq!(
                mine,
                case["results"][format!("discard:{name}")],
                "discard {name}"
            );
        }
    }
}
