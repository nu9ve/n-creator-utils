//! Single reads that `snapshot` already gathers, exposed as their own methods, as `media/service.py` does:
//! `jobs`, `job`, `library` (with its filters), `projects`, `profiles`, `devices` and `release_plans`.

use crate::catalog::Catalog;
use crate::snapshot;
use nu9ve_json::{Error, Result, require};
use rusqlite::OptionalExtension;
use rusqlite::types::ValueRef;
use serde_json::{Map, Value, json};

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

/// Python `str.casefold` for the cases a project title meets: lowercase plus the German sharp s. Other full case
/// foldings (ligatures, Greek final sigma) compare as plain lowercase.
fn casefold(text: &str) -> String {
    text.to_lowercase().replace('ß', "ss").replace('ς', "σ")
}

/// Python `Catalog.search(project, work_state)`: `project` matches any linked project title (casefolded substring).
pub fn library(catalog: &Catalog, params: &Value) -> Result<Vec<Value>> {
    let project = params["project"]
        .as_str()
        .filter(|p| !p.is_empty())
        .map(casefold);
    let state = params["work_state"].as_str().filter(|s| !s.is_empty());
    Ok(snapshot::library(catalog)?
        .into_iter()
        .filter(|asset| {
            project.as_ref().is_none_or(|wanted| {
                asset["projects"].as_array().into_iter().flatten().any(|p| {
                    casefold(p["title"].as_str().unwrap_or_default()).contains(wanted.as_str())
                })
            }) && state.is_none_or(|s| asset["work_delivery_status"] == s)
        })
        .collect())
}

/// Python `Catalog.jobs()`. A failed Sony job whose card read failed needs `transfer.sony_removal_recorded`, which
/// reads the plan's destinations: not ported, so the whole answer is `not_ported` rather than incomplete.
pub fn jobs(catalog: &Catalog) -> Result<Vec<Value>> {
    let (values, unported) = snapshot::jobs(catalog)?;
    require(
        !unported,
        "not_ported",
        "sony_removal_recorded for a Sony job that failed reading the card is still in media/transfer.py",
    )?;
    Ok(values)
}

fn cell(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
        // Python would return bytes, which its JSON envelope cannot carry either.
        ValueRef::Blob(_) => Value::Null,
    }
}

fn count(db: &rusqlite::Connection, query: &str, id: &str) -> Result<i64> {
    db.query_row(query, [id], |r| r.get(0)).map_err(sql)
}

/// Python `Catalog.job(job_id)`: the whole row, the parsed plan and progress from the catalog.
pub fn job(catalog: &Catalog, params: &Value) -> Result<Value> {
    let id = params["job_id"]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", "'job_id'"))?;
    let db = catalog
        .db()
        .ok_or_else(|| Error::new("unknown_job", "Job not found"))?;
    let mut statement = db.prepare("SELECT * FROM jobs WHERE id=?").map_err(sql)?;
    let names: Vec<String> = statement
        .column_names()
        .iter()
        .map(|n| (*n).to_owned())
        .collect();
    let row: Option<Map<String, Value>> = statement
        .query_row([id], |r| {
            let mut row = Map::new();
            for (i, name) in names.iter().enumerate() {
                row.insert(name.clone(), cell(r.get_ref(i)?));
            }
            Ok(row)
        })
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(sql)?;
    let mut result = row.ok_or_else(|| Error::new("unknown_job", "Job not found"))?;
    let plan: Value = serde_json::from_str(result["plan"].as_str().unwrap_or_default())
        .map_err(|e| Error::new("catalog_corrupt", e.to_string()))?;
    let mut progress = Map::new();
    progress.insert(
        "verified_copies".into(),
        json!(count(db, "SELECT COUNT(*) FROM copies WHERE job_id=?", id)?),
    );
    progress.insert(
        "cleaned_files".into(),
        json!(count(
            db,
            "SELECT COUNT(*) FROM members WHERE job_id=? AND cleaned=1",
            id
        )?),
    );
    progress.insert(
        "total_files".into(),
        json!(plan["files"].as_array().map_or(0, Vec::len)),
    );
    // Live bytes/phase from the running transfer (Transfer.report); absent for older jobs.
    if let Some(Value::Object(live)) = catalog.get(&format!("progress:{id}"))? {
        progress.extend(live);
    }
    result.insert("progress".into(), Value::Object(progress));
    if let Some(summary) = catalog.get(&format!("stalker-release:{id}"))? {
        result.insert("stalker_release".into(), summary);
    }
    if plan["cleanup_profile"] == "sony_clips_then_format" {
        let check = catalog.get(&format!("sony-card-check:{id}"))?;
        let state = result["state"].as_str().unwrap_or_default().to_owned();
        result.insert(
            "sony_stage".into(),
            json!(snapshot::sony_stage(
                &state,
                check.as_ref().and_then(Value::as_str),
                false
            )),
        );
        result.insert("sony_card_check".into(), check.unwrap_or(Value::Null));
        result.insert(
            "sony_reappeared".into(),
            catalog
                .get(&format!("sony-card-reappeared:{id}"))?
                .unwrap_or_else(|| json!([])),
        );
    }
    result.insert("plan".into(), plan);
    Ok(Value::Object(result))
}

/// Python `library.projects(catalog)`.
pub fn projects(catalog: &Catalog) -> Result<Vec<Value>> {
    snapshot::projects(catalog, &snapshot::library(catalog)?)
}

/// Python `recipes.devices(catalog)`: the recipe reference, its device rules and the confirmed registry by key.
pub fn devices(catalog: &Catalog) -> Result<Value> {
    let recipe = nuun_recipe::effective(catalog.get("recipe")?.as_ref());
    let mut registry: Vec<Value> = catalog
        .get("device-registry")?
        .and_then(|r| r.as_object().cloned())
        .map(|r| r.into_iter().map(|(_, v)| v).collect())
        .unwrap_or_default();
    registry.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
    Ok(
        json!({"recipe": nuun_recipe::recipe_ref(&recipe), "rules": recipe["devices"], "registry": registry}),
    )
}

/// Python `release.release_plans(catalog)`.
pub fn release_plans(catalog: &Catalog) -> Result<Vec<Value>> {
    catalog.json_rows("SELECT value FROM metadata WHERE key LIKE 'release:%'", [])
}

/// A required string parameter, failing as Python's `params['name']` does (`KeyError` → `operation_failed`).
fn param<'a>(params: &'a Value, name: &str) -> Result<&'a str> {
    match params.get(name) {
        Some(Value::String(text)) => Ok(text),
        Some(other) => Err(Error::new(
            "operation_failed",
            format!(
                "can only concatenate str (not \"{}\") to str",
                python_type(other)
            ),
        )),
        None => Err(Error::new("operation_failed", format!("'{name}'"))),
    }
}

fn python_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// Python `project_preview`: the name a project title becomes (`slug`). Python normalizes to NFC first; titles
/// typed in the app already are, a decomposed one would differ here.
pub fn project_preview(params: &Value) -> Result<Value> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| Error::new("invalid_tag", "Invalid project/profile tag"))?;
    Ok(json!({"name": crate::rules::slug(name)?}))
}

/// Python `closure_progress` / `action_progress`: a progress record by id, `{}` when there is none.
pub fn progress(catalog: &Catalog, prefix: &str, params: &Value, name: &str) -> Result<Value> {
    let id = param(params, name)?;
    Ok(catalog
        .get(&format!("{prefix}{id}"))?
        .unwrap_or_else(|| json!({})))
}

/// Python `asset_revisions`: every saved revision of an asset, or the current record when it has none.
pub fn asset_revisions(catalog: &Catalog, params: &Value) -> Result<Value> {
    let id = param(params, "asset_id")?;
    let current = catalog
        .json_rows("SELECT payload FROM assets WHERE id=?", [id])?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("unknown_asset", "Asset not found"))?;
    let key = current["id"].as_str().unwrap_or(id).to_owned();
    let history = catalog.json_rows(
        "SELECT payload FROM asset_revisions WHERE asset_id=?",
        [&key],
    )?;
    Ok(if history.is_empty() {
        json!([current])
    } else {
        json!(history)
    })
}

/// Python `living.collections`: each registered living folder with its indexed files and bytes, by name.
pub fn living_collections(catalog: &Catalog) -> Result<Value> {
    let Some(db) = catalog.db() else {
        return Ok(json!([]));
    };
    let mut result = Vec::new();
    for mut value in catalog.json_rows(
        "SELECT value FROM metadata WHERE key LIKE ?",
        ["living-collection:%"],
    )? {
        let id = value["id"].as_str().unwrap_or_default().to_owned();
        // Python creates `living_files` when it opens the catalog; one it never opened has no files yet.
        let (count, size): (i64, i64) = db
            .query_row(
                "SELECT count(*), coalesce(sum(size),0) FROM living_files WHERE collection_id=?",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .or_else(|e| match e {
                rusqlite::Error::SqliteFailure(_, Some(m)) if m.contains("no such table") => {
                    Ok((0, 0))
                }
                other => Err(other),
            })
            .map_err(sql)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("files".into(), json!(count));
            object.insert("bytes".into(), json!(size));
        }
        result.push(value);
    }
    result.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!(result))
}

/// Python `missions.status(catalog, flow_id=None)`: the archive's mission registry for a flow of the recipe.
pub fn missions_status(catalog: &Catalog, params: &Value) -> Result<Value> {
    if let Some(unexpected) = params
        .as_object()
        .and_then(|p| p.keys().find(|k| k.as_str() != "flow_id"))
    {
        return Err(Error::new(
            "operation_failed",
            format!("status() got an unexpected keyword argument '{unexpected}'"),
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
                let shown = wanted
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or(wanted.to_string());
                Error::new(
                    "no_mission_flow",
                    format!("Unknown mission registry: {shown}"),
                )
            })?,
    };
    let id = flow["id"].as_str().unwrap_or_default();
    let saved = catalog.get(&format!("mission-registry:{id}"))?;
    // Python: `get(...) or {...}`, so an empty registry also answers the placeholder.
    Ok(match saved {
        Some(value) if !is_falsy(&value) => value,
        _ => json!({"flow": id, "coverage": null}),
    })
}

fn is_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

/// Python `closure.project_record`: (id, profile, title) of a project that is not the `finales` collection.
fn project_record(catalog: &Catalog, project_id: &str) -> Result<(String, String, String)> {
    let row = match catalog.db() {
        Some(db) => db
            .query_row(
                "SELECT id,profile,title FROM projects WHERE id=?",
                [project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(sql)?,
        None => None,
    };
    let row: (String, String, String) =
        row.ok_or_else(|| Error::new("unknown_project", "Proyecto desconocido"))?;
    require(
        row.2 != "finales",
        "invalid_project",
        "Elige el proyecto original, no la colección finales",
    )?;
    Ok(row)
}

/// Python `closure.normalized_name`: casefolded, runs of non-word characters and `_` become `-`, trimmed.
fn normalized_name(value: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    // `[^\w]+|_`: a run of non-word characters is one `-`, and each `_` (a word character) is its own `-`.
    for c in casefold(value).chars() {
        if c.is_alphanumeric() || c == '_' {
            if gap {
                out.push('-');
                gap = false;
            }
            out.push(if c == '_' { '-' } else { c });
        } else {
            gap = true;
        }
    }
    if gap {
        out.push('-');
    }
    out.trim_matches('-').to_owned()
}

/// Python `closure.link_candidates`: same-profile projects to link, linked first, then suggested, then by title.
pub fn closure_link_candidates(catalog: &Catalog, params: &Value) -> Result<Value> {
    let project_id = param(params, "project_id")?;
    let (_, profile, title) = project_record(catalog, project_id)?;
    let Some(db) = catalog.db() else {
        return Ok(json!([]));
    };
    // closure.settings → related: the confirmed links that still name a known project.
    let default_required = if profile == "9logs" {
        json!(["horizontal"])
    } else {
        json!(["horizontal", "vertical", "thumbnail"])
    };
    let required = catalog
        .get(&format!("closure-profile:{profile}"))?
        .unwrap_or(default_required);
    let settings = catalog
        .get(&format!("project-closure:{project_id}"))?
        .unwrap_or_else(
            || json!({"required": required, "no_more": false, "published": false, "finals": []}),
        );
    let mut known = std::collections::BTreeSet::new();
    {
        let mut statement = db.prepare("SELECT id FROM projects").map_err(sql)?;
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(sql)?;
        for id in ids {
            known.insert(id.map_err(sql)?);
        }
    }
    let linked: std::collections::BTreeSet<String> = settings["related"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|p| known.contains(*p) && *p != project_id)
        .map(str::to_owned)
        .collect();
    let mine = normalized_name(&title);
    let mut statement = db
        .prepare("SELECT id,title FROM projects WHERE profile=? AND id!=? AND title!='finales'")
        .map_err(sql)?;
    let rows = statement
        .query_map([&profile, project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(sql)?;
    let mut values = Vec::new();
    for row in rows {
        let (id, other_title) = row.map_err(sql)?;
        let other = normalized_name(&other_title);
        let suggested =
            other.starts_with(&format!("{mine}-")) || mine.starts_with(&format!("{other}-"));
        values.push((linked.contains(&id), suggested, other_title, id));
    }
    // Python's key (not linked, not suggested, title); `sort` is stable, so equal keys keep the row order.
    values.sort_by(|a, b| (!a.0, !a.1, &a.2).cmp(&(!b.0, !b.1, &b.2)));
    Ok(json!(
        values
            .into_iter()
            .map(|(linked, suggested, title, id)| json!({"id": id, "title": title, "linked": linked, "suggested": suggested}))
            .collect::<Vec<_>>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::WritableCatalog;

    fn catalog_with_jobs() -> (tempfile::TempDir, Catalog) {
        let home = tempfile::tempdir().unwrap();
        drop(WritableCatalog::open(home.path()).unwrap()); // Python's whole schema
        {
            let db = rusqlite::Connection::open(home.path().join("catalog.sqlite3")).unwrap();
            let plan = json!({"files": [{"path": "a"}, {"path": "b"}], "cleanup_profile": "sony_clips_then_format"});
            db.execute(
                "INSERT INTO jobs(id,digest,plan,state,created_at,updated_at) VALUES('J1','d',?,'backup_verified','t1','t2')",
                [plan.to_string()],
            )
            .unwrap();
            db.execute(
                "INSERT INTO metadata(key,value) VALUES('progress:J1','{\"phase\":\"verify\"}')",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO metadata(key,value) VALUES('device-registry','{\"B|X|1\":{\"key\":\"B|X|1\"},\"A|X|1\":{\"key\":\"A|X|1\"}}')",
                [],
            )
            .unwrap();
        }
        let catalog = Catalog::open(home.path()).unwrap();
        (home, catalog)
    }

    #[test]
    fn a_job_has_its_plan_progress_and_sony_stage() {
        let (_home, catalog) = catalog_with_jobs();
        let job = job(&catalog, &json!({"job_id": "J1"})).unwrap();
        assert_eq!(job["digest"], "d");
        assert_eq!(job["plan"]["files"].as_array().unwrap().len(), 2);
        assert_eq!(
            job["progress"],
            json!({"verified_copies": 0, "cleaned_files": 0, "total_files": 2, "phase": "verify"})
        );
        assert_eq!(job["sony_stage"], "backup_verified");
        assert_eq!(job["sony_reappeared"], json!([]));
        assert_eq!(
            super::job(&catalog, &json!({"job_id": "nope"}))
                .unwrap_err()
                .code,
            "unknown_job"
        );
        assert_eq!(jobs(&catalog).unwrap()[0]["id"], "J1");
    }

    #[test]
    fn devices_list_the_registry_by_key_and_an_empty_catalog_answers() {
        let (_home, catalog) = catalog_with_jobs();
        let found = devices(&catalog).unwrap();
        assert_eq!(found["registry"][0]["key"], "A|X|1");
        assert_eq!(found["rules"][0]["model"], "FC3170");
        let empty = tempfile::tempdir().unwrap();
        let none = Catalog::open(empty.path()).unwrap();
        assert_eq!(devices(&none).unwrap()["registry"], json!([]));
        assert!(library(&none, &json!({"project": "x"})).unwrap().is_empty());
        assert!(release_plans(&none).unwrap().is_empty());
        assert_eq!(
            super::job(&none, &json!({"job_id": "J1"}))
                .unwrap_err()
                .code,
            "unknown_job"
        );
    }

    #[test]
    fn casefold_matches_python_for_titles() {
        assert_eq!(casefold("Straße ÁRBOL"), "strasse árbol");
    }
}
