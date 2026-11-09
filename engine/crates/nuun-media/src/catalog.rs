//! The local catalog, read only. Port of the reading half of `media/catalog.py`.
//!
//! The Rust engine never writes it yet: mutations stay with Python (the oracle) until each one is ported with its
//! journal. A missing catalog is an empty one: reading never creates a file.

use nu9ve_json::{Error, Result, require};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Where the catalog lives without `--state-dir`: `NU9VE_MEDIA_HOME`, else the same folder as Python on macOS,
/// else `%LOCALAPPDATA%\n-creator-utils\media` on Windows (new: Python has no Windows default) and the XDG folder
/// elsewhere.
pub fn default_home() -> PathBuf {
    if let Some(home) = std::env::var_os("NU9VE_MEDIA_HOME") {
        return PathBuf::from(home);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/n-creator-utils/media");
    }
    if cfg!(windows)
        && let Some(local) = std::env::var_os("LOCALAPPDATA")
    {
        return PathBuf::from(local).join("n-creator-utils").join("media");
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("n-creator-utils/media")
}

pub struct Catalog {
    db: Option<Connection>,
}

impl Catalog {
    /// Opens `<home>/catalog.sqlite3` read only, or an empty catalog when it does not exist.
    pub fn open(home: &Path) -> Result<Self> {
        let path = home.join("catalog.sqlite3");
        if !path.exists() {
            return Ok(Self { db: None });
        }
        require(
            !path.is_symlink(),
            "unsafe_state",
            "Catalog must not be a symlink",
        )?;
        let db = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
        db.busy_timeout(std::time::Duration::from_secs(15))
            .map_err(sql)?;
        let version: Option<String> = db
            .query_row("SELECT value FROM metadata WHERE key='schema'", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sql)?;
        require(
            version.as_deref() == Some("1"),
            "unsupported_catalog",
            "Unsupported catalog version",
        )?;
        Ok(Self { db: Some(db) })
    }

    /// An empty catalog (no file).
    pub fn empty() -> Self {
        Self { db: None }
    }

    /// Registered volumes ordered by name (Python `volumes()`).
    pub fn volumes(&self) -> Result<Vec<Value>> {
        let Some(db) = &self.db else {
            return Ok(Vec::new());
        };
        let mut statement = db
            .prepare("SELECT payload FROM volumes ORDER BY name")
            .map_err(sql)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sql)?;
        rows.map(|payload| parse(&payload.map_err(sql)?)).collect()
    }

    /// A metadata value (Python `get(key)`).
    pub fn get(&self, key: &str) -> Result<Option<Value>> {
        let Some(db) = &self.db else {
            return Ok(None);
        };
        let raw: Option<String> = db
            .query_row("SELECT value FROM metadata WHERE key=?", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sql)?;
        raw.map(|text| parse(&text)).transpose()
    }

    /// Last project and profile planned from a card with this volume identity (volumes.py `card_hint`).
    pub fn last_project(&self, volume_uuid: &str) -> Result<Option<(String, String)>> {
        let Some(db) = &self.db else {
            return Ok(None);
        };
        db.query_row(
            "SELECT json_extract(unit.value,'$.project') AS project, json_extract(unit.value,'$.profile') AS profile \
             FROM jobs, json_each(jobs.plan,'$.units') AS unit \
             WHERE json_extract(jobs.plan,'$.source_identity.volume_uuid')=? \
             AND coalesce(json_extract(unit.value,'$.support'),0)=0 ORDER BY jobs.created_at DESC,unit.key LIMIT 1",
            [volume_uuid],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(sql)
        .map(|row| match row {
            Some((Some(project), profile)) if !project.is_empty() => {
                Some((project, profile.unwrap_or_default()))
            }
            _ => None,
        })
    }
}

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn parse(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|e| Error::new("catalog_corrupt", e.to_string()))
}

/// Test support: a catalog with Python's schema, written the way `Catalog.register` writes it.
#[cfg(test)]
pub mod fixture {
    use super::*;

    pub fn create(home: &Path) -> Connection {
        std::fs::create_dir_all(home).unwrap();
        let db = Connection::open(home.join("catalog.sqlite3")).unwrap();
        db.execute_batch(
            "CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE volumes(id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL, payload TEXT NOT NULL);
             CREATE TABLE jobs(id TEXT PRIMARY KEY, digest TEXT NOT NULL, plan TEXT NOT NULL,
               state TEXT NOT NULL, error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
             INSERT INTO metadata VALUES('schema','1');",
        )
        .unwrap();
        db
    }

    pub fn register(
        db: &Connection,
        name: &str,
        role: &str,
        root: &Path,
        identity: &Value,
        prefix: &str,
    ) -> Value {
        let value = serde_json::json!({"id": format!("id-{name}"), "name": name, "role": role,
            "root": root.display().to_string(), "identity": identity, "prefix": prefix});
        let payload = String::from_utf8(nu9ve_json::canonical(&value)).unwrap();
        db.execute(
            "INSERT INTO volumes VALUES(?,?,?)",
            (value["id"].as_str().unwrap(), name, payload),
        )
        .unwrap();
        value
    }
}
