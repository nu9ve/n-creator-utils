//! The registry folder and its records (contracts/nu9ve-registry-v1.md §1 to §6).

use serde_json::{Map, Value, json};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::process::{current_uid, process};

pub const CONTRACT: &str = "nu9ve-registry/1";
pub const ENV_DIR: &str = "NU9VE_ENGINES_DIR";
pub const MAX_RECORD_BYTES: usize = 16 * 1024;
const MAX_SAFE: u64 = (1 << 53) - 1;

/// §1: the folder, or `None` when there is none. A relative override disables the registry.
pub fn default_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(ENV_DIR) {
        let dir = PathBuf::from(dir);
        return dir.is_absolute().then_some(dir);
    }
    #[cfg(target_os = "macos")]
    {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        home.is_absolute()
            .then(|| home.join("Library/Application Support/nu9ve/engines"))
    }
    #[cfg(windows)]
    {
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
        local
            .is_absolute()
            .then(|| local.join("nu9ve").join("engines"))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        None
    }
}

fn valid_app(app: &str) -> bool {
    let bytes = app.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn valid_capability(capability: &str) -> bool {
    let Some((head, version)) = capability.rsplit_once("_v") else {
        return false;
    };
    !head.is_empty()
        && head.as_bytes()[0].is_ascii_lowercase()
        && head
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && !version.is_empty()
        && version.bytes().all(|b| b.is_ascii_digit())
}

fn valid_utc(text: &str) -> bool {
    let b = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| range.into_iter().all(|i| b[i].is_ascii_digit());
    if b.len() < 20 || !b.ends_with(b"Z") {
        return false;
    }
    let fixed = digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && b[10] == b'T'
        && digits(11..13)
        && b[13] == b':'
        && digits(14..16)
        && b[16] == b':'
        && digits(17..19);
    let fraction = &b[19..b.len() - 1];
    fixed
        && (fraction.is_empty()
            || (fraction[0] == b'.'
                && (2..=10).contains(&fraction.len())
                && fraction[1..].iter().all(u8::is_ascii_digit)))
}

/// §5: only `<app>.json` with a valid app name is a registry file.
pub fn valid_file_name(name: &str) -> bool {
    name.strip_suffix(".json").is_some_and(valid_app)
}

/// How to reach the engine (§3).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Transport {
    Unix { path: PathBuf },
    Pipe { name: String },
}

/// The engine fields copied from its `engine_info`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineRef {
    pub contract: String,
    pub protocol: String,
    pub engine: String,
    pub version: String,
}

/// One engine's record (§2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub app: String,
    pub app_version: String,
    pub pid: u32,
    pub process_start_us: u64,
    pub transport: Transport,
    pub engine: EngineRef,
    pub capabilities: Vec<String>,
    pub written_at: String,
    pub library: Option<String>,
}

impl Record {
    /// A record for this process, with its pid and start time read from the system.
    pub fn for_current_process(
        app: &str,
        app_version: &str,
        transport: Transport,
        engine: EngineRef,
        capabilities: Vec<String>,
    ) -> io::Result<Self> {
        if !valid_app(app) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "app name must match ^[a-z][a-z0-9-]{0,31}$",
            ));
        }
        let pid = std::process::id();
        let me = process(pid).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "process start time is not available on this system",
            )
        })?;
        Ok(Self {
            app: app.to_owned(),
            app_version: app_version.to_owned(),
            pid,
            process_start_us: me.start_us,
            transport,
            engine,
            capabilities,
            written_at: crate::time::now_utc(),
            library: None,
        })
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    pub fn to_json(&self) -> Value {
        let transport = match &self.transport {
            Transport::Unix { path } => json!({"kind": "unix", "path": path.to_string_lossy()}),
            Transport::Pipe { name } => json!({"kind": "pipe", "name": name}),
        };
        let mut value = json!({
            "contract": CONTRACT,
            "app": self.app,
            "app_version": self.app_version,
            "pid": self.pid,
            "process_start_us": self.process_start_us,
            "transport": transport,
            "engine": {
                "contract": self.engine.contract,
                "protocol": self.engine.protocol,
                "engine": self.engine.engine,
                "version": self.engine.version,
            },
            "capabilities": self.capabilities,
            "written_at": self.written_at,
        });
        if let Some(library) = &self.library {
            value["library"] = json!(library);
        }
        value
    }
}

fn string(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// §2, §3 and §6 for one record: the record, or the problem token of the examples.
pub fn parse(file_name: &str, bytes: &[u8]) -> Result<Record, &'static str> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("too_large");
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "not_json")?;
    let Value::Object(map) = value else {
        return Err("not_object");
    };
    for (key, kind) in [
        ("contract", "string"),
        ("app", "string"),
        ("app_version", "string"),
        ("pid", "int"),
        ("process_start_us", "int"),
        ("transport", "object"),
        ("engine", "object"),
        ("capabilities", "array"),
        ("written_at", "string"),
    ] {
        let Some(field) = map.get(key) else {
            return Err("missing_field");
        };
        let fits = match kind {
            "string" => field.is_string(),
            "int" => field.is_u64() || field.is_i64(),
            "object" => field.is_object(),
            _ => field.is_array(),
        };
        if !fits {
            return Err("wrong_type");
        }
    }
    if map["contract"] != CONTRACT {
        return Err("unknown_contract");
    }
    let app = string(&map, "app");
    if !valid_app(&app) {
        return Err("bad_app");
    }
    if file_name.strip_suffix(".json") != Some(app.as_str()) {
        return Err("app_mismatch");
    }
    let pid = map["pid"]
        .as_u64()
        .filter(|pid| (1..=u64::from(u32::MAX)).contains(pid))
        .ok_or("bad_pid")? as u32;
    let start = map["process_start_us"]
        .as_u64()
        .filter(|start| (1..=MAX_SAFE).contains(start))
        .ok_or("bad_start")?;
    let written_at = string(&map, "written_at");
    if !valid_utc(&written_at) {
        return Err("bad_time");
    }
    let engine = map["engine"].as_object().ok_or("bad_engine")?;
    let field = |key: &str| {
        engine
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or("bad_engine")
    };
    let engine = EngineRef {
        contract: field("contract")?,
        protocol: field("protocol")?,
        engine: field("engine")?,
        version: field("version")?,
    };
    let mut capabilities = Vec::new();
    for capability in map["capabilities"].as_array().into_iter().flatten() {
        match capability.as_str() {
            Some(c) if valid_capability(c) => capabilities.push(c.to_owned()),
            _ => return Err("bad_capability"),
        }
    }
    let transport = map["transport"].as_object().ok_or("bad_transport")?;
    let transport = match transport.get("kind").and_then(Value::as_str) {
        Some("unix") => match transport.get("path").and_then(Value::as_str) {
            Some(path) if path.starts_with('/') => Transport::Unix { path: path.into() },
            _ => return Err("bad_transport"),
        },
        Some("pipe") => match transport.get("name").and_then(Value::as_str) {
            Some(name) if name.starts_with(r"\\.\pipe\") && name.len() > 9 => Transport::Pipe {
                name: name.to_owned(),
            },
            _ => return Err("bad_transport"),
        },
        _ => return Err("unknown_transport"),
    };
    Ok(Record {
        app,
        app_version: string(&map, "app_version"),
        pid,
        process_start_us: start,
        transport,
        engine,
        capabilities,
        written_at,
        library: map
            .get("library")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// §4: the pid exists, belongs to this user and started when the record says.
pub fn is_live(record: &Record) -> bool {
    match (process(record.pid), current_uid()) {
        (Some(p), Some(uid)) => p.uid == uid && p.start_us == record.process_start_us,
        _ => false,
    }
}

/// What a client sees for one `<app>.json` (§6).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum State {
    Live(Record),
    Stale(Record),
    Invalid(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub app: String,
    pub state: State,
}

/// §5: the folder must be this user's and private before anything in it is trusted.
fn folder_is_private(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::symlink_metadata(dir)?;
        if !meta.is_dir() || Some(meta.uid()) != current_uid() || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the registry folder is not a private folder of this user",
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

fn read_private(path: &Path) -> Result<Vec<u8>, &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "unreadable")?;
    if !meta.is_file() {
        return Err("unsafe_permissions");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if Some(meta.uid()) != current_uid() || meta.mode() & 0o077 != 0 {
            return Err("unsafe_permissions");
        }
    }
    let file = fs::File::open(path).map_err(|_| "unreadable")?;
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreadable")?;
    Ok(bytes)
}

/// One app's entry, or `None` when it has no file.
pub fn read_entry(dir: &Path, app: &str) -> io::Result<Option<Entry>> {
    if !valid_app(app) {
        return Ok(None);
    }
    let path = dir.join(format!("{app}.json"));
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    folder_is_private(dir)?;
    Ok(Some(entry(app, &path)))
}

fn entry(app: &str, path: &Path) -> Entry {
    let state = match read_private(path).and_then(|bytes| parse(&format!("{app}.json"), &bytes)) {
        Ok(record) if is_live(&record) => State::Live(record),
        Ok(record) => State::Stale(record),
        Err(problem) => State::Invalid(problem),
    };
    Entry {
        app: app.to_owned(),
        state,
    }
}

/// Every registry file in the folder, sorted by app. A missing folder is an empty registry.
pub fn scan(dir: &Path) -> io::Result<Vec<Entry>> {
    let listing = match fs::read_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        other => other?,
    };
    folder_is_private(dir)?;
    let mut entries = Vec::new();
    for item in listing {
        let item = item?;
        let Some(name) = item.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if valid_file_name(&name) {
            entries.push(entry(&name[..name.len() - 5], &item.path()));
        }
    }
    entries.sort_by(|a, b| a.app.cmp(&b.app));
    Ok(entries)
}

/// An engine's own record, removed on drop if it is still this engine's (§5).
#[derive(Debug)]
pub struct Registration {
    path: PathBuf,
    pid: u32,
    start: u64,
}

impl Registration {
    /// Write the record once the engine accepts connections. Fails with `AlreadyExists`
    /// when another live engine of the same app is registered.
    pub fn register(dir: &Path, record: &Record) -> io::Result<Self> {
        create_private_dir(dir)?;
        if let Some(Entry {
            state: State::Live(other),
            ..
        }) = read_entry(dir, &record.app)?
            && (other.pid, other.process_start_us) != (record.pid, record.process_start_us)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} already has a live engine (pid {})",
                    record.app, other.pid
                ),
            ));
        }
        let path = dir.join(format!("{}.json", record.app));
        let bytes = serde_json::to_vec(&record.to_json()).map_err(io::Error::other)?;
        nu9ve_fs::atomic_write_with(&path, &bytes, nu9ve_fs::SyncMode::Plain)?;
        Ok(Self {
            path,
            pid: record.pid,
            start: record.process_start_us,
        })
    }

    /// Remove the record if it still names this engine. Another one may have replaced it.
    pub fn unregister(&self) -> io::Result<()> {
        let Ok(bytes) = read_private(&self.path) else {
            return Ok(());
        };
        let name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        match parse(name, &bytes) {
            Ok(record) if (record.pid, record.process_start_us) == (self.pid, self.start) => {
                match fs::remove_file(&self.path) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        builder.mode(0o700);
        builder.create(dir)?;
        let meta = fs::symlink_metadata(dir)?;
        if meta.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
    }
    #[cfg(not(unix))]
    builder.create(dir)?;
    folder_is_private(dir)
}
