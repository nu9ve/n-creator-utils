//! nu9ve-data-v1 §2 (F7a, 2026-11-27): the readable suite key of each nüün project, chosen by the person.
//!
//! - A project's key is `project-key:<id>` in the catalog's metadata when the person chose one; until then it is
//!   the local id lowercased, as before, so nothing another app already saw changes on its own.
//! - The person chooses it once (`project_key_set`); nüün only **offers** one, the title's slug, or the known key
//!   `conversaciones` (§2.1) for the project of that name. It never derives a key to pair with another app.
//! - A key is never reused for another project of nüün: every key a project had stays in `project-key-used:<key>`
//!   with its project. Changing a project's key is allowed, and is a new project for the other apps.
use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use nu9ve_json::{Error, Result, require};
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

const KEY: &str = "project-key:";
const USED: &str = "project-key-used:";
/// §2.1: keys with a meaning fixed by the suite, offered when the title says so.
const KNOWN: [(&str, &[&str]); 1] = [(
    "conversaciones",
    &["conversaciones", "mis conversaciones", "my conversations"],
)];

/// §2: `^[a-z0-9-]{1,64}$`.
pub fn valid(key: &str) -> bool {
    (1..=64).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn chosen(db: &Connection, id: &str) -> Option<String> {
    let raw: String = db
        .query_row(
            "SELECT value FROM metadata WHERE key=?",
            [format!("{KEY}{id}")],
            |row| row.get(0),
        )
        .ok()?;
    serde_json::from_str::<String>(&raw)
        .ok()
        .filter(|key| valid(key))
}

/// The suite key of project `id` on this catalog: the chosen one, or the local id lowercased.
pub fn key_of(db: &Connection, id: &str) -> String {
    chosen(db, id).unwrap_or_else(|| id.to_ascii_lowercase())
}

/// What nüün offers for a title: a known key (§2.1), or the title's slug made to fit the pattern.
fn offer(title: &str) -> String {
    let folded = title.trim().to_lowercase();
    if let Some((key, _)) = KNOWN
        .iter()
        .find(|(_, titles)| titles.contains(&folded.as_str()))
    {
        return (*key).to_owned();
    }
    let mut out = String::new();
    for c in folded.chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(64).collect();
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "proyecto".to_owned()
    } else {
        out
    }
}

/// Every key taken in this catalog, by the project that has or had it.
fn taken(db: &Connection) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut ids = db.prepare("SELECT id FROM projects").map_err(sql)?;
    for id in ids
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql)?
    {
        let id = id.map_err(sql)?;
        out.insert(key_of(db, &id), id.clone());
        out.entry(id.to_ascii_lowercase()).or_insert(id);
    }
    let mut used = db
        .prepare("SELECT key, value FROM metadata WHERE key LIKE 'project-key-used:%'")
        .map_err(sql)?;
    for row in used
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql)?
    {
        let (key, value) = row.map_err(sql)?;
        if let Ok(id) = serde_json::from_str::<String>(&value) {
            out.entry(key[USED.len()..].to_owned()).or_insert(id);
        }
    }
    Ok(out)
}

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_error", error.to_string())
}

/// `project_keys`: each project's key, whether the person chose it, and what nüün offers until then.
pub fn keys(catalog: &Catalog) -> Result<Value> {
    let Some(db) = catalog.db() else {
        return Ok(json!({"projects": {}}));
    };
    let taken = taken(db)?;
    let mut offered = std::collections::BTreeSet::new();
    let mut out = Map::new();
    let mut rows = db
        .prepare("SELECT id, title FROM projects ORDER BY id")
        .map_err(sql)?;
    for row in rows
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql)?
    {
        let (id, title) = row.map_err(sql)?;
        let chosen = chosen(db, &id);
        let base = offer(&title);
        let mut suggested = base.clone();
        let mut n = 2;
        // Free for this project: not taken by another one, nor offered to another one in this list.
        while taken.get(&suggested).is_some_and(|owner| owner != &id)
            || offered.contains(&suggested)
        {
            let suffix = format!("-{n}");
            suggested = format!("{}{suffix}", &base[..base.len().min(64 - suffix.len())]);
            n += 1;
        }
        offered.insert(suggested.clone());
        out.insert(
            id.clone(),
            json!({"key": key_of(db, &id), "chosen": chosen.is_some(), "suggested": suggested}),
        );
    }
    Ok(json!({"projects": out}))
}

/// `project_key_set {project_id, key}`: the key the person chose for a project. Refused when it does not follow
/// §2 or another project of nüün has or had it.
pub fn set(home: &Path, params: &Value) -> Result<Value> {
    let fields = params.as_object().filter(|p| p.len() == 2).ok_or_else(|| {
        Error::new(
            "invalid_params",
            "project_key_set takes project_id and key.",
        )
    })?;
    let (Some(id), Some(key)) = (
        fields.get("project_id").and_then(Value::as_str),
        fields.get("key").and_then(Value::as_str),
    ) else {
        return Err(Error::new(
            "invalid_params",
            "project_key_set takes project_id and key.",
        ));
    };
    require(
        valid(key),
        "invalid_project_key",
        "A suite key is 1 to 64 lower-case letters, digits or hyphens.",
    )?;
    let writer = WritableCatalog::open(home)?;
    let _lock = writer_lock(home)?;
    let db = writer.db();
    let exists: bool = db
        .query_row("SELECT 1 FROM projects WHERE id=?", [id], |_| Ok(true))
        .unwrap_or(false);
    require(exists, "unknown_project", "Project not found")?;
    if let Some(owner) = taken(db)?.get(key).filter(|owner| owner.as_str() != id) {
        return Err(Error::new(
            "project_key_taken",
            format!(
                "Another project of nüün has or had the key {key} ({owner}); a key is never reused."
            ),
        ));
    }
    // The key it had stays reserved for it, and so does the new one.
    let previous = key_of(db, id);
    writer.set(&format!("{USED}{previous}"), &json!(id))?;
    writer.set(&format!("{USED}{key}"), &json!(id))?;
    writer.set(&format!("{KEY}{id}"), &json!(key))?;
    Ok(json!({"project_id": id, "key": key, "previous": previous}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> (tempfile::TempDir, WritableCatalog) {
        let dir = tempfile::tempdir().unwrap();
        let catalog = WritableCatalog::open(dir.path()).unwrap();
        for (id, title) in [
            ("8F0C2A1E-0000-4000-8000-000000000001", "Baches de Ñuñoa"),
            ("p2", "Mis conversaciones"),
            ("p3", "baches de ñuñoa"),
        ] {
            catalog
                .db()
                .execute("INSERT INTO projects VALUES(?,?,?)", [id, "canal-a", title])
                .unwrap();
        }
        (dir, catalog)
    }

    #[test]
    fn keys_are_the_old_id_until_the_person_chooses_one() {
        let (dir, _writer) = home();
        let catalog = Catalog::open(dir.path()).unwrap();
        let keys = keys(&catalog).unwrap();
        let first = &keys["projects"]["8F0C2A1E-0000-4000-8000-000000000001"];
        assert_eq!(first["key"], "8f0c2a1e-0000-4000-8000-000000000001");
        assert_eq!(first["chosen"], false);
        assert_eq!(first["suggested"], "baches-de-nunoa");
        // The same title elsewhere gets another offer, and the known key is offered for its project.
        assert_eq!(keys["projects"]["p3"]["suggested"], "baches-de-nunoa-2");
        assert_eq!(keys["projects"]["p2"]["suggested"], "conversaciones");
    }

    #[test]
    fn a_chosen_key_is_kept_and_never_given_to_another_project() {
        let (dir, _writer) = home();
        let id = "8F0C2A1E-0000-4000-8000-000000000001";
        let set_key =
            |project: &str, key: &str| set(dir.path(), &json!({"project_id": project, "key": key}));
        assert_eq!(
            set_key(id, "baches").unwrap()["previous"],
            id.to_ascii_lowercase()
        );
        let catalog = Catalog::open(dir.path()).unwrap();
        assert_eq!(key_of(catalog.db().unwrap(), id), "baches");
        assert_eq!(keys(&catalog).unwrap()["projects"][id]["chosen"], true);
        assert_eq!(
            set_key("p3", "baches").unwrap_err().code,
            "project_key_taken"
        );
        // Changing it frees nothing: the old key stays this project's.
        set_key(id, "baches-norte").unwrap();
        assert_eq!(
            set_key("p3", "baches").unwrap_err().code,
            "project_key_taken"
        );
        assert_eq!(
            set_key("p3", &id.to_ascii_lowercase()).unwrap_err().code,
            "project_key_taken"
        );
        // And it can go back to a key it had.
        set_key(id, "baches").unwrap();
        // The suite sees the chosen key; the others, their id until then.
        let listed = crate::suite_volume::data_catalog(dir.path(), &json!({})).unwrap();
        let listed: Vec<&str> = listed["projects"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p["project_key"].as_str())
            .collect();
        assert!(listed.contains(&"baches"), "{listed:?}");
        assert!(listed.contains(&"p3"), "{listed:?}");
        assert!(
            !listed.contains(&"8f0c2a1e-0000-4000-8000-000000000001"),
            "{listed:?}"
        );
        for (params, code) in [
            (
                json!({"project_id": "p3", "key": "Baches"}),
                "invalid_project_key",
            ),
            (
                json!({"project_id": "p3", "key": ""}),
                "invalid_project_key",
            ),
            (
                json!({"project_id": "p3", "key": "a".repeat(65)}),
                "invalid_project_key",
            ),
            (
                json!({"project_id": "nope", "key": "otro"}),
                "unknown_project",
            ),
            (json!({"project_id": "p3"}), "invalid_params"),
            (
                json!({"project_id": "p3", "key": "x", "more": 1}),
                "invalid_params",
            ),
        ] {
            assert_eq!(set(dir.path(), &params).unwrap_err().code, code, "{params}");
        }
    }

    #[test]
    fn offers_fit_the_pattern() {
        for title in [
            "",
            "  ",
            "¡Hola, Mundo!",
            "Ñandú 2026 · Lima",
            &"x".repeat(200),
        ] {
            let key = offer(title);
            assert!(valid(&key), "{title:?} → {key}");
        }
        assert_eq!(offer("Ñandú 2026 · Lima"), "nandu-2026-lima");
    }
}
