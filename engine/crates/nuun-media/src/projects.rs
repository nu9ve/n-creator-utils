//! Python `projects.overview` (with `closure.delivery_states`/`delivery_summary` and `finals.published_folder`,
//! `subfolders`, `published_receipts`): what the Projects and Captures screens show about each project.
//! Reads only: one stat per saved final on an online editing disk, and a walk of the published folder when a
//! final is not where it was saved.

use crate::captures;
use crate::catalog::Catalog;
use crate::reads;
use crate::snapshot::kind_of;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, Result};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

const OPAQUE: [&str; 4] = [".fcpbundle", ".stalker", ".app", ".photoslibrary"];
const MEDIA_KINDS: [&str; 4] = ["video", "audio", "image", "raw_image"];
/// projects.py `OPEN_JOBS`: backup is not settled while one of these can still resume.
const OPEN_JOBS: [&str; 5] = [
    "failed",
    "paused",
    "copying",
    "cleaning_source",
    "backup_verified",
];

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn io(error: std::io::Error) -> Error {
    Error::new("operation_failed", error.to_string())
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

/// Python `PurePath.suffix`, lowercased.
fn suffix(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// finals.py `PUBLISHED_NAME`: `^(\d+[_-])?(subidos|publicados)$`, ignoring case.
fn published_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    let rest = match lower.find(|c: char| !c.is_ascii_digit()) {
        Some(i) if i > 0 && matches!(lower.as_bytes()[i], b'_' | b'-') => &lower[i + 1..],
        _ => lower.as_str(),
    };
    rest == "subidos" || rest == "publicados"
}

/// finals.py `published_folder`: an explicit choice wins (also `''`); a chosen folder that is gone means none.
pub fn published_folder(catalog: &Catalog, profile: &str, names: &[String]) -> Result<String> {
    if let Some(chosen) = catalog.get(&format!("finals-published:{profile}"))? {
        let chosen = chosen.as_str().unwrap_or_default();
        return Ok(if names.iter().any(|n| n == chosen) {
            chosen.to_owned()
        } else {
            String::new()
        });
    }
    Ok(names
        .iter()
        .find(|n| published_name(n))
        .cloned()
        .unwrap_or_default())
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// finals.py `subfolders`: visible, real folders directly inside, not opaque bundles, sorted.
fn subfolders(root: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .map_err(io)?
        .flatten()
        .filter(|e| e.path().is_dir() && !is_link(&e.path()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.') && !OPAQUE.contains(&suffix(n).as_str()))
        .collect();
    names.sort();
    Ok(names)
}

/// closure.py `delivery_files`: regular media, text and subtitle files, in `os.walk` order (a folder's files,
/// then its subfolders, both sorted); hidden entries and opaque bundles stay closed, links are not followed.
fn delivery_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // `os.walk` sorts a link to a folder with the folders and never walks into it.
        if entry.path().is_dir() {
            folders.push(name);
        } else {
            files.push(name);
        }
    }
    files.sort();
    folders.sort();
    for name in files {
        let path = root.join(&name);
        let wanted = kind_of(&name).is_some_and(|k| MEDIA_KINDS.contains(&k))
            || [".txt", ".srt", ".vtt"].contains(&suffix(&name).as_str());
        if name.starts_with('.') || !wanted {
            continue;
        }
        if std::fs::symlink_metadata(&path).is_ok_and(|info| info.file_type().is_file()) {
            out.push(path);
        }
    }
    for name in folders {
        let path = root.join(&name);
        if name.starts_with('.') || OPAQUE.contains(&suffix(&name).as_str()) || is_link(&path) {
            continue;
        }
        delivery_files(&path, out);
    }
}

/// finals.py `published_receipts`: verified transfers out of a profile's published folder, by destination.
fn published_receipts(catalog: &Catalog) -> Result<HashMap<(String, String), Value>> {
    let Some(db) = catalog.db() else {
        return Ok(HashMap::new());
    };
    let work: Vec<Value> = catalog
        .volumes()?
        .into_iter()
        .filter(|v| v["role"] == "work")
        .collect();
    let mut statement = db
        .prepare(
            "SELECT json_extract(j.plan,'$.source'),json_extract(j.plan,'$.units[0].profile'),c.payload
             FROM jobs j JOIN copies c ON c.job_id=j.id
             WHERE j.state IN ('completed','source_retained','source_has_new_material')
               AND json_extract(j.plan,'$.source_kind')='delivery'
               AND json_type(j.plan,'$.layout')='object'
             ORDER BY j.updated_at",
        )
        .map_err(sql)?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(sql)?;
    let mut result = HashMap::new();
    for row in rows {
        let (source, profile, copy) = row.map_err(sql)?;
        let (Some(source), Some(profile)) = (
            source.filter(|s| !s.is_empty()),
            profile.filter(|p| !p.is_empty()),
        ) else {
            continue;
        };
        let copy: Value = serde_json::from_str(&copy)
            .map_err(|e| Error::new("operation_failed", e.to_string()))?;
        let origin = Path::new(&source).join(copy["path"].as_str().unwrap_or_default());
        for volume in &work {
            let prefix = volume["finals_prefix"].as_str().unwrap_or("002_finales");
            let root = Path::new(volume["root"].as_str().unwrap_or_default())
                .join(prefix)
                .join(&profile);
            let Ok(rest) = origin.strip_prefix(&root) else {
                continue;
            };
            let parts: Vec<String> = rest
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            if parts.len() > 1 && !published_folder(catalog, &profile, &parts[..1])?.is_empty() {
                result.insert(
                    (
                        copy["volume_id"].as_str().unwrap_or_default().to_owned(),
                        copy["destination"].as_str().unwrap_or_default().to_owned(),
                    ),
                    copy.get("stamp").cloned().unwrap_or(Value::Null),
                );
            }
        }
    }
    Ok(result)
}

/// closure.py `delivery_states`: where each saved delivery stands now.
pub fn delivery_states(
    catalog: &Catalog,
    backend: Option<&dyn Backend>,
    project_id: &str,
    config: Option<&Value>,
) -> Result<Vec<Value>> {
    let owned;
    let config = match config {
        Some(c) => c,
        None => {
            owned = reads::closure_settings(catalog, project_id)?;
            &owned
        }
    };
    let (_, profile, _) = reads::project_record(catalog, project_id)?;
    let volumes: HashMap<String, Value> = catalog
        .volumes()?
        .into_iter()
        .map(|v| (v["id"].as_str().unwrap_or_default().to_owned(), v))
        .collect();
    let mut online: HashMap<String, bool> = HashMap::new();
    let mut receipts: Option<HashMap<(String, String), Value>> = None;
    let mut states = Vec::new();
    let default_published = truthy(config.get("published").unwrap_or(&json!(false)));
    for final_ in config["finals"].as_array().into_iter().flatten() {
        let volume_id = final_["volume_id"].as_str().unwrap_or_default();
        let path = final_["path"].as_str().unwrap_or_default();
        let volume = volumes.get(volume_id).cloned().unwrap_or_else(|| json!({}));
        let published = match final_.get("published") {
            Some(v) => truthy(v),
            None => default_published,
        };
        let name = file_name(path);
        let mut item = json!({"volume_id": final_["volume_id"], "path": final_["path"], "role": final_["role"],
                              "name": name, "published": published});
        if volume["role"] != "work" {
            item["state"] = json!("archived");
            states.push(item);
            continue;
        }
        let up = *online
            .entry(volume_id.to_owned())
            .or_insert_with(|| backend.is_some_and(|b| volumes::online(b, &volume).is_some()));
        let mut state = if published { "published" } else { "awaiting" };
        if !up {
            item["state"] = json!(state);
            item["offline"] = json!(true);
            states.push(item);
            continue;
        }
        let root = PathBuf::from(volume["root"].as_str().unwrap_or_default());
        match std::fs::symlink_metadata(root.join(path)) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let saved = final_
                    .get("stamp")
                    .filter(|s| truthy(s))
                    .cloned()
                    .unwrap_or(json!({}));
                let prefix = volume["finals_prefix"].as_str().unwrap_or("002_finales");
                let profile_root = root.join(prefix).join(&profile);
                let folder = if profile_root.is_dir() {
                    published_folder(catalog, &profile, &subfolders(&profile_root)?)?
                } else {
                    String::new()
                };
                let mut moved = String::new();
                if !folder.is_empty() && !published {
                    let mut files = Vec::new();
                    delivery_files(&profile_root.join(&folder), &mut files);
                    for candidate in files {
                        let Ok(info) = nu9ve_volumes::stamp(&candidate) else {
                            continue;
                        };
                        if file_name(&candidate.to_string_lossy()) == name
                            && json!(info.size) == saved["size"]
                            && json!(info.mtime_ns) == saved["mtime_ns"]
                            && json!(info.inode) == saved["inode"]
                        {
                            moved = candidate
                                .strip_prefix(&root)
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            break;
                        }
                    }
                }
                if !moved.is_empty() {
                    state = "moved";
                    item["current_path"] = json!(moved);
                } else {
                    if receipts.is_none() {
                        receipts = Some(published_receipts(catalog)?);
                    }
                    let matched =
                        receipts
                            .as_ref()
                            .into_iter()
                            .flatten()
                            .any(|((_, dest), stamp)| {
                                file_name(dest) == name
                                    && stamp.get("size").unwrap_or(&Value::Null)
                                        == saved.get("size").unwrap_or(&Value::Null)
                            });
                    state = if matched {
                        "published_archived"
                    } else {
                        "missing"
                    };
                }
            }
            Err(e) => return Err(io(e)),
        }
        item["state"] = json!(state);
        states.push(item);
    }
    Ok(states)
}

/// closure.py `delivery_summary`: what still waits on the editing disk, never a backup claim.
pub fn delivery_summary(states: &[Value]) -> Value {
    let awaiting: Vec<&Value> = states.iter().filter(|s| s["state"] == "awaiting").collect();
    json!({
        "awaiting": awaiting.len(),
        "awaiting_names": awaiting.iter().take(20).map(|s| s["name"].clone()).collect::<Vec<_>>(),
        "published": states.iter().filter(|s| s["state"] == "published" || s["state"] == "moved").count(),
        "missing": states.iter().filter(|s| s["state"] == "missing").count(),
        "offline": states.iter().any(|s| truthy(&s["offline"])),
    })
}

/// The `closure_deliveries` method.
pub fn closure_deliveries(
    catalog: &Catalog,
    backend: &dyn Backend,
    params: &Value,
) -> Result<Value> {
    let project_id = params
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("operation_failed", "'project_id'"))?;
    let cached = volumes::Cached::new(backend);
    Ok(json!(delivery_states(
        catalog,
        Some(&cached),
        project_id,
        None
    )?))
}

#[derive(Default)]
struct Entry {
    key: String,
    project_id: String,
    project: String,
    profile: String,
    assets: u64,
    kinds: Map<String, Value>,
    cameras: BTreeSet<String>,
    places: BTreeSet<String>,
    jobs: BTreeSet<String>,
    pending_sessions: u64,
    work: [u64; 3],
    preview: Value,
    seen_at: String,
}

/// projects.py `overview(catalog, sessions, backend)`: newest first.
pub fn overview(
    catalog: &Catalog,
    sessions: &[Value],
    backend: Option<&dyn Backend>,
) -> Result<Vec<Value>> {
    let Some(db) = catalog.db() else {
        return Ok(Vec::new());
    };
    let mut aliases: HashMap<String, String> = HashMap::new();
    let mut names: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut statement = db
            .prepare("SELECT alias,project_id FROM project_aliases ORDER BY profile")
            .map_err(sql)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(sql)?;
        for row in rows {
            let (alias, project) = row.map_err(sql)?;
            aliases
                .entry(alias.clone())
                .or_insert_with(|| project.clone());
            names.entry(project).or_default().push(alias);
        }
    }
    let mut profiles: HashMap<String, (String, String)> = HashMap::new();
    {
        let mut statement = db
            .prepare("SELECT id,title,profile FROM projects")
            .map_err(sql)?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(sql)?;
        for row in rows {
            let (id, title, profile) = row.map_err(sql)?;
            profiles.insert(id, (title, profile));
        }
    }
    let mut jobs: HashMap<String, (String, String)> = HashMap::new();
    {
        let mut statement = db
            .prepare("SELECT id,state,updated_at FROM jobs")
            .map_err(sql)?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(sql)?;
        for row in rows {
            let (id, state, updated) = row.map_err(sql)?;
            jobs.insert(id, (state, updated));
        }
    }
    // Python fills a dict: insertion order, then a stable sort.
    let mut order: Vec<String> = Vec::new();
    let mut entries: HashMap<String, Entry> = HashMap::new();
    let mut entry = |key: String, title: &str, profile: &str, project_id: &str| -> String {
        if !entries.contains_key(&key) {
            order.push(key.clone());
            entries.insert(
                key.clone(),
                Entry {
                    key: key.clone(),
                    project_id: project_id.to_owned(),
                    project: title.to_owned(),
                    profile: profile.to_owned(),
                    preview: json!({}),
                    ..Default::default()
                },
            );
        }
        key
    };
    let mut assets_seen = Vec::new();
    for asset in crate::snapshot::library(catalog)? {
        let project_id = asset["project_id"].as_str().unwrap_or_default().to_owned();
        let project = asset["project"].as_str().unwrap_or_default().to_owned();
        let key = if project_id.is_empty() {
            format!("draft:{project}")
        } else {
            project_id.clone()
        };
        let key = entry(
            key,
            &project,
            asset["profile"].as_str().unwrap_or_default(),
            &project_id,
        );
        assets_seen.push((key, asset));
    }
    let mut session_keys = Vec::new();
    for session in sessions {
        let title = session["tag"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if title.is_empty() || session["status"] != "completed" || truthy(&session["archived"]) {
            continue;
        }
        let project_id = aliases.get(&title).cloned().unwrap_or_default();
        let known = profiles.get(&project_id).cloned();
        let key = if project_id.is_empty() {
            format!("draft:{title}")
        } else {
            project_id.clone()
        };
        let (shown, profile) = known.unwrap_or((title.clone(), String::new()));
        let key = entry(key, &shown, &profile, &project_id);
        session_keys.push((key, session));
    }
    for (key, asset) in assets_seen {
        let item = entries.get_mut(&key).expect("entry");
        item.assets += 1;
        let kind = asset["kind"].as_str().unwrap_or_default().to_owned();
        let count = item.kinds.get(&kind).and_then(Value::as_u64).unwrap_or(0) + 1;
        item.kinds.insert(kind, json!(count));
        let slot = match asset["work_delivery_status"].as_str() {
            Some("pending") => 0,
            Some("ready") => 1,
            Some("skipped") => 2,
            other => {
                return Err(Error::new(
                    "operation_failed",
                    format!("'work_{}'", other.unwrap_or("None")),
                ));
            }
        };
        item.work[slot] += 1;
        if let Some(job) = asset["job_id"].as_str() {
            item.jobs.insert(job.to_owned());
        }
        if let Some(camera) = asset["camera"]
            .as_str()
            .filter(|c| !c.is_empty() && *c != "generic")
        {
            item.cameras.insert(camera.to_owned());
        }
        let place: Vec<&str> = [&asset["country"], &asset["city"]]
            .into_iter()
            .filter_map(Value::as_str)
            .filter(|p| !p.is_empty())
            .collect();
        if !place.is_empty() {
            item.places.insert(place.join("/"));
        }
    }
    for (key, session) in session_keys {
        let item = entries.get_mut(&key).expect("entry");
        item.pending_sessions += 1;
        let created = session["created_at"].as_str().unwrap_or_default();
        if created > item.seen_at.as_str() {
            item.seen_at = created.to_owned();
        }
        if item.preview.as_object().is_some_and(Map::is_empty)
            && let Some(preview) = session["preview_path"].as_str().filter(|p| !p.is_empty())
        {
            item.preview = json!({"path": session["path"], "preview_path": preview});
        }
    }
    let mut result = Vec::new();
    for key in order {
        let item = entries.remove(&key).expect("entry");
        let state = if item.project_id.is_empty() {
            "active".to_owned()
        } else {
            catalog
                .get(&format!("project-state:{key}"))?
                .and_then(|v| v["state"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "active".into())
        };
        let open_jobs = item
            .jobs
            .iter()
            .filter(|j| {
                jobs.get(*j)
                    .is_some_and(|(s, _)| OPEN_JOBS.contains(&s.as_str()))
            })
            .count();
        let stage = if state == "finished" {
            "finished"
        } else if item.assets == 0 {
            "draft"
        } else if item.pending_sessions > 0 || item.work[0] > 0 || open_jobs > 0 {
            "in_progress"
        } else {
            "backed_up"
        };
        let touched = catalog
            .get(&format!("project-touch:{key}"))?
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let closable = if item.project != "finales" {
            item.project_id.clone()
        } else {
            String::new()
        };
        let deliveries = deliveries(catalog, backend, &closable)?;
        let released = !closable.is_empty()
            && catalog
                .get(&format!("project-released:{closable}"))?
                .is_some_and(|v| truthy(&v));
        let related: Vec<Value> = if closable.is_empty() {
            Vec::new()
        } else {
            reads::related(catalog, &closable)?
                .into_iter()
                .map(|id| {
                    let title = profiles.get(&id).map(|p| p.0.clone()).unwrap_or_default();
                    json!({"id": id, "title": title})
                })
                .collect()
        };
        let suggestions: Vec<Value> = if closable.is_empty() {
            Vec::new()
        } else {
            match reads::link_candidates(catalog, &closable) {
                Ok(list) => list
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["suggested"] == true && c["linked"] == false)
                    .map(|c| c["id"].clone())
                    .collect(),
                Err(_) => Vec::new(),
            }
        };
        let updated = std::iter::once(touched.clone())
            .chain(std::iter::once(item.seen_at.clone()))
            .chain(
                item.jobs
                    .iter()
                    .filter_map(|j| jobs.get(j).map(|(_, u)| u.clone())),
            )
            .max()
            .unwrap_or_default();
        let aliases: Vec<&String> = names
            .get(&item.project_id)
            .into_iter()
            .flatten()
            .filter(|n| **n != item.project)
            .collect();
        result.push(json!({
            "key": item.key, "project_id": item.project_id, "project": item.project, "profile": item.profile,
            "assets": item.assets, "kinds": item.kinds, "cameras": item.cameras, "places": item.places,
            "pending_sessions": item.pending_sessions, "work_pending": item.work[0], "work_ready": item.work[1],
            "work_skipped": item.work[2], "preview": item.preview, "deliveries": deliveries, "released": released,
            "related": related, "link_suggestions": suggestions, "state": state, "stage": stage,
            "jobs": item.jobs.len(), "open_jobs": open_jobs, "aliases": aliases, "updated_at": updated,
        }));
    }
    let key = |e: &Value| {
        (
            e["updated_at"].as_str().unwrap_or_default().to_owned(),
            e["project"].as_str().unwrap_or_default().to_owned(),
        )
    };
    result.sort_by_key(|e| std::cmp::Reverse(key(e)));
    Ok(result)
}

/// projects.py `_deliveries`.
fn deliveries(catalog: &Catalog, backend: Option<&dyn Backend>, project_id: &str) -> Result<Value> {
    let config = if project_id.is_empty() {
        None
    } else {
        catalog.get(&format!("project-closure:{project_id}"))?
    };
    match config {
        Some(config) if truthy(&config) && truthy(&config["finals"]) => Ok(delivery_summary(
            &delivery_states(catalog, backend, project_id, Some(&config))?,
        )),
        _ => Ok(
            json!({"awaiting": 0, "awaiting_names": [], "published": 0, "missing": 0, "offline": false}),
        ),
    }
}

/// The `capture_overview` method: the Stalker libraries plus every project's overview.
pub fn capture_overview(catalog: &Catalog, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let mut data = captures::capture_libraries(catalog, params)?;
    let cached = volumes::Cached::new(backend);
    let sessions = data["sessions"].as_array().cloned().unwrap_or_default();
    data["projects"] = json!(overview(catalog, &sessions, Some(&cached))?);
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Online when the registered root exists: answers the identity the catalog saved for it.
    struct Registered(HashMap<String, Value>);

    impl Backend for Registered {
        fn mounted_roots(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        fn describe(&self, root: &Path, _: bool) -> Result<Value> {
            nu9ve_json::require(root.is_dir(), "volume_offline", "Fixture disk disconnected")?;
            self.0
                .get(&root.display().to_string())
                .cloned()
                .ok_or_else(|| Error::new("volume_offline", "Unknown fixture disk"))
        }
        fn usage(&self, _: &Path) -> Option<(u64, u64)> {
            None
        }
    }

    /// Phase L / step 6: the same disks, finals, receipts, aliases and sessions as Python's, the same answers.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn deliveries_and_overview_match_python() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let case = &golden["projects"];
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
        let catalog = Catalog::open(&state).unwrap();
        let backend = Registered(
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
        );
        let back = |value: Value| -> Value {
            serde_json::from_str(&value.to_string().replace(&text_root, "<ROOT>")).unwrap()
        };
        let deliveries = delivery_states(&catalog, Some(&backend), "p1", None).unwrap();
        assert_eq!(back(json!(deliveries)), case["deliveries"]);
        let sessions = case["sessions"].as_array().unwrap();
        let projects = overview(&catalog, sessions, Some(&backend)).unwrap();
        let theirs = case["projects"].as_array().unwrap();
        assert_eq!(projects.len(), theirs.len());
        for (mine, theirs) in projects.into_iter().zip(theirs) {
            assert_eq!(back(mine), *theirs, "{}", theirs["key"]);
        }
    }

    /// A final moved by hand into the published folder is found by name, size, mtime and inode.
    #[test]
    fn a_final_moved_to_the_published_folder_is_found() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let moved = root.join("work/002_finales/e64/01_subidos/2026/final.mp4");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::write(&moved, "f").unwrap();
        let s = nu9ve_volumes::stamp(&moved).unwrap();
        let state = root.join("state");
        let db = crate::catalog::fixture::create(&state);
        let work = root.join("work").display().to_string();
        let identity = json!({"mode": "fixture", "volume_uuid": "w"});
        db.execute(
            "INSERT INTO volumes VALUES('w','ESSD',?)",
            [json!({"id": "w", "name": "ESSD", "role": "work", "root": work, "identity": identity}).to_string()],
        )
        .unwrap();
        db.execute("INSERT INTO projects VALUES('p1','e64','terra')", [])
            .unwrap();
        drop(db);
        let catalog = Catalog::open(&state).unwrap();
        let backend = Registered(HashMap::from([(work, identity)]));
        let config = json!({"finals": [{"role": "horizontal", "volume_id": "w",
            "path": "002_finales/e64/terra/final.mp4",
            "stamp": {"size": s.size, "mtime_ns": s.mtime_ns, "inode": s.inode}}]});
        let states = delivery_states(&catalog, Some(&backend), "p1", Some(&config)).unwrap();
        assert_eq!(states[0]["state"], "moved");
        assert_eq!(
            states[0]["current_path"],
            "002_finales/e64/01_subidos/2026/final.mp4"
        );
    }

    #[test]
    fn published_folder_names_follow_finals_py() {
        for name in ["subidos", "Publicados", "01_subidos", "2-PUBLICADOS"] {
            assert!(published_name(name), "{name}");
        }
        for name in [
            "subidos2",
            "_subidos",
            "x_subidos",
            "01subidos",
            "01__subidos",
        ] {
            assert!(!published_name(name), "{name}");
        }
    }
}
