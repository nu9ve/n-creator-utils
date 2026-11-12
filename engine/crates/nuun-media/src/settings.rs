//! E1 · writes that change only the catalog, never media: defaults, integration settings, profiles, devices,
//! project state and names, work state, publications, capture tags, the published finals folder, closeout links,
//! dismissed jobs, cancelled plans and discarded sessions. Each one is Python's, step by step, and takes the
//! writer lock exactly where Python does (so, as in Python, dismissing a job never waits for a running copy).

use crate::catalog::{WritableCatalog, writer_lock};
use crate::rules::{relative, slug};
use nu9ve_json::{Error, Result, require};
use rusqlite::OptionalExtension;
use serde_json::{Map, Value, json};
use std::path::Path;

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn now() -> String {
    nu9ve_time::now_utc()
}

/// A Python call with keyword arguments: an unknown one is a `TypeError`.
fn only(params: &Value, allowed: &[&str], function: &str) -> Result<()> {
    if let Some(unexpected) = params
        .as_object()
        .and_then(|p| p.keys().find(|k| !allowed.contains(&k.as_str())))
    {
        return Err(Error::new(
            "operation_failed",
            format!("{function}() got an unexpected keyword argument '{unexpected}'"),
        ));
    }
    Ok(())
}

/// `params['name']` (a missing key is a `KeyError`).
fn required<'a>(params: &'a Value, name: &str) -> Result<&'a Value> {
    params
        .get(name)
        .ok_or_else(|| Error::new("operation_failed", format!("'{name}'")))
}

fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str> {
    required(params, name)?
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", format!("'{name}' must be a string")))
}

/// The `defaults` method with parameters: the suggestions a new ingest starts from.
pub fn defaults(home: &Path, params: &Value) -> Result<Value> {
    let allowed = ["project", "profile", "country", "city", "archive", "work"];
    let map = params.as_object().cloned().unwrap_or_default();
    require(
        map.keys().all(|k| allowed.contains(&k.as_str())) && map.values().all(Value::is_string),
        "invalid_settings",
        "Invalid defaults",
    )?;
    let catalog = WritableCatalog::open(home)?;
    let mut value = catalog
        .get("defaults")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    value.extend(map);
    catalog.set("defaults", &Value::Object(value))?;
    Ok(catalog
        .get("defaults")?
        .unwrap_or_else(|| json!({"profile": "e64", "country": "mx", "city": "hermosillo"})))
}

/// The `integration_settings` method with parameters: Clipper and Stalker executables, capture override, retention.
pub fn integration_settings(home: &Path, params: &Value) -> Result<Value> {
    let allowed = [
        "clipper_executable",
        "stalker_executable",
        "capture_override",
        "retain_stalker_source",
    ];
    let map = params.as_object().cloned().unwrap_or_default();
    require(
        map.keys().all(|k| allowed.contains(&k.as_str()))
            && map.iter().all(|(k, v)| {
                if k == "retain_stalker_source" {
                    v.is_boolean()
                } else {
                    v.is_string()
                }
            }),
        "invalid_settings",
        "Invalid integration settings",
    )?;
    let catalog = WritableCatalog::open(home)?;
    {
        let _lock = writer_lock(home)?;
        for (key, value) in &map {
            catalog.set(&key.replace('_', "-"), value)?;
        }
    }
    let mut out = Map::new();
    for key in [
        "clipper_executable",
        "stalker_executable",
        "capture_override",
    ] {
        out.insert(
            key.into(),
            catalog.get(&key.replace('_', "-"))?.unwrap_or(json!("")),
        );
    }
    out.insert(
        "retain_stalker_source".into(),
        catalog
            .get("retain-stalker-source")?
            .unwrap_or(json!(false)),
    );
    Ok(Value::Object(out))
}

/// The `dismiss_job` method: a reminder choice only; the job keeps its state and can still resume.
pub fn dismiss_job(home: &Path, params: &Value) -> Result<Value> {
    let id = text(params, "job_id")?;
    let catalog = WritableCatalog::open(home)?;
    let found: Option<String> = catalog
        .db()
        .query_row("SELECT id FROM jobs WHERE id=?", [id], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    require(found.is_some(), "unknown_job", "Job not found")?;
    let dismissed = params.get("dismissed").filter(|d| d.is_boolean());
    let dismissed =
        dismissed.ok_or_else(|| Error::new("invalid_request", "dismissed must be boolean"))?;
    catalog.set(&format!("job-dismissed:{id}"), dismissed)?;
    Ok(json!({"job_id": id, "dismissed": dismissed}))
}

/// projects.py `key_for`: a named project without receipts yet is a draft.
fn key_for(catalog: &WritableCatalog, title: &str) -> Result<String> {
    let found: Option<String> = catalog
        .db()
        .query_row(
            "SELECT project_id FROM project_aliases WHERE alias=? ORDER BY profile LIMIT 1",
            [title],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(found.unwrap_or_else(|| format!("draft:{title}")))
}

/// The `touch_project` method: the project rises in the quick picks.
pub fn touch_project(home: &Path, params: &Value) -> Result<Value> {
    let key = params["key"].as_str().filter(|k| !k.is_empty());
    let key = key.ok_or_else(|| Error::new("invalid_request", "Project key required"))?;
    WritableCatalog::open(home)?.set(&format!("project-touch:{key}"), &json!(now()))?;
    Ok(json!({"key": key}))
}

/// The `set_project_state` method: declared by the person, never inferred; it deletes nothing.
pub fn set_project_state(home: &Path, params: &Value) -> Result<Value> {
    let id = text(params, "project_id")?;
    let state = required(params, "state")?;
    require(
        ["active", "paused", "finished"].iter().any(|s| state == s),
        "invalid_project_state",
        "Project state must be active, paused or finished",
    )?;
    let catalog = WritableCatalog::open(home)?;
    let found: Option<i64> = catalog
        .db()
        .query_row("SELECT 1 FROM projects WHERE id=?", [id], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    require(found.is_some(), "unknown_project", "Project not found")?;
    catalog.set(
        &format!("project-state:{id}"),
        &json!({"state": state, "updated_at": now()}),
    )?;
    Ok(json!({"project_id": id, "state": state}))
}

/// projects.py `_retag`: capture tags and the default project follow a renamed title.
fn retag(catalog: &WritableCatalog, old: &[String], title: &str) -> Result<()> {
    let rows: Vec<(String, String)> = {
        let mut statement = catalog
            .db()
            .prepare("SELECT key,value FROM metadata WHERE key LIKE 'capture-tag:%'")
            .map_err(sql)?;
        let found = statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(sql)?;
        found.collect::<rusqlite::Result<_>>().map_err(sql)?
    };
    for (key, raw) in rows {
        let mut tag: Value = serde_json::from_str(&raw)
            .map_err(|e| Error::new("operation_failed", e.to_string()))?;
        if tag["name"]
            .as_str()
            .is_some_and(|n| old.iter().any(|o| o == n))
        {
            tag["name"] = json!(title);
            catalog.set(&key, &tag)?;
        }
    }
    if let Some(mut defaults) = catalog.get("defaults")?
        && defaults["project"]
            .as_str()
            .is_some_and(|p| old.iter().any(|o| o == p))
    {
        defaults["project"] = json!(title);
        catalog.set("defaults", &defaults)?;
    }
    Ok(())
}

/// The `rename_project` method: catalog, tags and defaults follow the new name; old copies keep their folders and
/// the old title stays an alias that still resolves them.
pub fn rename_project(home: &Path, params: &Value) -> Result<Value> {
    let key = match params.get("key") {
        Some(Value::String(k)) if !k.is_empty() => k.clone(),
        _ => text(params, "project_id")?.to_owned(),
    };
    let title = slug(text(params, "title")?)?;
    let catalog = WritableCatalog::open(home)?;
    if let Some(old) = key.strip_prefix("draft:") {
        let _lock = writer_lock(home)?;
        retag(&catalog, &[old.to_owned()], &title)?;
        let new_key = key_for(&catalog, &title)?;
        catalog.set(&format!("project-touch:{new_key}"), &json!(now()))?;
        return Ok(json!({"key": key_for(&catalog, &title)?, "title": title, "previous": old}));
    }
    let previous: Option<(String, String)> = catalog
        .db()
        .query_row(
            "SELECT title,profile FROM projects WHERE id=?",
            [&key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let (previous, profile) =
        previous.ok_or_else(|| Error::new("unknown_project", "Project not found"))?;
    {
        // Catalog.rename_project: one transaction under the writer lock.
        let _lock = writer_lock(home)?;
        let db = catalog.db();
        let other: Option<String> = db
            .query_row(
                "SELECT project_id FROM project_aliases WHERE profile=? AND alias=?",
                [&profile, &title],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
        require(
            other.as_ref().is_none_or(|o| *o == key),
            "project_conflict",
            "Project title is already in use",
        )?;
        db.execute_batch("BEGIN").map_err(sql)?;
        let result = (|| -> Result<()> {
            db.execute(
                "INSERT OR IGNORE INTO project_aliases VALUES(?,?,?)",
                [&profile, &title, &key],
            )
            .map_err(sql)?;
            db.execute("UPDATE projects SET title=? WHERE id=?", [&title, &key])
                .map_err(sql)?;
            let assets: Vec<(String, String)> = {
                let mut statement = db.prepare("SELECT id,payload FROM assets").map_err(sql)?;
                let rows = statement
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(sql)?;
                rows.collect::<rusqlite::Result<_>>().map_err(sql)?
            };
            for (id, raw) in assets {
                let mut asset: Value = serde_json::from_str(&raw)
                    .map_err(|e| Error::new("operation_failed", e.to_string()))?;
                if asset["project_id"] == key.as_str() {
                    asset["project"] = json!(title);
                    let payload =
                        String::from_utf8(nu9ve_json::canonical(&asset)).unwrap_or_default();
                    db.execute("UPDATE assets SET payload=? WHERE id=?", [&payload, &id])
                        .map_err(sql)?;
                }
            }
            Ok(())
        })();
        db.execute_batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })
            .map_err(sql)?;
        result?;
    }
    {
        let _lock = writer_lock(home)?;
        retag(&catalog, std::slice::from_ref(&previous), &title)?;
    }
    Ok(json!({"key": key, "project_id": key, "title": title, "previous": previous}))
}

fn asset_record(catalog: &WritableCatalog, id: &str) -> Result<Value> {
    let raw: Option<String> = catalog
        .db()
        .query_row("SELECT payload FROM assets WHERE id=?", [id], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    let raw = raw.ok_or_else(|| Error::new("unknown_asset", "Asset not found"))?;
    serde_json::from_str(&raw).map_err(|e| Error::new("operation_failed", e.to_string()))
}

/// The `set_work_state` method: some assets wait for, or skip, their editing copy.
pub fn set_work_state(home: &Path, params: &Value) -> Result<Value> {
    let ids = required(params, "asset_ids")?.clone();
    let state = required(params, "state")?;
    require(
        state == "pending" || state == "skipped",
        "invalid_state",
        "Choose pending or skipped",
    )?;
    let state = state.as_str().unwrap_or_default().to_owned();
    let catalog = WritableCatalog::open(home)?;
    let _lock = writer_lock(home)?;
    for id in ids.as_array().into_iter().flatten() {
        let id = id.as_str().unwrap_or_default();
        let asset = asset_record(&catalog, id)?;
        catalog
            .db()
            .execute(
                "UPDATE work SET state=?,updated_at=? WHERE asset_id=?",
                [state.as_str(), &now(), id],
            )
            .map_err(sql)?;
        catalog.event(
            asset["job_id"].as_str().unwrap_or_default(),
            &format!("work_{state}"),
            &json!({"asset_id": id}),
        )?;
    }
    Ok(json!({"updated": ids, "state": state}))
}

/// `urlparse(url).scheme`: the lowercased scheme, or `''`.
fn url_scheme(url: &str) -> String {
    match url.find(':') {
        Some(i)
            if i > 0
                && url[..i].starts_with(|c: char| c.is_ascii_alphabetic())
                && url[..i]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) =>
        {
            url[..i].to_ascii_lowercase()
        }
        _ => String::new(),
    }
}

/// The `set_publication` method: where a delivery's variant stands on a channel.
pub fn set_publication(home: &Path, params: &Value) -> Result<Value> {
    only(
        params,
        &["asset_id", "variant", "channel", "state", "url"],
        "set_publication",
    )?;
    let state = required(params, "state")?;
    require(
        ["candidate", "approved", "published"]
            .iter()
            .any(|s| state == s),
        "invalid_state",
        "Invalid publication state",
    )?;
    let variant = relative(params["variant"].as_str().unwrap_or("\0"))?.to_owned();
    let channel = params["channel"]
        .as_str()
        .filter(|c| !c.is_empty() && c.chars().count() <= 160);
    let channel = channel.ok_or_else(|| Error::new("invalid_channel", "Choose a channel"))?;
    let url = params.get("url").cloned().unwrap_or(Value::Null);
    require(
        url.is_null()
            || url
                .as_str()
                .is_some_and(|u| ["https", "http"].contains(&url_scheme(u).as_str())),
        "invalid_url",
        "Publication URL must be HTTP(S)",
    )?;
    let id = text(params, "asset_id")?;
    let catalog = WritableCatalog::open(home)?;
    let asset = asset_record(&catalog, id)?;
    require(
        asset["kind"] == "delivery",
        "invalid_asset",
        "Publication state belongs to a delivery",
    )?;
    require(
        asset["paths"]
            .as_array()
            .is_some_and(|p| p.iter().any(|v| v == variant.as_str())),
        "unknown_variant",
        "Variant is not a member of this delivery",
    )?;
    {
        let _lock = writer_lock(home)?;
        catalog
            .db()
            .execute(
                "INSERT OR REPLACE INTO publications VALUES(?,?,?,?,?,?)",
                rusqlite::params![id, variant, channel, state.as_str(), url.as_str(), now()],
            )
            .map_err(sql)?;
    }
    catalog.event(
        asset["job_id"].as_str().unwrap_or_default(),
        &format!("publication_{}", state.as_str().unwrap_or_default()),
        &json!({"asset_id": id, "variant": variant, "channel": channel}),
    )?;
    Ok(json!({"asset_id": id, "variant": variant, "channel": channel, "state": state, "url": url}))
}

fn uuid_text(value: &Value) -> Result<String> {
    value
        .as_str()
        .and_then(crate::captures::python_uuid)
        .ok_or_else(|| Error::new("operation_failed", "badly formed hexadecimal UUID string"))
}

/// The `save_capture_tags` method (and `save_capture_tag` for one): one writer for a whole batch, and the project
/// rises in the quick picks right away.
pub fn save_capture_tags(home: &Path, params: &Value, one: bool) -> Result<Value> {
    let function = if one {
        "save_capture_tag"
    } else {
        "save_capture_tags"
    };
    let ids_key = if one { "session_id" } else { "session_ids" };
    only(params, &[ids_key, "project", "country"], function)?;
    let ids: Vec<Value> = if one {
        vec![required(params, ids_key)?.clone()]
    } else {
        let list = required(params, ids_key)?
            .as_array()
            .filter(|l| !l.is_empty());
        list.ok_or_else(|| Error::new("invalid_request", "Sessions required"))?
            .clone()
    };
    let sessions = ids.iter().map(uuid_text).collect::<Result<Vec<_>>>()?;
    let country = match params.get("country") {
        Some(c) => c
            .as_str()
            .ok_or_else(|| Error::new("invalid_tag", "Invalid project/profile tag"))?,
        None => "mx",
    };
    let project = params["project"]
        .as_str()
        .ok_or_else(|| Error::new("invalid_tag", "Invalid project/profile tag"))?;
    let value = json!({"name": slug(project)?, "country": slug(country)?});
    let catalog = WritableCatalog::open(home)?;
    let _lock = writer_lock(home)?;
    for session in &sessions {
        catalog.set(&format!("capture-tag:{session}"), &value)?;
    }
    let key = key_for(&catalog, value["name"].as_str().unwrap_or_default())?;
    catalog.set(&format!("project-touch:{key}"), &json!(now()))?;
    Ok(value)
}

/// The `set_finals_folder` method: a profile's published folder, `''` for none, or `null` to detect it again.
pub fn set_finals_folder(home: &Path, params: &Value) -> Result<Value> {
    let profile = required(params, "profile")?;
    let folder = params.get("folder").cloned().unwrap_or(Value::Null);
    let folder_text = folder.as_str().filter(|f| !f.is_empty()).unwrap_or("x");
    for value in [profile.as_str(), Some(folder_text)] {
        require(
            value.is_some_and(|v| !v.contains('/') && v != "." && v != ".." && !v.starts_with('.')),
            "invalid_folder",
            "Use a folder directly inside the profile",
        )?;
    }
    let profile = profile.as_str().unwrap_or_default();
    WritableCatalog::open(home)?.set(&format!("finals-published:{profile}"), &folder)?;
    Ok(json!({"profile": profile, "folder": folder}))
}

/// The `cancel_plan` method: a reviewed plan that never started is marked cancelled.
pub fn cancel_plan(home: &Path, params: &Value) -> Result<Value> {
    let id = text(params, "plan_id")?;
    let catalog = WritableCatalog::open(home)?;
    let _lock = writer_lock(home)?;
    let key = format!("reviewed-plan:{id}");
    let mut plan = catalog
        .get(&key)?
        .ok_or_else(|| Error::new("unknown_plan", "Unknown reviewed plan"))?;
    let started: Option<i64> = catalog
        .db()
        .query_row("SELECT 1 FROM jobs WHERE id=?", [id], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    require(
        started.is_none(),
        "retitle_plan_active",
        "An ingest job already started; cancellation cannot discard its journal",
    )?;
    if let Some(map) = plan.as_object_mut() {
        map.insert("cancelled".into(), json!(true));
    }
    catalog.set(&key, &plan)?;
    Ok(json!({"plan_id": id, "cancelled": true}))
}

/// The `capture_discarded` method: a note of the decision; the tag stays so a session restored from the Trash
/// keeps its project.
pub fn capture_discarded(home: &Path, params: &Value) -> Result<Value> {
    let session = uuid_text(required(params, "session_id")?)?;
    let value = json!({"name": required(params, "name")?, "trash_path": required(params, "trash_path")?,
                       "bytes": required(params, "bytes")?, "discarded_at": now()});
    WritableCatalog::open(home)?.set(&format!("capture-discarded:{session}"), &value)?;
    Ok(value)
}

/// Save the reviewed delivery decisions and the stamps of the selected finals as one catalog transaction.
pub fn closure_save(
    home: &Path,
    backend: Option<&dyn crate::volumes::Backend>,
    params: &Value,
) -> Result<Value> {
    use crate::{projects, reads, tree, volumes};
    use std::collections::BTreeSet;

    only(
        params,
        &[
            "project_id",
            "required",
            "no_more",
            "published",
            "finals",
            "profile_default",
        ],
        "save",
    )?;
    let project_id = text(params, "project_id")?;
    let _lock = writer_lock(home)?;
    let catalog = WritableCatalog::open(home)?;
    let reader = crate::catalog::Catalog::open(home)?;
    let (_, profile, _) = reads::project_record(&reader, project_id)?;
    let previous = reads::closure_settings(&reader, project_id)?;
    let roles = ["horizontal", "vertical", "thumbnail"];
    let required_roles = required(params, "required")?;
    let required_roles = required_roles
        .as_array()
        .filter(|items| {
            !items.is_empty()
                && items
                    .iter()
                    .all(|v| v.as_str().is_some_and(|r| roles.contains(&r)))
        })
        .ok_or_else(|| Error::new("invalid_request", "Elige los tipos de entrega requeridos"))?;
    let required_roles: BTreeSet<&str> = required_roles.iter().filter_map(Value::as_str).collect();
    let decision = |value: &Value| {
        value
            .as_bool()
            .ok_or_else(|| Error::new("invalid_request", "Decisiones inválidas"))
    };
    let no_more = decision(required(params, "no_more")?)?;
    let published = decision(required(params, "published")?)?;
    let profile_default = params
        .get("profile_default")
        .map(decision)
        .transpose()?
        .unwrap_or(false);
    let finals = required(params, "finals")?
        .as_array()
        .filter(|v| v.len() <= 1000)
        .ok_or_else(|| Error::new("invalid_request", "Selección de finales inválida"))?;
    let receipts = if !published
        && finals
            .iter()
            .any(|v| v.is_object() && v.get("published").is_none())
    {
        projects::published_receipts(&reader)?
    } else {
        Default::default()
    };
    let key = format!("project-closure:{project_id}");
    let mut other_finals = BTreeSet::new();
    let mut statement = catalog
        .db()
        .prepare("SELECT value FROM metadata WHERE key LIKE 'project-closure:%' AND key != ?")
        .map_err(sql)?;
    let configs = statement
        .query_map([&key], |row| row.get::<_, String>(0))
        .map_err(sql)?;
    for config in configs {
        let value: Value = serde_json::from_str(&config.map_err(sql)?)
            .map_err(|error| Error::new("operation_failed", error.to_string()))?;
        for item in value["finals"].as_array().into_iter().flatten() {
            if let (Some(id), Some(path)) = (item["volume_id"].as_str(), item["path"].as_str()) {
                other_finals.insert((id.to_owned(), path.to_owned()));
            }
        }
    }
    drop(statement);
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    let mut event_finals = Vec::new();
    let cached = backend.map(volumes::Cached::new);
    for item in finals {
        let role = item["role"]
            .as_str()
            .filter(|role| roles.contains(role) || *role == "auxiliary")
            .ok_or_else(|| Error::new("invalid_request", "Rol de entrega inválido"))?;
        let volume = reader.volume(text(item, "volume_id")?)?;
        let id = volume["id"].as_str().unwrap_or_default();
        let path = relative(text(item, "path")?)?;
        let finals_prefix = volume["finals_prefix"].as_str().unwrap_or("002_finales");
        let prefix = format!("{finals_prefix}/{profile}/");
        require(
            matches!(volume["role"].as_str(), Some("archive" | "work"))
                && path.starts_with(&prefix),
            "unsafe_path",
            "Selecciona finales del mismo perfil",
        )?;
        require(
            !path.split('/').any(|part| {
                part.starts_with('.')
                    || projects::OPAQUE
                        .iter()
                        .any(|suffix| part.to_ascii_lowercase().ends_with(suffix))
            }),
            "unsafe_path",
            "No se adoptan bibliotecas ni bundles como finales",
        )?;
        require(
            seen.insert((id.to_owned(), path.to_owned())),
            "invalid_request",
            "Final repetido",
        )?;
        let backend = cached
            .as_ref()
            .ok_or_else(|| Error::new("not_ported", "No volume backend"))?;
        volumes::check(backend, &volume)?;
        let root = Path::new(volume["root"].as_str().unwrap_or_default());
        let stamp = tree::Root::open(root)?
            .stat(path)?
            .ok_or_else(|| Error::new("operation_failed", format!("[FileNotFoundError] {path}")))?;
        let stamp = tree::stamp_json(&stamp);
        let selection = (id.to_owned(), path.to_owned());
        require(
            !other_finals.contains(&selection),
            "final_already_linked",
            "Este final ya pertenece a otro proyecto",
        )?;
        let selected = item
            .get("published")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| Error::new("invalid_request", "Publicado debe ser booleano"))
            })
            .transpose()?;
        let profile_root = root.join(finals_prefix).join(&profile);
        let folder = if profile_root.is_dir() {
            projects::published_folder(&reader, &profile, &projects::subfolders(&profile_root)?)?
        } else {
            String::new()
        };
        let in_published = !folder.is_empty()
            && path
                .strip_prefix(&prefix)
                .and_then(|rel| rel.split('/').next())
                == Some(folder.as_str());
        let item_published = published
            || selected.unwrap_or(in_published || receipts.get(&selection) == Some(&stamp));
        normalized.push(json!({"volume_id": id, "path": path, "role": role, "stamp": stamp, "published": item_published}));
        event_finals.push(
            json!({"volume_id": id, "path": path, "role": role, "published": item_published}),
        );
    }
    let all_published = !normalized.is_empty() && normalized.iter().all(|v| v["published"] == true);
    let value = json!({"required": required_roles, "no_more": no_more, "published": all_published,
                       "finals": normalized, "updated_at": now(),
                       "related": previous.get("related").cloned().unwrap_or(json!([]))});
    let tx = catalog.db().unchecked_transaction().map_err(sql)?;
    catalog.set(&key, &value)?;
    if profile_default {
        catalog.set(&format!("closure-profile:{profile}"), &value["required"])?;
    }
    catalog.event(
        project_id,
        "project_delivery_decision",
        &json!({"project_id": project_id, "previous_digest": nu9ve_json::digest(&previous),
                "required": required_roles, "no_more": no_more, "published": all_published,
                "finals": event_finals}),
    )?;
    tx.commit().map_err(sql)?;
    Ok(value)
}

/// The `closure_link` method: the person confirms (or removes) that two projects are one work. Moves no media.
pub fn closure_link(home: &Path, params: &Value) -> Result<Value> {
    let project_id = text(params, "project_id")?;
    let other_id = text(params, "other_id")?;
    let reader = crate::catalog::Catalog::open(home)?;
    let first = crate::reads::project_record(&reader, project_id)?;
    let second = crate::reads::project_record(&reader, other_id)?;
    let linked = params.get("linked").cloned().unwrap_or(json!(true));
    require(
        linked.is_boolean(),
        "invalid_request",
        "linked debe ser booleano",
    )?;
    require(
        first.0 != second.0,
        "invalid_request",
        "Elige otro proyecto",
    )?;
    require(
        first.1 == second.1,
        "invalid_request",
        "Vincula proyectos del mismo perfil",
    )?;
    let catalog = WritableCatalog::open(home)?;
    let _lock = writer_lock(home)?;
    let mut changes: Vec<(String, Value)> = Vec::new();
    for (me, other) in [(&first, &second), (&second, &first)] {
        let mut config = crate::reads::closure_settings(&reader, &me.0)?;
        let mut ids: std::collections::BTreeSet<String> = config["related"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if linked == true {
            ids.insert(other.0.clone());
        } else {
            ids.remove(&other.0);
        }
        config["related"] = json!(ids);
        config["updated_at"] = json!(now());
        changes.push((format!("project-closure:{}", me.0), config));
    }
    // Catalog.set_many: the decisions and their event in one transaction.
    let db = catalog.db();
    db.execute_batch("BEGIN").map_err(sql)?;
    let result = (|| -> Result<()> {
        for (key, value) in &changes {
            catalog.set(key, value)?;
        }
        let kind = if linked == true {
            "project_relation_confirmed"
        } else {
            "project_relation_removed"
        };
        catalog.event(
            project_id,
            kind,
            &json!({"projects": [first.0, second.0], "kind": "rerecording"}),
        )
    })();
    db.execute_batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })
        .map_err(sql)?;
    result?;
    drop(reader);
    let reader = crate::catalog::Catalog::open(home)?;
    Ok(json!({"project_id": project_id, "related": crate::reads::related(&reader, project_id)?}))
}

/// The `save_profile` method: a creative profile, its raw folder, default disks and Clipper profile.
pub fn save_profile(home: &Path, params: &Value) -> Result<Value> {
    only(
        params,
        &[
            "id",
            "name",
            "raw_dir",
            "work",
            "archive",
            "clipper_profile",
        ],
        "save_profile",
    )?;
    let id = text(params, "id")?;
    require(
        slug(id)? == id,
        "invalid_profile",
        "Use a normalized profile ID",
    )?;
    let name = params["name"].as_str().map(str::trim);
    require(
        name.is_some_and(|n| !n.is_empty() && n.chars().count() <= 120),
        "invalid_profile",
        "Choose a profile label",
    )?;
    let raw_dir = relative(text(params, "raw_dir")?)?.to_owned();
    let catalog = WritableCatalog::open(home)?;
    let mut resolved = Map::new();
    for role in ["work", "archive"] {
        let selected = params.get(role).cloned().unwrap_or(json!(""));
        let selected = selected
            .as_str()
            .ok_or_else(|| Error::new("invalid_profile", "Volume selection must be a string"))?;
        let mut id_value = String::new();
        if !selected.is_empty() {
            let record = catalog.volume(selected)?;
            require(
                record["role"] == role,
                "invalid_role",
                "Wrong profile volume role",
            )?;
            id_value = record["id"].as_str().unwrap_or_default().to_owned();
        }
        resolved.insert(role.into(), json!(id_value));
    }
    let _lock = writer_lock(home)?;
    let mut entries = catalog
        .get("creative-profiles")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    // Older clients omit the field; keep an explicitly configured mapping.
    let clipper = match params.get("clipper_profile") {
        Some(value) => value.clone(),
        None => entries
            .get(id)
            .map(|e| e["clipper_profile"].clone())
            .unwrap_or(Value::Null),
    };
    require(
        clipper.is_null()
            || clipper
                .as_str()
                .is_some_and(|c| !c.is_empty() && c.chars().count() <= 120),
        "invalid_profile",
        "Clipper profile must be an opaque nonempty reference or null",
    )?;
    let mut value = json!({"clipper_profile": clipper, "id": id, "name": name.unwrap_or_default(), "raw_dir": raw_dir});
    for (k, v) in resolved {
        value[k.as_str()] = v;
    }
    entries.insert(id.into(), value.clone());
    catalog.set("creative-profiles", &Value::Object(entries))?;
    Ok(value)
}

/// The `confirm_device` method: a confirmed device is classified without asking again.
pub fn confirm_device(home: &Path, params: &Value) -> Result<Value> {
    only(
        params,
        &["make", "model", "serial", "category", "source", "label"],
        "confirm_device",
    )?;
    let field = |k: &str| params[k].as_str().filter(|v| !v.trim().is_empty());
    let (Some(make), Some(model), Some(serial)) = (field("make"), field("model"), field("serial"))
    else {
        return Err(Error::new(
            "invalid_device",
            "Device needs Make, Model and Serial from the file",
        ));
    };
    let category = required(params, "category")?;
    require(
        category
            .as_str()
            .is_some_and(|c| nuun_recipe::CAMERAS.contains(&c)),
        "invalid_camera",
        "Unknown camera category",
    )?;
    let source_text = match params.get("source") {
        Some(Value::String(s)) if !s.is_empty() => s.as_str(),
        _ => model,
    };
    let source = slug(source_text)?.to_uppercase().replace('-', "");
    require(
        !source.is_empty() && source.chars().count() <= 40,
        "invalid_device",
        "Invalid source tag",
    )?;
    let label = params.get("label").cloned().unwrap_or(Value::Null);
    require(
        label.is_null() || label.as_str().is_some_and(|l| l.chars().count() <= 120),
        "invalid_device",
        "Invalid label",
    )?;
    let key = crate::devices::device_key(Some(make), Some(model), Some(serial));
    let entry = json!({"key": key, "make": make.trim(), "model": model.trim(), "serial": serial.trim(),
                       "category": category, "source": source, "label": label,
                       "confirmed_at": now().replace('Z', "+00:00")});
    let catalog = WritableCatalog::open(home)?;
    let _lock = writer_lock(home)?;
    let mut registry = catalog
        .get("device-registry")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    registry.insert(key, entry.clone());
    catalog.set("device-registry", &Value::Object(registry))?;
    Ok(entry)
}
