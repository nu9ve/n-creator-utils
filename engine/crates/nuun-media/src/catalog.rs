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

    /// The read-only connection, for read modules of this crate (snapshot). None for an empty catalog.
    pub(crate) fn db(&self) -> Option<&Connection> {
        self.db.as_ref()
    }

    /// Rows of a single text column as JSON values (`SELECT value FROM metadata …`).
    pub(crate) fn json_rows(
        &self,
        sql_text: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<Value>> {
        let Some(db) = &self.db else {
            return Ok(Vec::new());
        };
        let mut statement = db.prepare(sql_text).map_err(sql)?;
        let rows = statement
            .query_map(params, |row| row.get::<_, String>(0))
            .map_err(sql)?;
        rows.map(|text| parse(&text.map_err(sql)?)).collect()
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
    /// Python `Catalog.volume(name)`: a registered volume by name or id.
    pub fn volume(&self, name: &str) -> Result<Value> {
        self.volumes()?
            .into_iter()
            .find(|v| v["name"] == name || v["id"] == name)
            .ok_or_else(|| {
                Error::new(
                    "unknown_volume",
                    format!("Unknown registered volume: {name}"),
                )
            })
    }

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

/// Python's schema (`media/catalog.py` `Catalog.__init__`), so either engine opens a catalog the other created.
const SCHEMA: &str = "
  CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS volumes(id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL, payload TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, digest TEXT NOT NULL, plan TEXT NOT NULL,
    state TEXT NOT NULL, error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT, job_id TEXT NOT NULL,
    kind TEXT NOT NULL, payload TEXT NOT NULL, created_at TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS members(job_id TEXT NOT NULL, relative_path TEXT NOT NULL,
    sha256 TEXT NOT NULL, cleaned INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(job_id,relative_path));
  CREATE TABLE IF NOT EXISTS copies(job_id TEXT NOT NULL, volume_id TEXT NOT NULL,
    relative_path TEXT NOT NULL, sha256 TEXT NOT NULL, payload TEXT NOT NULL,
    PRIMARY KEY(job_id,volume_id,relative_path));
  CREATE TABLE IF NOT EXISTS sessions(session_id TEXT NOT NULL, manifest_digest TEXT NOT NULL, asset_id TEXT NOT NULL, PRIMARY KEY(session_id,manifest_digest,asset_id));
  CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY, profile TEXT NOT NULL, title TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS project_aliases(profile TEXT NOT NULL, alias TEXT NOT NULL, project_id TEXT NOT NULL, PRIMARY KEY(profile,alias));
  CREATE TABLE IF NOT EXISTS asset_projects(asset_id TEXT NOT NULL, project_id TEXT NOT NULL, PRIMARY KEY(asset_id,project_id));
  CREATE TABLE IF NOT EXISTS asset_jobs(asset_id TEXT NOT NULL, job_id TEXT NOT NULL, PRIMARY KEY(asset_id,job_id));
  CREATE TABLE IF NOT EXISTS publications(asset_id TEXT NOT NULL, variant TEXT NOT NULL, channel TEXT NOT NULL, state TEXT NOT NULL,
    url TEXT, updated_at TEXT NOT NULL, PRIMARY KEY(asset_id,variant,channel));
  CREATE TABLE IF NOT EXISTS assets(id TEXT PRIMARY KEY, job_id TEXT NOT NULL, payload TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS asset_revisions(asset_id TEXT NOT NULL, revision TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(asset_id,revision));
  CREATE TABLE IF NOT EXISTS work(asset_id TEXT PRIMARY KEY, state TEXT NOT NULL,
    target TEXT, updated_at TEXT NOT NULL);
";

/// The catalog opened to write (Python `Catalog(home)`): creates the folder and the schema when missing, with the
/// same durability settings (WAL, synchronous FULL, full fsync where the filesystem has it).
pub struct WritableCatalog {
    db: Connection,
}

fn schema_version_ok(db: &Connection) -> Result<()> {
    let exists: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='metadata'",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if exists > 0 {
        let version: Option<String> = db
            .query_row("SELECT value FROM metadata WHERE key='schema'", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(sql)?;
        require(
            version.is_none() || version.as_deref() == Some("1"),
            "unsupported_catalog",
            "Unsupported catalog version",
        )?;
    }
    Ok(())
}

impl WritableCatalog {
    pub fn open(home: &Path) -> Result<Self> {
        let home = std::path::absolute(home).map_err(io)?;
        for path in home.ancestors() {
            require(
                !path.is_symlink(),
                "unsafe_state",
                format!("State path must not contain symlinks: {}", path.display()),
            )?;
        }
        std::fs::create_dir_all(&home).map_err(io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
        }
        let path = home.join("catalog.sqlite3");
        require(
            !path.is_symlink(),
            "unsafe_state",
            "Catalog must not be a symlink",
        )?;
        let db = Connection::open(&path).map_err(sql)?;
        db.busy_timeout(std::time::Duration::from_secs(15))
            .map_err(sql)?;
        schema_version_ok(&db)?;
        // journal_mode returns a row; the others return nothing.
        let _: String = db
            .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))
            .map_err(sql)?;
        db.execute_batch(
            "PRAGMA synchronous=FULL; PRAGMA fullfsync=1; PRAGMA checkpoint_fullfsync=1;",
        )
        .map_err(sql)?;
        db.execute_batch(SCHEMA).map_err(sql)?;
        schema_version_ok(&db)?;
        db.execute("INSERT OR IGNORE INTO metadata VALUES('schema','1')", [])
            .map_err(sql)?;
        Ok(Self { db })
    }

    pub fn volumes(&self) -> Result<Vec<Value>> {
        let mut statement = self
            .db
            .prepare("SELECT payload FROM volumes ORDER BY name")
            .map_err(sql)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sql)?;
        rows.map(|payload| parse(&payload.map_err(sql)?)).collect()
    }

    /// The connection, for the journal writes of `transfer` (same statements as Python).
    pub(crate) fn db(&self) -> &Connection {
        &self.db
    }

    /// Python `Catalog.get(key)`.
    pub fn get(&self, key: &str) -> Result<Option<Value>> {
        let raw: Option<String> = self
            .db
            .query_row("SELECT value FROM metadata WHERE key=?", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sql)?;
        raw.map(|text| parse(&text)).transpose()
    }

    /// Python `Catalog.volume(name)`: a registered volume by name or id.
    pub fn volume(&self, name: &str) -> Result<Value> {
        self.volumes()?
            .into_iter()
            .find(|v| v["name"] == name || v["id"] == name)
            .ok_or_else(|| {
                Error::new(
                    "unknown_volume",
                    format!("Unknown registered volume: {name}"),
                )
            })
    }

    /// Python `Catalog.event(job_id, kind, data)`.
    pub fn event(&self, job_id: &str, kind: &str, data: &Value) -> Result<()> {
        let payload = String::from_utf8(nu9ve_json::canonical(data)).unwrap_or_default();
        self.db
            .execute(
                "INSERT INTO events(job_id,kind,payload,created_at) VALUES(?,?,?,?)",
                (job_id, kind, payload, nu9ve_time::now_utc()),
            )
            .map_err(sql)?;
        Ok(())
    }

    /// Python `Catalog.state(job_id, state, error)`: the job row and an event of the same name.
    pub fn state(&self, job_id: &str, state: &str, error: Option<&str>) -> Result<()> {
        self.db
            .execute(
                "UPDATE jobs SET state=?,error=?,updated_at=? WHERE id=?",
                (state, error, nu9ve_time::now_utc(), job_id),
            )
            .map_err(sql)?;
        let data = match error {
            Some(e) => serde_json::json!({"error": e}),
            None => serde_json::json!({}),
        };
        self.event(job_id, state, &data)
    }

    /// Python `Catalog.project_for(profile, title)`: the project of an alias, created on first use.
    pub fn project_for(&self, profile: &str, title: &str) -> Result<String> {
        let found: Option<String> = self
            .db
            .query_row(
                "SELECT project_id FROM project_aliases WHERE profile=? AND alias=?",
                (profile, title),
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
        if let Some(id) = found {
            return Ok(id);
        }
        let id = nu9ve_json::uid()?;
        let tx = self.db.unchecked_transaction().map_err(sql)?;
        tx.execute("INSERT INTO projects VALUES(?,?,?)", (&id, profile, title))
            .map_err(sql)?;
        tx.execute(
            "INSERT INTO project_aliases VALUES(?,?,?)",
            (profile, title, &id),
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(id)
    }

    /// Python `Catalog.set(key, value)`: canonical JSON in `metadata`, replacing any earlier value.
    pub fn set(&self, key: &str, value: &Value) -> Result<()> {
        let text = String::from_utf8(nu9ve_json::canonical(value)).unwrap_or_default();
        self.db
            .execute("INSERT OR REPLACE INTO metadata VALUES(?,?)", (key, text))
            .map_err(sql)?;
        Ok(())
    }

    /// Python `Catalog.register`: role, name and prefix validated, root and name unique, payload canonical.
    pub fn register(
        &self,
        name: &str,
        role: &str,
        root: &str,
        identity: &Value,
        prefix: &str,
    ) -> Result<Value> {
        require(
            role == "archive" || role == "work",
            "invalid_role",
            "Volume role must be archive or work",
        )?;
        crate::rules::slug(name)?;
        crate::rules::relative(prefix)?;
        require(
            !self
                .volumes()?
                .iter()
                .any(|v| v["root"] == root || v["name"] == name),
            "volume_exists",
            "Root/name already registered",
        )?;
        let value = serde_json::json!({"id": nu9ve_json::uid()?, "name": name, "role": role, "root": root,
            "identity": identity, "prefix": prefix});
        let payload = String::from_utf8(nu9ve_json::canonical(&value)).unwrap_or_default();
        self.db
            .execute(
                "INSERT INTO volumes VALUES(?,?,?)",
                (value["id"].as_str().unwrap_or_default(), name, payload),
            )
            .map_err(sql)?;
        Ok(value)
    }
}

/// The global mutation lock (Python `Catalog.writer`): `writer.lock` in the state folder, exclusive and
/// non-blocking. On macOS `File::try_lock` is `flock`, the lock Python takes, so the two engines exclude each other.
/// Dropping the file releases it.
pub fn writer_lock(home: &Path) -> Result<std::fs::File> {
    operation_lock(
        home,
        "writer.lock",
        "job_busy",
        "Another media operation is running",
    )
}

pub(crate) fn operation_lock(
    home: &Path,
    name: &str,
    busy_code: &str,
    busy_message: &str,
) -> Result<std::fs::File> {
    let home = std::path::absolute(home).map_err(io)?;
    for path in home.ancestors() {
        require(
            !path.is_symlink(),
            "unsafe_state",
            format!("State path must not contain symlinks: {}", path.display()),
        )?;
    }
    std::fs::create_dir_all(&home).map_err(io)?;
    let path = home.join(name);
    require(
        !path.is_symlink(),
        "unsafe_state",
        format!("{name} must not be a symlink"),
    )?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(&path).map_err(io)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(Error::new(busy_code, busy_message)),
        Err(std::fs::TryLockError::Error(e)) => Err(io(e)),
    }
}

fn io(error: std::io::Error) -> Error {
    Error::new("operation_failed", error.to_string())
}

/// Test support: a catalog with Python's schema, written the way `Catalog.register` writes it.
#[cfg(test)]
pub mod fixture {
    use super::*;

    pub fn create(home: &Path) -> Connection {
        std::fs::create_dir_all(home).unwrap();
        let db = Connection::open(home.join("catalog.sqlite3")).unwrap();
        // Python's whole schema, as a catalog Python created.
        db.execute_batch(SCHEMA).unwrap();
        db.execute("INSERT INTO metadata VALUES('schema','1')", [])
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
