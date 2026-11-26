//! nu9ve-profile-v1: the profiles of the suite on this machine. `profiles.json` lists each profile (a
//! stable `profile_id`, a name, a kind) and, per app, where that app keeps the profile's library. Each app
//! writes only its own key, under a lock; every app may read it all. Knows no app.
//!
//! The reference is `scripts/nu9ve_profiles.py`; `data/profile-examples-v1.json` holds the cases both
//! answer the same way. Changes are JSON objects (`{"op": "upsert_app_entry", …}`, §5) so the two
//! languages share one description; the builders below make them.

use serde_json::{Map, Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const CONTRACT: &str = "nu9ve-profile/1";
pub const MAX_BYTES: usize = 64 * 1024;
pub const MAX_PROFILES: usize = 64;
pub const MAX_APPS: usize = 32;
pub const MAX_ALIASES: usize = 16;
pub const MAX_RETIRED: usize = 1024;
const MAX_PATH: usize = 1024;
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
pub const KINDS: [&str; 3] = ["person", "team", "viewer"];
pub const UIS: [&str; 2] = ["full", "simple"];

// ------------------------------------------------------------------ §1

/// §1: the file for this platform and environment, or `None` (never a guess).
pub fn file_for(os: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(over) = env("NU9VE_PROFILES_FILE") {
        let absolute = if os == "windows" {
            drive_path(&over)
        } else {
            over.starts_with('/')
        };
        return absolute.then(|| PathBuf::from(over));
    }
    match os {
        "macos" => env("HOME").filter(|h| h.starts_with('/')).map(|h| {
            PathBuf::from(h.trim_end_matches('/'))
                .join("Library/Application Support/nu9ve/profiles.json")
        }),
        "windows" => env("LOCALAPPDATA").filter(|l| !l.is_empty()).map(|l| {
            PathBuf::from(format!(
                "{}\\nu9ve\\profiles.json",
                l.trim_end_matches('\\')
            ))
        }),
        "linux" => match env("XDG_DATA_HOME").filter(|d| d.starts_with('/')) {
            Some(d) => Some(PathBuf::from(d.trim_end_matches('/')).join("nu9ve/profiles.json")),
            None => env("HOME").filter(|h| h.starts_with('/')).map(|h| {
                PathBuf::from(h.trim_end_matches('/')).join(".local/share/nu9ve/profiles.json")
            }),
        },
        _ => None,
    }
}

/// This system's name in the contract.
pub fn this_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// §1 for this process.
pub fn default_file() -> Option<PathBuf> {
    file_for(this_os(), &|k| std::env::var(k).ok())
}

// ------------------------------------------------------------- §2, §3

fn drive_path(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

fn text_ok(value: &str, longest: usize) -> bool {
    (1..=longest).contains(&value.chars().count()) && !value.chars().any(char::is_control)
}

/// §2: a name a person reads: 1 to 64 characters, no control characters.
pub fn name_ok(value: &str) -> bool {
    text_ok(value, 64)
}

/// §2: `^[a-z0-9][a-z0-9-]{0,31}$`.
pub fn profile_id_ok(id: &str) -> bool {
    let b = id.as_bytes();
    (1..=32).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// §2: the app key, as in the engine registry: `^[a-z][a-z0-9-]{0,31}$`.
pub fn app_ok(app: &str) -> bool {
    let b = app.as_bytes();
    (1..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// §2: an absolute path of this machine (POSIX, drive or UNC), up to 1024 characters.
pub fn library_ok(value: &str) -> bool {
    text_ok(value, MAX_PATH)
        && (value.starts_with('/') || drive_path(value) || value.starts_with("\\\\"))
}

fn uuid4(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => *c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

/// RFC 3339 in UTC with `Z`, as `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?Z$`.
pub fn utc_time_ok(text: &str) -> bool {
    let b = text.as_bytes();
    let digits = |r: std::ops::Range<usize>| {
        r.into_iter()
            .all(|i| b.get(i).is_some_and(u8::is_ascii_digit))
    };
    if b.len() < 20
        || !digits(0..4)
        || b[4] != b'-'
        || !digits(5..7)
        || b[7] != b'-'
        || !digits(8..10)
        || b[10] != b'T'
        || !digits(11..13)
        || b[13] != b':'
        || !digits(14..16)
        || b[16] != b':'
        || !digits(17..19)
    {
        return false;
    }
    match &b[19..] {
        [b'Z'] => true,
        [b'.', rest @ .., b'Z'] => {
            (1..=9).contains(&rest.len()) && rest.iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Str,
    OptStr,
    List,
    Object,
    Bool,
}

fn fits(value: &Value, kind: Kind) -> bool {
    match kind {
        Kind::Str => value.is_string(),
        Kind::OptStr => value.is_null() || value.is_string(),
        Kind::List => value.is_array(),
        Kind::Object => value.is_object(),
        Kind::Bool => value.is_boolean(),
    }
}

fn fields(
    map: &Map<String, Value>,
    required: &[(&str, Kind)],
    optional: &[(&str, Kind)],
) -> Option<&'static str> {
    for (name, kind) in required {
        match map.get(*name) {
            None => return Some("missing_field"),
            Some(v) if !fits(v, *kind) => return Some("wrong_type"),
            _ => {}
        }
    }
    for (name, kind) in optional {
        if map.get(*name).is_some_and(|v| !fits(v, *kind)) {
            return Some("wrong_type");
        }
    }
    None
}

fn str_of<'a>(map: &'a Map<String, Value>, key: &str) -> &'a str {
    map.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn opt_str<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

fn aliases_ok(value: &Value) -> bool {
    let Some(list) = value.as_array() else {
        return false;
    };
    let mut seen = std::collections::BTreeSet::new();
    list.len() <= MAX_ALIASES
        && list
            .iter()
            .all(|a| a.as_str().is_some_and(|a| name_ok(a) && seen.insert(a)))
}

fn entry_problem(entry: &Value) -> Option<&'static str> {
    let Some(map) = entry.as_object() else {
        return Some("wrong_type");
    };
    if let Some(problem) = fields(
        map,
        &[("library", Kind::Str), ("default", Kind::Bool)],
        &[("last_used_at", Kind::OptStr), ("ui", Kind::OptStr)],
    ) {
        return Some(problem);
    }
    if !library_ok(str_of(map, "library")) {
        return Some("bad_library");
    }
    if opt_str(map, "last_used_at").is_some_and(|t| !utc_time_ok(t)) {
        return Some("bad_time");
    }
    if opt_str(map, "ui").is_some_and(|u| !UIS.contains(&u)) {
        return Some("bad_ui");
    }
    None
}

fn profile_problem(profile: &Value) -> Option<&'static str> {
    let Some(map) = profile.as_object() else {
        return Some("wrong_type");
    };
    if let Some(problem) = fields(
        map,
        &[
            ("profile_id", Kind::Str),
            ("display_name", Kind::Str),
            ("kind", Kind::Str),
            ("created_at", Kind::Str),
            ("apps", Kind::Object),
        ],
        &[
            ("aliases", Kind::List),
            ("default_space", Kind::OptStr),
            ("account_link", Kind::OptStr),
        ],
    ) {
        return Some(problem);
    }
    if !profile_id_ok(str_of(map, "profile_id")) {
        return Some("bad_profile_id");
    }
    if !name_ok(str_of(map, "display_name")) {
        return Some("bad_name");
    }
    if !KINDS.contains(&str_of(map, "kind")) {
        return Some("bad_kind");
    }
    if !utc_time_ok(str_of(map, "created_at")) {
        return Some("bad_time");
    }
    if map.get("aliases").is_some_and(|a| !aliases_ok(a)) {
        return Some("bad_alias");
    }
    if opt_str(map, "default_space").is_some_and(|s| !text_ok(s, 128)) {
        return Some("bad_space");
    }
    if opt_str(map, "account_link").is_some_and(|s| !text_ok(s, 256)) {
        return Some("bad_account_link");
    }
    let apps = map["apps"]
        .as_object()
        .map(sorted_entries)
        .unwrap_or_default();
    if apps.len() > MAX_APPS {
        return Some("too_many_apps");
    }
    for (app, entry) in apps {
        if !app_ok(app) {
            return Some("bad_app");
        }
        if let Some(problem) = entry_problem(entry) {
            return Some(problem);
        }
    }
    None
}

/// The entries of an object in code-point order, whatever the map's own order is.
fn sorted_entries(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

/// §2 and §3 on a parsed document: the first problem, or `None` when it is valid.
pub fn document_problem(value: &Value) -> Option<&'static str> {
    let Some(map) = value.as_object() else {
        return Some("not_object");
    };
    if let Some(problem) = fields(
        map,
        &[
            ("contract", Kind::Str),
            ("device_id", Kind::OptStr),
            ("profiles", Kind::List),
            ("updated_at", Kind::Str),
        ],
        &[("retired", Kind::List)],
    ) {
        return Some(problem);
    }
    if str_of(map, "contract") != CONTRACT {
        return Some("unknown_contract");
    }
    if opt_str(map, "device_id").is_some_and(|d| !uuid4(d)) {
        return Some("bad_device_id");
    }
    if !utc_time_ok(str_of(map, "updated_at")) {
        return Some("bad_time");
    }
    let profiles = map["profiles"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if profiles.len() > MAX_PROFILES {
        return Some("too_many_profiles");
    }
    let mut seen = std::collections::BTreeSet::new();
    for profile in profiles {
        if let Some(problem) = profile_problem(profile) {
            return Some(problem);
        }
        if !seen.insert(profile["profile_id"].as_str().unwrap_or_default()) {
            return Some("duplicate_profile");
        }
    }
    let mut defaults = std::collections::BTreeMap::<&str, usize>::new();
    for profile in profiles {
        for (app, entry) in profile["apps"].as_object().into_iter().flatten() {
            if entry["default"] == Value::Bool(true) {
                *defaults.entry(app.as_str()).or_default() += 1;
            }
        }
    }
    if defaults.values().any(|count| *count > 1) {
        return Some("two_defaults");
    }
    if let Some(retired) = map.get("retired").and_then(Value::as_array) {
        let mut unique = std::collections::BTreeSet::new();
        let ok = retired.len() <= MAX_RETIRED
            && retired.iter().all(|r| {
                r.as_str().is_some_and(|r| {
                    profile_id_ok(r) && !seen.contains(r) && unique.insert(r.to_owned())
                })
            });
        if !ok {
            return Some("bad_retired");
        }
    }
    None
}

/// §3: the first rule the file's bytes break, or `None`.
pub fn file_problem(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > MAX_BYTES {
        return Some("too_large");
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Some("bom");
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Some("not_json");
    };
    document_problem(&value)
}

// ------------------------------------------------------------------ §5

/// §4: what a writer starts from when there is no file.
pub fn new_document(device_id: Option<&str>, now: &str) -> Value {
    json!({"contract": CONTRACT, "device_id": device_id, "profiles": [], "updated_at": now})
}

fn check_fields(op: &Map<String, Value>) -> Result<(), &'static str> {
    if op
        .get("display_name")
        .is_some_and(|n| !n.as_str().is_some_and(name_ok))
    {
        return Err("bad_name");
    }
    if op
        .get("kind")
        .is_some_and(|k| !k.as_str().is_some_and(|k| KINDS.contains(&k)))
    {
        return Err("bad_kind");
    }
    if op.get("aliases").is_some_and(|a| !aliases_ok(a)) {
        return Err("bad_alias");
    }
    let optional = |key: &str, longest: usize| {
        op.get(key)
            .is_some_and(|v| !v.is_null() && !v.as_str().is_some_and(|s| text_ok(s, longest)))
    };
    if optional("default_space", 128) {
        return Err("bad_space");
    }
    if optional("account_link", 256) {
        return Err("bad_account_link");
    }
    Ok(())
}

const PROFILE_FIELDS: [&str; 5] = [
    "display_name",
    "kind",
    "aliases",
    "default_space",
    "account_link",
];

fn find_index(profiles: &[Value], id: &str) -> Result<usize, &'static str> {
    profiles
        .iter()
        .position(|p| p["profile_id"] == id)
        .ok_or("unknown_profile")
}

/// §5: one change on a valid document, at `now`. The new document, or the code that refused it.
pub fn apply(doc: &Value, change: &Value, now: &str) -> Result<Value, &'static str> {
    let empty = Map::new();
    let op = change.as_object().unwrap_or(&empty);
    let profile_id = opt_str(op, "profile_id")
        .filter(|id| profile_id_ok(id))
        .ok_or("bad_profile_id")?
        .to_owned();
    let mut doc = doc.clone();
    let retired: Vec<String> = doc
        .get("retired")
        .and_then(Value::as_array)
        .map(|r| {
            r.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut profiles = match doc.get_mut("profiles").map(Value::take) {
        Some(Value::Array(profiles)) => profiles,
        _ => return Err("bad_change"),
    };
    let outcome = change_profiles(&mut profiles, op, &profile_id, &retired, now);
    doc["profiles"] = Value::Array(profiles);
    if outcome? {
        match doc.get_mut("retired").and_then(Value::as_array_mut) {
            Some(list) => list.push(json!(profile_id)),
            None => doc["retired"] = json!([profile_id]),
        }
    }
    doc["updated_at"] = json!(now);
    let bytes = serde_json::to_vec(&doc).map_err(|_| "bad_change")?;
    match file_problem(&bytes) {
        Some(problem) => Err(problem),
        None => Ok(doc),
    }
}

/// The change on the list of profiles; `true` when the id is retired (a deleted profile).
fn change_profiles(
    profiles: &mut Vec<Value>,
    op: &Map<String, Value>,
    profile_id: &str,
    retired: &[String],
    now: &str,
) -> Result<bool, &'static str> {
    match str_of(op, "op") {
        "create_profile" => {
            if !op.contains_key("display_name") || !op.contains_key("kind") {
                return Err("bad_change");
            }
            check_fields(op)?;
            if find_index(profiles, profile_id).is_ok() {
                return Err("profile_exists");
            }
            if retired.iter().any(|r| r == profile_id) {
                return Err("profile_retired");
            }
            if profiles.len() >= MAX_PROFILES {
                return Err("too_many_profiles");
            }
            let field = |key: &str, empty: Value| op.get(key).cloned().unwrap_or(empty);
            profiles.push(json!({
                "profile_id": profile_id,
                "display_name": op["display_name"],
                "kind": op["kind"],
                "aliases": field("aliases", json!([])),
                "default_space": field("default_space", Value::Null),
                "account_link": field("account_link", Value::Null),
                "created_at": now,
                "apps": {},
            }));
            Ok(false)
        }
        "update_profile" => {
            check_fields(op)?;
            let i = find_index(profiles, profile_id)?;
            for key in PROFILE_FIELDS {
                if let Some(value) = op.get(key) {
                    profiles[i][key] = value.clone();
                }
            }
            Ok(false)
        }
        kind @ ("upsert_app_entry" | "remove_app_entry") => {
            let app = opt_str(op, "app")
                .filter(|a| app_ok(a))
                .ok_or("bad_app")?
                .to_owned();
            let i = find_index(profiles, profile_id)?;
            if kind == "remove_app_entry" {
                if let Some(apps) = profiles[i]["apps"].as_object_mut() {
                    apps.remove(&app);
                }
                return Ok(false);
            }
            let library = opt_str(op, "library")
                .filter(|l| library_ok(l))
                .ok_or("bad_library")?
                .to_owned();
            let default = match op.get("default") {
                None => false,
                Some(Value::Bool(b)) => *b,
                Some(_) => return Err("bad_change"),
            };
            let ui = match op.get("ui") {
                None | Some(Value::Null) => Value::Null,
                Some(value) => json!(value.as_str().filter(|u| UIS.contains(u)).ok_or("bad_ui")?),
            };
            let apps = profiles[i]["apps"].as_object_mut().ok_or("bad_change")?;
            if !apps.contains_key(&app) && apps.len() >= MAX_APPS {
                return Err("too_many_apps");
            }
            let entry = apps.entry(app.clone()).or_insert_with(|| json!({}));
            if !entry.is_object() {
                *entry = json!({});
            }
            entry["library"] = json!(library);
            entry["last_used_at"] = json!(now);
            entry["default"] = json!(default);
            entry["ui"] = ui;
            if default {
                for (j, other) in profiles.iter_mut().enumerate() {
                    if j != i
                        && let Some(entry) = other["apps"].get_mut(app.as_str())
                    {
                        entry["default"] = json!(false);
                    }
                }
            }
            Ok(false)
        }
        "delete_profile" => {
            let i = find_index(profiles, profile_id)?;
            if profiles[i]["apps"]
                .as_object()
                .is_some_and(|apps| !apps.is_empty())
            {
                return Err("profile_in_use");
            }
            profiles.remove(i);
            Ok(true)
        }
        _ => Err("bad_change"),
    }
}

/// §5 `create_profile`.
pub fn create_profile_change(profile_id: &str, display_name: &str, kind: &str) -> Value {
    json!({"op": "create_profile", "profile_id": profile_id, "display_name": display_name, "kind": kind})
}

/// §5 `upsert_app_entry`.
pub fn upsert_app_change(
    profile_id: &str,
    app: &str,
    library: &str,
    default: bool,
    ui: Option<&str>,
) -> Value {
    json!({"op": "upsert_app_entry", "profile_id": profile_id, "app": app, "library": library,
           "default": default, "ui": ui})
}

/// §5 `remove_app_entry`.
pub fn remove_app_change(profile_id: &str, app: &str) -> Value {
    json!({"op": "remove_app_entry", "profile_id": profile_id, "app": app})
}

/// §5 `delete_profile`.
pub fn delete_profile_change(profile_id: &str) -> Value {
    json!({"op": "delete_profile", "profile_id": profile_id})
}

// ------------------------------------------------------------------ §6

/// One profile as one app sees it (§6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppProfile {
    pub profile_id: String,
    pub display_name: String,
    pub kind: String,
    pub library: String,
    pub last_used_at: Option<String>,
    pub default: bool,
    pub ui: Option<String>,
}

/// §6: the profiles with an entry for `app`: the default one first, then the most recently used, then by id.
pub fn for_app(doc: Option<&Value>, app: &str) -> Vec<AppProfile> {
    let mut rows: Vec<AppProfile> = doc
        .and_then(|d| d["profiles"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|profile| {
            let entry = profile["apps"].get(app)?;
            Some(AppProfile {
                profile_id: profile["profile_id"].as_str()?.to_owned(),
                display_name: profile["display_name"].as_str()?.to_owned(),
                kind: profile["kind"].as_str()?.to_owned(),
                library: entry["library"].as_str()?.to_owned(),
                last_used_at: entry["last_used_at"].as_str().map(str::to_owned),
                default: entry["default"].as_bool().unwrap_or(false),
                ui: entry["ui"].as_str().map(str::to_owned),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.default
            .cmp(&a.default)
            .then_with(|| b.last_used_at.cmp(&a.last_used_at))
            .then_with(|| a.profile_id.cmp(&b.profile_id))
    });
    rows
}

/// The profile with this id, if the document has it.
pub fn find<'a>(doc: &'a Value, profile_id: &str) -> Option<&'a Value> {
    doc["profiles"]
        .as_array()?
        .iter()
        .find(|p| p["profile_id"] == profile_id)
}

// ------------------------------------------------------------------ §4

/// Why reading or changing the file failed.
#[derive(Debug)]
pub enum Error {
    /// The file breaks §3 (its code); it is left as it is.
    Invalid(&'static str),
    /// The change was refused (§5 code); nothing was written.
    Refused(&'static str),
    /// Another process held the lock for longer than the timeout.
    Busy,
    Io(io::Error),
}

impl Error {
    /// `invalid`, `busy`, `io` or the refusal's own code.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::Refused(code) => code,
            Error::Busy => "busy",
            Error::Io(_) => "io",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Invalid(problem) => write!(
                f,
                "profiles.json is invalid ({problem}); it is left as it is"
            ),
            Error::Refused(code) => write!(f, "the change was refused: {code}"),
            Error::Busy => write!(f, "profiles.json is locked by another process"),
            Error::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

/// §4: the document; `None` when there is no file. A file that is not regular or that the group or others
/// can read is `Invalid("unsafe_permissions")` on Unix.
pub fn read(path: &Path) -> Result<Option<Value>, Error> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_file() {
        return Err(Error::Invalid("unsafe_permissions"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(Error::Invalid("unsafe_permissions"));
        }
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if let Some(problem) = file_problem(&bytes) {
        return Err(Error::Invalid(problem));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| Error::Invalid("not_json"))
}

fn lock_file(path: &Path) -> Result<File, Error> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path.with_file_name(name))?;
    match nu9ve_fs::lock(&file, nu9ve_fs::LockMode::Exclusive, LOCK_TIMEOUT) {
        Ok(()) => Ok(file),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(Error::Busy),
        Err(e) => Err(e.into()),
    }
}

/// §4 and §5: apply one change to the file under the lock and publish it atomically. A missing file starts
/// from `new_document(device_id)`; a file without a device takes `device_id`. Returns the new document.
pub fn change(path: &Path, change: &Value, device_id: Option<&str>) -> Result<Value, Error> {
    change_at(path, change, device_id, &now_utc())
}

/// `change` at a given instant (RFC 3339 UTC with `Z`).
pub fn change_at(
    path: &Path,
    change: &Value,
    device_id: Option<&str>,
    now: &str,
) -> Result<Value, Error> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::other("profiles.json needs a parent folder"))?;
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let _lock = lock_file(path)?;
    let mut doc = match read(path)? {
        Some(doc) => doc,
        None => new_document(device_id, now),
    };
    if doc["device_id"].is_null()
        && let Some(id) = device_id
    {
        doc["device_id"] = json!(id);
    }
    let doc = apply(&doc, change, now).map_err(Error::Refused)?;
    let bytes = serde_json::to_vec(&doc).map_err(io::Error::other)?;
    nu9ve_fs::atomic_write_with(path, &bytes, nu9ve_fs::SyncMode::Plain)
        .map_err(io::Error::from)?;
    Ok(doc)
}

// ---------------------------------------------------------------- time

/// The current time as RFC 3339 UTC with `Z`, to the second.
pub fn now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    utc_at(secs)
}

/// `secs` since 1970 as RFC 3339 UTC with `Z`. Strings of this form sort as their instants.
pub fn utc_at(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}
