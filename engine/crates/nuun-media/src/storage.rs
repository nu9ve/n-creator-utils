//! Python `storage_index` reads: `storage_overview`, `storage_progress` and `storage_tree` — what occupies each
//! disk and whether it already has a copy on the archive, from the disk index (`storage-index.sqlite3`).
//!
//! The index is opened read only. Python creates an empty index when the file is missing and rebuilds one of
//! another version; here a missing or foreign index reads as an empty one, which answers the same. Scans and
//! comparisons (the writes) stay in Python.

use crate::catalog::{Catalog, WritableCatalog, operation_lock};
use crate::volumes::{self, Backend, Cached};
use nu9ve_json::{Error, Result, require};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;

const SCHEMA: i64 = 1;
const SAFETY: [&str; 5] = ["verified", "probable", "only_here", "derived", "unknown"];
pub const THIS_MAC: &str = "this-mac";
/// storage_index.py `THIS_MAC_ROOT`: the data volume holds everything on the internal disk but the sealed system.
pub const THIS_MAC_ROOT: &str = "/System/Volumes/Data";

fn index_error(error: rusqlite::Error) -> Error {
    Error::new(
        "storage_index_error",
        format!("Disk index unavailable: {error}"),
    )
}

/// The disk index, read only; `None` reads as an empty index.
pub fn open_index(home: &Path) -> Result<Option<Connection>> {
    index_connection(home, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

fn index_connection(home: &Path, access: OpenFlags) -> Result<Option<Connection>> {
    let path = home.join("storage-index.sqlite3");
    require(
        !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()),
        "unsafe_state",
        "Storage index must not be a symlink",
    )?;
    if !path.exists() {
        return Ok(None);
    }
    let db = Connection::open_with_flags(&path, access | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(index_error)?;
    db.busy_timeout(std::time::Duration::from_secs(15))
        .map_err(index_error)?;
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(index_error)?;
    Ok((version == SCHEMA).then_some(db))
}

/// A row as a JSON object, every column by name (Python's `dict(row)`).
fn row_json(row: &Row) -> rusqlite::Result<Value> {
    let mut map = Map::new();
    let statement = row.as_ref();
    for i in 0..statement.column_count() {
        let name = statement.column_name(i)?.to_owned();
        let value = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(x) => json!(x),
            ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
            ValueRef::Blob(b) => json!(String::from_utf8_lossy(b)),
        };
        map.insert(name, value);
    }
    Ok(Value::Object(map))
}

fn one(db: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Option<Value>> {
    db.query_row(sql, params, row_json)
        .optional()
        .map_err(index_error)
}

fn all(db: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<Value>> {
    let mut statement = db.prepare(sql).map_err(index_error)?;
    let rows = statement.query_map(params, row_json).map_err(index_error)?;
    rows.collect::<rusqlite::Result<_>>().map_err(index_error)
}

fn parse(text: &Value) -> Value {
    text.as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Value::Null)
}

/// storage_index.py `targets`: registered disks, folders chosen only to be seen, and this Mac.
pub fn targets(catalog: &Catalog, this_mac: Option<&str>) -> Result<Vec<Value>> {
    let mut found: Vec<Value> = catalog
        .volumes()?
        .into_iter()
        .map(|mut v| {
            v["observed"] = json!(false);
            v
        })
        .collect();
    for mut folder in observed_folders(catalog)? {
        folder["observed"] = json!(true);
        found.push(folder);
    }
    if let Some(root) = this_mac.filter(|r| Path::new(r).is_dir()) {
        found.push(
            json!({"id": THIS_MAC, "name": "Mac", "role": "observed", "root": root, "prefix": "",
                          "identity": {}, "observed": true, "builtin": true}),
        );
    }
    Ok(found)
}

fn target(all_targets: &[Value], params: &Value) -> Result<Value> {
    let wanted = params
        .get("target")
        .ok_or_else(|| Error::new("operation_failed", "'target'"))?;
    all_targets
        .iter()
        .find(|t| &t["id"] == wanted || &t["name"] == wanted)
        .cloned()
        .ok_or_else(|| {
            let shown = wanted
                .as_str()
                .map(str::to_owned)
                .unwrap_or(wanted.to_string());
            Error::new("unknown_volume", format!("Unknown disk or folder: {shown}"))
        })
}

fn storage_lock(home: &Path) -> Result<crate::catalog::OperationLock> {
    operation_lock(
        home,
        "storage.lock",
        "storage_busy",
        "Another disk scan or comparison is running",
    )
}

/// Register an observed folder without adding an ingest destination.
pub fn add_folder(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    const SYSTEM_ROOTS: &[&str] = &[
        "/System", "/Library", "/private", "/usr", "/bin", "/sbin", "/etc", "/var", "/dev",
        "/cores", "/opt",
    ];
    add_observed(home, backend, params, SYSTEM_ROOTS)
}

fn add_observed(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    system_roots: &[&str],
) -> Result<Value> {
    let source = params
        .get("root")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("operation_failed", "'root'"))?;
    let root = crate::planner::absolute_source(source)?;
    require(
        root.parent().is_some() && !system_roots.iter().any(|base| root.starts_with(base)),
        "unsafe_path",
        "Choose a folder of yours or a disk, not the system",
    )?;
    let info = backend.describe(&root, false)?;
    require(
        info["removable"] != true
            || info["mount"]
                .as_str()
                .is_none_or(|mount| Path::new(mount) != root),
        "card_not_indexable",
        "Memory cards are read through Ingreso, not indexed",
    )?;
    let _lock = storage_lock(home)?;
    let reader = Catalog::open(home)?;
    let found = targets(&reader, Some(THIS_MAC_ROOT))?;
    for existing in &found {
        let other = Path::new(existing["root"].as_str().unwrap_or_default());
        require(
            !root.starts_with(other) && !other.starts_with(&root),
            "volume_exists",
            format!(
                "That folder overlaps {}",
                existing["name"].as_str().unwrap_or_default()
            ),
        )?;
    }
    let label = params
        .get("name")
        .filter(|v| crate::volumes::truthy(v))
        .cloned()
        .unwrap_or_else(|| json!(root.file_name().unwrap_or_default().to_string_lossy()));
    require(
        !found.iter().any(|v| v["name"] == label),
        "volume_exists",
        "Name already in use",
    )?;
    let value = json!({"id": nu9ve_json::uid()?, "name": label, "role": "observed", "root": root,
                       "prefix": "", "identity": info});
    let mut observed = observed_folders(&reader)?;
    observed.push(value.clone());
    WritableCatalog::open(home)?.set("storage-observed", &json!(observed))?;
    Ok(value)
}

fn observed_folders(catalog: &Catalog) -> Result<Vec<Value>> {
    Ok(catalog
        .get("storage-observed")?
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default())
}

/// Forget the chosen target and its derived rows. Keep the target registered if index deletion fails.
pub fn remove_folder(home: &Path, params: &Value) -> Result<Value> {
    let _lock = storage_lock(home)?;
    let reader = Catalog::open(home)?;
    let found = target(&targets(&reader, Some(THIS_MAC_ROOT))?, params)?;
    require(
        found["observed"] == true && found["builtin"] != true,
        "invalid_role",
        "Only chosen folders can be removed from the index",
    )?;
    let id = found["id"].as_str().unwrap_or_default();
    let catalog = WritableCatalog::open(home)?;
    if let Some(mut index) = index_connection(home, OpenFlags::SQLITE_OPEN_READ_WRITE)? {
        let tx = index.transaction().map_err(index_error)?;
        for table in ["entries", "folders", "safety", "hashes", "indexes", "runs"] {
            tx.execute(&format!("DELETE FROM {table} WHERE target_id=?"), [id])
                .map_err(index_error)?;
        }
        tx.commit().map_err(index_error)?;
    }
    let mut observed = observed_folders(&reader)?;
    observed.retain(|folder| folder["id"] != id);
    catalog.set("storage-observed", &json!(observed))?;
    Ok(json!({"removed": id}))
}

/// storage_index.py `_online`: a registered disk as `check`; a folder only seen, by the inode of its root.
fn online(backend: &dyn Backend, target: &Value) -> Option<Value> {
    if target["observed"] != true {
        return volumes::online(backend, target);
    }
    let info = backend
        .describe(
            Path::new(target["root"].as_str().unwrap_or_default()),
            false,
        )
        .ok()?;
    (target["builtin"] == true || info["root_inode"] == target["identity"]["root_inode"])
        .then_some(info)
}

#[cfg(unix)]
fn alive(pid: i64) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    let answer = unsafe { libc::kill(pid as libc::pid_t, 0) };
    answer == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn alive(_: i64) -> bool {
    true
}

/// storage_index.py `_run_view`: samples parsed; a running scan whose process is gone was interrupted.
fn run_view(row: Option<Value>) -> Value {
    let Some(mut value) = row else {
        return Value::Null;
    };
    value["samples"] = parse(&value["samples"]);
    if value["state"] == "running" && !alive(value["pid"].as_i64().unwrap_or(0)) {
        value["state"] = json!("interrupted");
    }
    value
}

/// storage_index.py `progress`: the scan or comparison running now, if any.
pub fn progress(db: Option<&Connection>) -> Result<Value> {
    let Some(db) = db else { return Ok(Value::Null) };
    let view = run_view(one(
        db,
        "SELECT * FROM runs WHERE state='running' ORDER BY id DESC LIMIT 1",
        [],
    )?);
    Ok(if view["state"] == "running" {
        view
    } else {
        Value::Null
    })
}

fn safety_view(db: &Connection, target: &Value, path: &str, allocated: &Value) -> Result<Value> {
    if target["role"] == "archive" {
        return Ok(json!({"backup": allocated}));
    }
    let row = one(
        db,
        "SELECT * FROM safety WHERE target_id=? AND path=?",
        [target["id"].as_str().unwrap_or_default(), path],
    )?;
    Ok(match row {
        Some(row) => Value::Object(
            SAFETY
                .iter()
                .map(|k| ((*k).to_owned(), row[*k].clone()))
                .collect(),
        ),
        None => json!({"unknown": allocated}),
    })
}

/// `shutil.disk_usage`: `(total, used, free)`.
fn disk_usage(root: &str) -> Option<(u64, u64, u64)> {
    #[cfg(target_os = "macos")]
    {
        nu9ve_volumes::macos::disk_usage(Path::new(root)).ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = root;
        None
    }
}

/// Python `round(x, 3)`: correctly rounded from the exact binary value, as `{:.3}` formats it.
fn round3(value: f64) -> Value {
    format!("{value:.3}")
        .parse::<f64>()
        .map(|v| json!(v))
        .unwrap_or(Value::Null)
}

/// The `storage_overview` method.
pub fn overview(
    catalog: &Catalog,
    home: &Path,
    backend: &dyn Backend,
    this_mac: Option<&str>,
) -> Result<Value> {
    let db = open_index(home)?;
    let cached = Cached::new(backend);
    volumes::prefetch(&cached, catalog, false)?;
    let mut result = Vec::new();
    for target in targets(catalog, this_mac)? {
        let id = target["id"].as_str().unwrap_or_default().to_owned();
        let info = online(&cached, &target);
        let (index, last) = match &db {
            Some(db) => (
                one(db, "SELECT * FROM indexes WHERE target_id=?", [&id])?,
                run_view(one(
                    db,
                    "SELECT * FROM runs WHERE target_id=? ORDER BY id DESC LIMIT 1",
                    [&id],
                )?),
            ),
            None => (None, Value::Null),
        };
        let mut value = json!({"id": id, "name": target["name"], "role": target["role"], "root": target["root"],
            "observed": target["observed"], "builtin": target["builtin"] == true, "online": info.is_some(),
            "indexed": index.is_some(), "last_run": last});
        let usage = info
            .as_ref()
            .and_then(|_| disk_usage(target["root"].as_str().unwrap_or_default()));
        if let (Some(index), Some(db)) = (&index, &db) {
            let root = one(
                db,
                "SELECT files,bytes,allocated,kinds FROM folders WHERE target_id=? AND run_id=? AND path=?",
                rusqlite::params![&id, index["run_id"].as_i64(), ""],
            )?
            .ok_or_else(|| Error::new("operation_failed", "'NoneType' object is not subscriptable"))?;
            value["indexed_at"] = index["indexed_at"].clone();
            value["seconds"] = round3(index["seconds"].as_f64().unwrap_or(0.0));
            value["unreadable"] = index["unreadable"].clone();
            value["samples"] = parse(&index["samples"]);
            value["files"] = root["files"].clone();
            value["bytes"] = root["bytes"].clone();
            value["allocated"] = root["allocated"].clone();
            value["kinds"] = parse(&root["kinds"]);
            value["safety"] = safety_view(db, &target, "", &root["allocated"])?;
        }
        let from_index = |key: &str| {
            index
                .as_ref()
                .map(|i| i[key].clone())
                .unwrap_or(Value::Null)
        };
        value["fs_total"] = usage
            .map(|u| json!(u.0))
            .unwrap_or_else(|| from_index("fs_total"));
        value["fs_used"] = usage
            .map(|u| json!(u.1))
            .unwrap_or_else(|| from_index("fs_used"));
        value["fs_free"] = usage
            .map(|u| json!(u.2))
            .unwrap_or_else(|| from_index("fs_free"));
        // Used space the walk did not reach: system, snapshots, purgeable, other folders or denied access.
        value["uncounted"] = match (&index, value["fs_used"].as_i64()) {
            (Some(_), Some(used)) => {
                json!((used - value["allocated"].as_i64().unwrap_or(0)).max(0))
            }
            _ => Value::Null,
        };
        result.push(value);
    }
    Ok(json!({"targets": result, "running": progress(db.as_ref())?}))
}

/// The `storage_progress` method.
pub fn progress_method(home: &Path) -> Result<Value> {
    let db = open_index(home)?;
    Ok(json!({"running": progress(db.as_ref())?}))
}

/// storage_index.py `_Evidence`: what the archives are known to hold, to classify the files of one target.
struct Evidence {
    located: HashMap<String, (Value, Value, Value)>,
    sizes: HashMap<i64, (i64, i64)>,
    indexed: bool,
}

impl Evidence {
    fn new(
        catalog: &Catalog,
        db: &Connection,
        target: &Value,
        all_targets: &[Value],
    ) -> Result<Self> {
        let id = target["id"].as_str().unwrap_or_default();
        let mut evidence = Self {
            located: HashMap::new(),
            sizes: HashMap::new(),
            indexed: false,
        };
        for archive in all_targets {
            let archive_id = archive["id"].as_str().unwrap_or_default();
            if archive["role"] != "archive" || archive_id == id {
                continue;
            }
            let independent = one(
                db,
                "SELECT value FROM meta WHERE key=?",
                [format!("independent:{id}:{archive_id}")],
            )?
            .map(|row| parse(&row["value"]))
            .unwrap_or(json!(true));
            if !crate::volumes::truthy(&independent)
                || archive["identity"]["volume_uuid"] == target["identity"]["volume_uuid"]
            {
                continue;
            }
            // storage_index.py `_receipts(catalog, archive)`: the first receipt of each hash wins.
            if let Some(cdb) = catalog.db() {
                let mut statement = cdb
                    .prepare("SELECT sha256,payload FROM copies WHERE volume_id=?")
                    .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
                let rows = statement
                    .query_map([archive_id], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })
                    .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
                for row in rows {
                    let (sha, payload) =
                        row.map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
                    let payload: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
                    if crate::volumes::truthy(&payload["destination"]) {
                        evidence.located.entry(sha).or_insert((
                            archive["name"].clone(),
                            payload["destination"].clone(),
                            json!("receipt"),
                        ));
                    }
                }
            }
            let Some(index) = one(
                db,
                "SELECT run_id,indexed_at FROM indexes WHERE target_id=?",
                [archive_id],
            )?
            else {
                continue;
            };
            evidence.indexed = true;
            for row in all(
                db,
                "SELECT path,sha256,origin FROM hashes WHERE target_id=?",
                [archive_id],
            )? {
                if let Some(sha) = row["sha256"].as_str() {
                    evidence.located.entry(sha.to_owned()).or_insert((
                        archive["name"].clone(),
                        row["path"].clone(),
                        row["origin"].clone(),
                    ));
                }
            }
            for row in all(
                db,
                "SELECT e.size,COUNT(*) AS total,COUNT(h.sha256) AS hashed FROM entries e LEFT JOIN hashes h
                 ON h.target_id=e.target_id AND h.path=e.path AND h.size=e.size AND h.mtime_ns=e.mtime_ns
                 WHERE e.target_id=? AND e.run_id=? AND e.size>0 GROUP BY e.size",
                rusqlite::params![archive_id, index["run_id"].as_i64()],
            )? {
                let size = row["size"].as_i64().unwrap_or(0);
                let slot = evidence.sizes.entry(size).or_insert((0, 0));
                slot.0 += row["total"].as_i64().unwrap_or(0);
                slot.1 += row["hashed"].as_i64().unwrap_or(0);
            }
        }
        Ok(evidence)
    }

    fn state(&self, size: i64, kind: &str, sha: Option<&str>) -> &'static str {
        if sha.is_some_and(|s| self.located.contains_key(s)) {
            return "verified";
        }
        if kind == "derived" {
            return "derived";
        }
        if !self.indexed {
            return "unknown";
        }
        match self.sizes.get(&size) {
            None => "only_here",
            // Same size only means "maybe" until both sides have a hash.
            Some((total, hashed)) if sha.is_some_and(|s| !s.is_empty()) && hashed == total => {
                "only_here"
            }
            Some(_) => "probable",
        }
    }
}

/// storage_index.py `_rel`.
fn relative(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok(String::new()),
        Value::String(s) if s.is_empty() => Ok(String::new()),
        Value::String(s)
            if !s.contains('\0')
                && !s.starts_with('/')
                && s.split('/').all(|p| !matches!(p, "" | "." | "..")) =>
        {
            Ok(s.clone())
        }
        _ => Err(Error::new(
            "unsafe_path",
            "Path must remain within its root",
        )),
    }
}

fn last_part(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The `storage_tree` method: one folder of the index, its largest folders and files, and how safe each is.
pub fn tree(
    catalog: &Catalog,
    home: &Path,
    params: &Value,
    this_mac: Option<&str>,
) -> Result<Value> {
    let all_targets = targets(catalog, this_mac)?;
    let found = target(&all_targets, params)?;
    let path = relative(params.get("path").unwrap_or(&Value::Null))?;
    let limit = match params.get("limit") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => 200,
        Some(Value::Bool(true)) => 1,
        Some(Value::Number(n)) if n.as_i64() == Some(0) => 200,
        Some(Value::Number(n)) => n
            .as_i64()
            .filter(|l| *l > 0 && *l <= 1000 && n.is_i64())
            .ok_or_else(|| Error::new("invalid_request", "Invalid page size"))?,
        Some(Value::String(s)) if s.is_empty() => 200,
        Some(_) => return Err(Error::new("invalid_request", "Invalid page size")),
    };
    let id = found["id"].as_str().unwrap_or_default().to_owned();
    let db = open_index(home)?;
    let index = match &db {
        Some(db) => one(db, "SELECT * FROM indexes WHERE target_id=?", [&id])?,
        None => None,
    };
    let (Some(db), Some(index)) = (db, index) else {
        return Ok(
            json!({"target": id, "indexed": false, "path": path, "folders": [], "files": []}),
        );
    };
    let run = index["run_id"].as_i64();
    let row = one(
        &db,
        "SELECT * FROM folders WHERE target_id=? AND run_id=? AND path=?",
        rusqlite::params![&id, run, &path],
    )?
    .ok_or_else(|| {
        Error::new(
            "unknown_path",
            format!("Folder is not in the index: {path}"),
        )
    })?;
    let opaque = row["opaque"].as_i64().unwrap_or(0) != 0;
    let mut children = Vec::new();
    let mut files = Vec::new();
    if !opaque {
        for item in all(
            &db,
            "SELECT * FROM folders WHERE target_id=? AND run_id=? AND parent=? AND path!='' ORDER BY allocated DESC LIMIT ?",
            rusqlite::params![&id, run, &path, limit],
        )? {
            let item_path = item["path"].as_str().unwrap_or_default().to_owned();
            children.push(json!({"path": item_path, "name": last_part(&item_path), "files": item["files"],
                "bytes": item["bytes"], "allocated": item["allocated"], "kinds": parse(&item["kinds"]),
                "kind": item["kind"], "opaque": item["opaque"].as_i64().unwrap_or(0) != 0,
                "safety": safety_view(&db, &found, &item_path, &item["allocated"])?}));
        }
        let rows = all(
            &db,
            "SELECT e.path,e.size,e.allocated,e.mtime_ns,e.kind,h.sha256,h.origin,h.checked_at FROM entries e LEFT JOIN hashes h ON h.target_id=e.target_id AND h.path=e.path
             AND h.size=e.size AND h.mtime_ns=e.mtime_ns WHERE e.target_id=? AND e.run_id=? AND e.parent=? ORDER BY e.allocated DESC LIMIT ?",
            rusqlite::params![&id, run, &path, limit],
        )?;
        if !rows.is_empty() {
            let evidence = if found["role"] == "archive" {
                None
            } else {
                Some(Evidence::new(catalog, &db, &found, &all_targets)?)
            };
            for item in rows {
                let sha = item["sha256"].as_str();
                let state = match &evidence {
                    None => "backup",
                    Some(e) => e.state(
                        item["size"].as_i64().unwrap_or(0),
                        item["kind"].as_str().unwrap_or_default(),
                        sha,
                    ),
                };
                let item_path = item["path"].as_str().unwrap_or_default().to_owned();
                let mut value = json!({"path": item_path, "name": last_part(&item_path), "bytes": item["size"],
                    "allocated": item["allocated"], "kind": item["kind"], "state": state, "checked_at": item["checked_at"]});
                if state == "verified"
                    && let (Some(e), Some(sha)) = (&evidence, sha)
                {
                    let (volume, path, origin) = &e.located[sha];
                    value["copy"] = json!({"volume": volume, "path": path, "origin": origin});
                }
                files.push(value);
            }
        }
    }
    let listed: i64 = children
        .iter()
        .chain(files.iter())
        .map(|v| v["allocated"].as_i64().unwrap_or(0))
        .sum();
    let name = if path.is_empty() {
        found["name"].clone()
    } else {
        json!(last_part(&path))
    };
    Ok(
        json!({"target": id, "indexed": true, "indexed_at": index["indexed_at"], "role": found["role"],
        "root": found["root"], "path": path, "name": name, "parent": row["parent"], "files_total": row["files"],
        "bytes": row["bytes"], "allocated": row["allocated"], "kinds": parse(&row["kinds"]), "kind": row["kind"],
        "opaque": opaque, "safety": safety_view(&db, &found, &path, &row["allocated"])?, "folders": children,
        "files": files, "rest": (row["allocated"].as_i64().unwrap_or(0) - listed).max(0)}),
    )
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

    /// Phase L: Python's scans of an archive and a work disk, read back by this engine, the same answers.
    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn index_reads_match_python() {
        let (_temp, state, root, case) = golden("storage");
        let catalog = Catalog::open(&state).unwrap();
        let backend = backend_for(&catalog);
        for (key, expected) in case["results"].as_object().unwrap() {
            let request: Value = serde_json::from_str(key).unwrap();
            let params = &request[1];
            let mut mine = answer(
                match request[0].as_str().unwrap() {
                    "storage_overview" => overview(&catalog, &state, &backend, None),
                    "storage_progress" => progress_method(&state),
                    _ => tree(&catalog, &state, params, None),
                },
                &root,
            );
            let targets = mine
                .get_mut("data")
                .and_then(|d| d.get_mut("targets"))
                .and_then(Value::as_array_mut);
            for target in targets.into_iter().flatten() {
                for field in ["fs_total", "fs_used", "fs_free", "uncounted"] {
                    target.as_object_mut().unwrap().remove(field);
                }
            }
            assert_eq!(&mine, expected, "{key}");
        }
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn observed_folder_and_coverage_match_python() {
        let (_temp, state, root, case) = golden("storage");
        let observed_root = Path::new(&root).join("observed");
        let reader = Catalog::open(&state).unwrap();
        let mut backend = backend_for(&reader);
        let expected = &case["writes"];
        let identity: Value = serde_json::from_str(
            &expected["added"]["identity"]
                .to_string()
                .replace("<ROOT>", &root),
        )
        .unwrap();
        backend
            .0
            .insert(observed_root.display().to_string(), identity);
        let params = json!({"root": observed_root, "name": "Observada"});
        let mut added = add_observed(&state, &backend, &params, &[]).unwrap();
        let id = added["id"].as_str().unwrap().to_owned();
        added["id"] = json!("<ID>");
        assert_eq!(answer(Ok(added), &root)["data"], expected["added"]);
        assert_eq!(
            add_observed(&state, &backend, &params, &[])
                .unwrap_err()
                .code,
            "volume_exists"
        );
        assert_eq!(
            remove_folder(&state, &json!({"target": "ESSD"}))
                .unwrap_err()
                .code,
            "invalid_role"
        );
        let lock = storage_lock(&state).unwrap();
        assert_eq!(
            remove_folder(&state, &json!({"target": id}))
                .unwrap_err()
                .code,
            "storage_busy"
        );
        drop(lock);
        let index = Connection::open(state.join("storage-index.sqlite3")).unwrap();
        index.execute(
            "INSERT INTO runs(target_id,kind,state,pid,started_at) VALUES(?,'scan','finished',0,'t')",
            [&id],
        ).unwrap();
        index
            .execute(
                "INSERT INTO hashes VALUES(?,'keep.wav',4,0,'hash','fixture','t')",
                [&id],
            )
            .unwrap();
        let removed = remove_folder(&state, &json!({"target": "Observada"})).unwrap();
        assert_eq!(removed["removed"], id);
        assert_eq!(expected["removed"], json!({"removed": "<ID>"}));
        for table in ["runs", "hashes"] {
            assert!(
                one(
                    &index,
                    &format!("SELECT * FROM {table} WHERE target_id=?"),
                    [&id]
                )
                .unwrap()
                .is_none()
            );
        }
        assert!(
            one(&index, "SELECT * FROM runs LIMIT 1", [])
                .unwrap()
                .is_some()
        );
        assert_eq!(
            std::fs::read(observed_root.join("keep.wav")).unwrap(),
            b"keep"
        );
        assert!(
            observed_folders(&Catalog::open(&state).unwrap())
                .unwrap()
                .is_empty()
        );
        for case in expected["coverage"].as_array().unwrap() {
            let mut reply = crate::coverage::report(&state, &backend,
                &json!({"work": "ESSD", "archive": "MEDIA", "path": case["path"], "no_hash": case["no_hash"]})
            ).unwrap();
            let data = std::fs::read(reply["report"].as_str().unwrap()).unwrap();
            reply.as_object_mut().unwrap().remove("report");
            let mut report: Value = serde_json::from_slice(&data).unwrap();
            report.as_object_mut().unwrap().remove("created_at");
            assert_eq!(reply, case["reply"]);
            assert_eq!(report, case["report"]);
        }
    }

    #[test]
    fn a_missing_index_reads_as_an_empty_one() {
        let temp = tempfile::tempdir().unwrap();
        let catalog = Catalog::empty();
        assert_eq!(
            progress_method(temp.path()).unwrap(),
            json!({"running": null})
        );
        assert_eq!(
            tree(
                &catalog,
                temp.path(),
                &json!({"target": "this-mac"}),
                Some("/")
            )
            .unwrap()["indexed"],
            false
        );
        assert!(!temp.path().join("storage-index.sqlite3").exists());
    }
}
