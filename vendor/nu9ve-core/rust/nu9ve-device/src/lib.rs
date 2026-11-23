//! nu9ve-device-v1: the local identity of a machine. `device.json` holds a random UUID 4 that never
//! changes and the name the person gives it; `host_check` notices when the file was copied to another
//! machine, without keeping any hardware identifier. Knows no app.

use serde_json::{Value, json};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const CONTRACT: &str = "nu9ve-device/1";
const MAX_BYTES: usize = 4 * 1024;

/// §1: the file for this platform and environment, or `None` (never a guess).
pub fn file_for(os: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(over) = env("NU9VE_DEVICE_FILE") {
        let absolute = if os == "windows" {
            over.len() > 2 && over.as_bytes()[0].is_ascii_alphabetic() && &over[1..3] == ":\\"
        } else {
            over.starts_with('/')
        };
        return absolute.then(|| PathBuf::from(over));
    }
    match os {
        "macos" => env("HOME").filter(|h| h.starts_with('/')).map(|h| {
            PathBuf::from(h.trim_end_matches('/'))
                .join("Library/Application Support/nu9ve/device.json")
        }),
        "windows" => env("LOCALAPPDATA")
            .map(|l| PathBuf::from(format!("{}\\nu9ve\\device.json", l.trim_end_matches('\\')))),
        "linux" => match env("XDG_DATA_HOME").filter(|d| d.starts_with('/')) {
            Some(d) => Some(PathBuf::from(d.trim_end_matches('/')).join("nu9ve/device.json")),
            None => env("HOME").filter(|h| h.starts_with('/')).map(|h| {
                PathBuf::from(h.trim_end_matches('/')).join(".local/share/nu9ve/device.json")
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

fn utc_time(text: &str) -> bool {
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

/// §2: a name the person gives: 1 to 64 characters, no control characters.
pub fn name_ok(name: &str) -> bool {
    (1..=64).contains(&name.chars().count()) && !name.chars().any(char::is_control)
}

/// §2: the first rule the file breaks, or `None`.
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
    let Some(map) = value.as_object() else {
        return Some("not_object");
    };
    for field in ["contract", "device_id", "name", "created_at", "os"] {
        match map.get(field) {
            None => return Some("missing_field"),
            Some(v) if !v.is_string() => return Some("wrong_type"),
            _ => {}
        }
    }
    let text = |k: &str| map[k].as_str().unwrap_or_default();
    if text("contract") != CONTRACT {
        return Some("unknown_contract");
    }
    if !uuid4(text("device_id")) {
        return Some("bad_device_id");
    }
    if !name_ok(text("name")) {
        return Some("bad_name");
    }
    if !utc_time(text("created_at")) {
        return Some("bad_time");
    }
    if !["macos", "windows", "linux"].contains(&text("os")) {
        return Some("bad_os");
    }
    if let Some(check) = map.get("host_check") {
        let ok = check.as_str().is_some_and(|c| {
            c.len() == 64
                && c.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        if !ok {
            return Some("bad_host_check");
        }
    }
    if let Some(previous) = map.get("previous_device_id")
        && !previous
            .as_str()
            .is_some_and(|p| uuid4(p) && p != text("device_id"))
    {
        return Some("bad_previous_device_id");
    }
    None
}

/// §5.
pub fn host_check(device_id: &str, platform_id: &str) -> String {
    nu9ve_json::sha256_hex(format!("nu9ve-device/1\n{device_id}\n{platform_id}").as_bytes())
}

/// §2: the name while the person has not chosen one.
pub fn default_name(os: &str, device_id: &str) -> String {
    let system = match os {
        "macos" => "Mac",
        "windows" => "PC",
        _ => "Linux",
    };
    format!("{system} {}", &device_id[..4.min(device_id.len())])
}

/// §4 and §5.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Ok(Value),
    Copied(Value),
    None,
}

impl State {
    pub fn id(&self) -> &'static str {
        match self {
            State::Ok(_) => "ok",
            State::Copied(_) => "copied",
            State::None => "none",
        }
    }
}

/// §4 and §5: what a reader gets from the file's bytes (`None`: no file) and this machine's platform id.
pub fn state(bytes: Option<&[u8]>, platform_id: Option<&str>) -> State {
    let Some(bytes) = bytes else {
        return State::None;
    };
    if file_problem(bytes).is_some() {
        return State::None;
    }
    let value: Value = serde_json::from_slice(bytes).unwrap_or_default();
    if let (Some(check), Some(platform)) =
        (value.get("host_check").and_then(Value::as_str), platform_id)
        && host_check(value["device_id"].as_str().unwrap_or_default(), platform) != check
    {
        return State::Copied(value);
    }
    State::Ok(value)
}

/// §5: this machine's platform id, or `None` when it cannot be read.
pub fn platform_id() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/sbin/ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text.lines().find(|l| l.contains("\"IOPlatformUUID\""))?;
        let value = line.rsplit('"').nth(1)?;
        (!value.is_empty()).then(|| value.to_owned())
    }
    #[cfg(target_os = "linux")]
    {
        let id = fs::read_to_string("/etc/machine-id").ok()?;
        let id = id.trim_end_matches('\n').to_owned();
        (!id.is_empty()).then_some(id)
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("reg")
            .args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        text.lines()
            .find(|l| l.contains("MachineGuid"))
            .and_then(|l| l.split_whitespace().last())
            .map(str::to_owned)
    }
}

/// The current time as RFC 3339 UTC with `Z`, to the second.
pub fn now_utc() -> String {
    utc_at(unix_now())
}

/// Seconds since 1970 UTC.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64
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

fn new_device(previous: Option<&Value>, platform_id: Option<&str>) -> io::Result<Value> {
    let id = nu9ve_json::uid().map_err(|e| io::Error::other(e.message))?;
    let os = this_os();
    let name = previous
        .and_then(|p| p["name"].as_str())
        .map(|n| n.to_owned())
        .unwrap_or_else(|| default_name(os, &id));
    let mut value = json!({"contract": CONTRACT, "device_id": id, "name": name, "created_at": now_utc(), "os": os});
    if let Some(platform) = platform_id {
        value["host_check"] = json!(host_check(&id, platform));
    }
    if let Some(prev) = previous.and_then(|p| p["device_id"].as_str()) {
        value["previous_device_id"] = json!(prev);
    }
    Ok(value)
}

fn write_temp(path: &Path, value: &Value) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("no parent folder"))?;
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let suffix =
        nu9ve_json::hex(&nu9ve_json::random_bytes::<6>().map_err(|e| io::Error::other(e.message))?);
    let temp = parent.join(format!(".device.json.{suffix}.tmp"));
    let mut file = nu9ve_fs::create_new(&temp)?;
    file.write_all(&serde_json::to_vec(value).map_err(io::Error::other)?)?;
    file.sync_all()?;
    Ok(temp)
}

/// What `load_or_create` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Existing,
    Created,
    /// The file came from another machine: a new device replaced it, with `previous_device_id` (§5).
    ReplacedCopy,
}

/// §3 and §5: the device of this machine at `path`, created the first time, without ever making two ids.
/// An invalid file is left as it is and is an error: there is no device until a person looks at it.
pub fn load_or_create(path: &Path, platform_id: Option<&str>) -> io::Result<(Value, Outcome)> {
    let read = |p: &Path| match fs::read(p) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    };
    match read(path)? {
        Some(bytes) => {
            if let Some(problem) = file_problem(&bytes) {
                return Err(io::Error::other(format!(
                    "{}: {problem}; it is left as it is",
                    path.display()
                )));
            }
            match state(Some(&bytes), platform_id) {
                State::Copied(old) => {
                    let value = new_device(Some(&old), platform_id)?;
                    let temp = write_temp(path, &value)?;
                    nu9ve_fs::replace(&temp, path)?;
                    Ok((value, Outcome::ReplacedCopy))
                }
                State::Ok(value) => Ok((value, Outcome::Existing)),
                State::None => Err(io::Error::other("unreadable device.json")),
            }
        }
        None => {
            let value = new_device(None, platform_id)?;
            let temp = write_temp(path, &value)?;
            let published = nu9ve_fs::publish_new_locked(&temp, path, nu9ve_fs::SyncMode::Plain);
            let _ = fs::remove_file(&temp);
            match published {
                Ok(_) => Ok((value, Outcome::Created)),
                // Another process won the race: use its file (§3).
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let bytes = read(path)?.unwrap_or_default();
                    match state(Some(&bytes), platform_id) {
                        State::Ok(v) => Ok((v, Outcome::Existing)),
                        _ => Err(io::Error::other("device.json changed while it was created")),
                    }
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// §3: rename the device, keeping everything else.
pub fn rename(path: &Path, name: &str) -> io::Result<Value> {
    if !name_ok(name) {
        return Err(io::Error::other(
            "a name has 1 to 64 characters and no control characters",
        ));
    }
    let bytes = fs::read(path)?;
    if let Some(problem) = file_problem(&bytes) {
        return Err(io::Error::other(problem));
    }
    let mut value: Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    value["name"] = json!(name);
    let temp = write_temp(path, &value)?;
    nu9ve_fs::replace(&temp, path)?;
    Ok(value)
}
