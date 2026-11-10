//! Single reads that `snapshot` already gathers, exposed as their own methods, as `media/service.py` does:
//! `jobs`, `job`, `library` (with its filters), `projects`, `profiles`, `devices` and `release_plans`.

use crate::catalog::Catalog;
use crate::snapshot;
use nu9ve_json::{Error, Result, require};
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
