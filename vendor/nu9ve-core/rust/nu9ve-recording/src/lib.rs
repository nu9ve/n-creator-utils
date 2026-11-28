//! The reader of nu9ve-recording-v1: whether this machine is recording, which session and from which
//! instant its clock counts. [`problem`] checks a state file with the codes of
//! `data/recording-examples-v1.json`, [`status`] adds liveness (nu9ve-registry-v1 §4), [`derive`] is the
//! §4 table (`capturing`, `quiet`, `sessions`), [`is_newer`] orders notifications and [`session_time`] is
//! the §5 clock. Same rules as `Nu9veRecording` in Swift. Producing the signal stays with each producer.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CONTRACT: &str = "nu9ve-recording/1";
pub const HOST_CLOCK: &str = "coremedia-host-clock";
/// §2: a state file is at most 4 KiB.
pub const MAX_BYTES: usize = 4 * 1024;
const MAX_SAFE: i64 = (1 << 53) - 1;
const REQUIRED: [&str; 13] = [
    "contract",
    "app",
    "app_version",
    "pid",
    "process_start_us",
    "seq",
    "state",
    "session_id",
    "epoch",
    "epoch_wall_clock",
    "captures",
    "ended_at",
    "written_at",
];
const CAPTURES: [&str; 4] = ["screen", "system_audio", "microphone", "camera"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Idle,
    Preparing,
    Recording,
    Finalizing,
}

impl State {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "idle" => State::Idle,
            "preparing" => State::Preparing,
            "recording" => State::Recording,
            "finalizing" => State::Finalizing,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Preparing => "preparing",
            State::Recording => "recording",
            State::Finalizing => "finalizing",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Epoch {
    pub clock: String,
    pub value: i64,
    pub timescale: i64,
}

/// One valid state file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub app: String,
    pub app_version: String,
    pub pid: u32,
    pub process_start_us: u64,
    pub seq: u64,
    pub state: State,
    pub session_id: Option<String>,
    pub epoch: Option<Epoch>,
    pub epoch_wall_clock: Option<String>,
    pub captures: Vec<String>,
    pub ended_at: Option<String>,
    pub written_at: String,
}

/// §1, for a platform: `NU9VE_RECORDING_DIR` (absolute) or the folder under the user's (or the QA root's)
/// application data. `None` means there is no signal: a relative override disables it.
pub fn directory_for(
    windows: bool,
    env: &dyn Fn(&str) -> Option<String>,
    qa_root: Option<&str>,
) -> Option<PathBuf> {
    if let Some(over) = env("NU9VE_RECORDING_DIR") {
        let absolute = if windows {
            over.starts_with("\\\\") || over.get(1..3) == Some(":\\")
        } else {
            over.starts_with('/')
        };
        return absolute.then(|| PathBuf::from(over));
    }
    if windows {
        let base = match qa_root {
            Some(root) => format!("{root}\\AppData\\Local"),
            None => env("LOCALAPPDATA")?,
        };
        Some(PathBuf::from(format!("{base}\\nu9ve\\recording")))
    } else {
        let home = qa_root.map(str::to_owned).or_else(|| env("HOME"))?;
        Some(PathBuf::from(format!(
            "{home}/Library/Application Support/nu9ve/recording"
        )))
    }
}

/// §1 on this system, from the process environment.
pub fn directory(qa_root: Option<&Path>) -> Option<PathBuf> {
    let root = qa_root.map(|p| p.to_string_lossy().into_owned());
    directory_for(cfg!(windows), &|k| std::env::var(k).ok(), root.as_deref())
}

fn valid_app(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// §4: only `<app>.json` with a valid app name; temporaries and the rest are not listed.
pub fn is_listed(file_name: &str) -> bool {
    file_name.strip_suffix(".json").is_some_and(valid_app)
}

fn uuid(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

/// RFC 3339 in UTC with `Z`: `YYYY-MM-DDTHH:MM:SS`, optional fraction of 1 to 9 digits (exactly 3 when
/// `millis`).
fn utc(text: &str, millis: bool) -> bool {
    let b = text.as_bytes();
    if b.len() < 20 || b[b.len() - 1] != b'Z' {
        return false;
    }
    let shape = b"0000-00-00T00:00:00";
    let head = b[..19].iter().zip(shape).all(|(c, s)| {
        if *s == b'0' {
            c.is_ascii_digit()
        } else {
            c == s
        }
    });
    let fraction = &b[19..b.len() - 1];
    let fraction_ok = match fraction.split_first() {
        None => !millis,
        Some((b'.', digits)) => {
            digits.iter().all(u8::is_ascii_digit)
                && if millis {
                    digits.len() == 3
                } else {
                    (1..=9).contains(&digits.len())
                }
        }
        Some(_) => false,
    };
    head && fraction_ok
}

/// A JSON integer, never a float or a boolean.
fn integer(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) if n.is_i64() => n.as_i64(),
        Value::Number(n) if n.is_u64() => i64::try_from(n.as_u64()?).ok(),
        _ => None,
    }
}

/// `None` when the file is valid, otherwise the first reason it is not, with the codes of the examples.
pub fn problem(file_name: &str, bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > MAX_BYTES {
        return Some("too_large");
    }
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Some("bom");
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Some("not_json");
    };
    let Value::Object(doc) = value else {
        return Some("not_object");
    };
    check(file_name, &doc).err()
}

fn check(file_name: &str, doc: &Map<String, Value>) -> Result<(), &'static str> {
    if let Some(missing) = REQUIRED.iter().find(|k| !doc.contains_key(**k)) {
        return Err(missing_code(missing));
    }
    let text = |k: &str| doc.get(k).and_then(Value::as_str);
    if text("contract") != Some(CONTRACT) {
        return Err("bad_contract");
    }
    let app = text("app").ok_or("app_mismatch")?;
    if format!("{app}.json") != file_name || !valid_app(app) {
        return Err("app_mismatch");
    }
    if text("app_version").is_none_or(str::is_empty) {
        return Err("bad_app_version");
    }
    if !integer(doc.get("pid")).is_some_and(|p| (1..=u32::MAX as i64).contains(&p)) {
        return Err("bad_pid");
    }
    if !integer(doc.get("process_start_us")).is_some_and(|p| (0..=MAX_SAFE).contains(&p)) {
        return Err("bad_process_start_us");
    }
    if !integer(doc.get("seq")).is_some_and(|p| (1..=MAX_SAFE).contains(&p)) {
        return Err("bad_seq");
    }
    let state = text("state")
        .and_then(State::from_name)
        .ok_or("bad_state")?;
    if !text("written_at").is_some_and(|t| utc(t, false)) {
        return Err("bad_written_at");
    }
    let session = &doc["session_id"];
    if state == State::Idle {
        if !session.is_null() {
            return Err("bad_session_id");
        }
    } else if !session.as_str().is_some_and(uuid) {
        return Err("bad_session_id");
    }
    let (epoch, wall) = (&doc["epoch"], &doc["epoch_wall_clock"]);
    if matches!(state, State::Recording | State::Finalizing) {
        let ok = epoch.as_object().is_some_and(|e| {
            e.get("clock").and_then(Value::as_str) == Some(HOST_CLOCK)
                && integer(e.get("timescale")).is_some_and(|t| (1..=MAX_SAFE).contains(&t))
                && integer(e.get("value")).is_some_and(|v| (-MAX_SAFE..=MAX_SAFE).contains(&v))
        });
        if !ok {
            return Err("bad_epoch");
        }
        if !wall.as_str().is_some_and(|t| utc(t, true)) {
            return Err("bad_epoch_wall_clock");
        }
    } else {
        if !epoch.is_null() {
            return Err("bad_epoch");
        }
        if !wall.is_null() {
            return Err("bad_epoch_wall_clock");
        }
    }
    let captures = doc["captures"].as_array().ok_or("bad_captures")?;
    let names: Vec<&str> = captures.iter().filter_map(Value::as_str).collect();
    let unique: std::collections::BTreeSet<&str> = names.iter().copied().collect();
    if names.len() != captures.len()
        || names.iter().any(|c| !CAPTURES.contains(c))
        || unique.len() != names.len()
        || (state == State::Idle) != names.is_empty()
    {
        return Err("bad_captures");
    }
    let ended = &doc["ended_at"];
    if state == State::Finalizing {
        if !ended.as_str().is_some_and(|t| utc(t, false)) {
            return Err("bad_ended_at");
        }
    } else if !ended.is_null() {
        return Err("bad_ended_at");
    }
    Ok(())
}

fn missing_code(key: &str) -> &'static str {
    match key {
        "contract" => "missing_contract",
        "app" => "missing_app",
        "app_version" => "missing_app_version",
        "pid" => "missing_pid",
        "process_start_us" => "missing_process_start_us",
        "seq" => "missing_seq",
        "state" => "missing_state",
        "session_id" => "missing_session_id",
        "epoch" => "missing_epoch",
        "epoch_wall_clock" => "missing_epoch_wall_clock",
        "captures" => "missing_captures",
        "ended_at" => "missing_ended_at",
        _ => "missing_written_at",
    }
}

/// The record, or `None` when [`problem`] finds anything.
pub fn parse(file_name: &str, bytes: &[u8]) -> Option<Record> {
    if problem(file_name, bytes).is_some() {
        return None;
    }
    let doc: Map<String, Value> = serde_json::from_slice(bytes).ok()?;
    let text = |k: &str| doc.get(k).and_then(Value::as_str).map(str::to_owned);
    Some(Record {
        app: text("app")?,
        app_version: text("app_version")?,
        pid: u32::try_from(integer(doc.get("pid"))?).ok()?,
        process_start_us: u64::try_from(integer(doc.get("process_start_us"))?).ok()?,
        seq: u64::try_from(integer(doc.get("seq"))?).ok()?,
        state: State::from_name(&text("state")?)?,
        session_id: text("session_id"),
        epoch: doc["epoch"].as_object().map(|e| Epoch {
            clock: e["clock"].as_str().unwrap_or_default().to_owned(),
            value: integer(e.get("value")).unwrap_or(0),
            timescale: integer(e.get("timescale")).unwrap_or(1),
        }),
        epoch_wall_clock: text("epoch_wall_clock"),
        captures: doc["captures"]
            .as_array()?
            .iter()
            .filter_map(|c| c.as_str().map(str::to_owned))
            .collect(),
        ended_at: text("ended_at"),
        written_at: text("written_at")?,
    })
}

/// A process as the registry §4 sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Process {
    pub own_user: bool,
    pub start_us: u64,
}

/// This machine's process table.
pub fn process(pid: u32) -> Option<Process> {
    let p = nu9ve_ipc::process(pid)?;
    Some(Process {
        own_user: Some(p.uid) == nu9ve_ipc::current_uid(),
        start_us: p.start_us,
    })
}

pub fn is_live(record: &Record, processes: &dyn Fn(u32) -> Option<Process>) -> bool {
    processes(record.pid).is_some_and(|p| p.own_user && p.start_us == record.process_start_us)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Invalid,
    Stale,
    Live,
}

/// Invalid and stale both count as idle (§4).
pub fn status(file_name: &str, bytes: &[u8], processes: &dyn Fn(u32) -> Option<Process>) -> Status {
    match parse(file_name, bytes) {
        None => Status::Invalid,
        Some(r) if is_live(&r, processes) => Status::Live,
        Some(_) => Status::Stale,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub app: String,
    pub session_id: String,
    pub state: State,
    pub captures: Vec<String>,
    pub epoch: Option<Epoch>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Derived {
    pub capturing: bool,
    pub quiet: bool,
    pub sessions: Vec<Session>,
}

impl Derived {
    /// The apps whose live file is in `recording`.
    pub fn recording_apps(&self) -> Vec<&str> {
        self.sessions
            .iter()
            .filter(|s| s.state == State::Recording)
            .map(|s| s.app.as_str())
            .collect()
    }
}

/// The §4 table: what a consumer derives from every file of the folder, by file name.
pub fn derive(
    files: &BTreeMap<String, Vec<u8>>,
    processes: &dyn Fn(u32) -> Option<Process>,
) -> Derived {
    let live: Vec<Record> = files
        .iter()
        .filter(|(name, _)| is_listed(name))
        .filter_map(|(name, bytes)| parse(name, bytes))
        .filter(|r| is_live(r, processes))
        .collect();
    Derived {
        capturing: live.iter().any(|r| r.state == State::Recording),
        quiet: live
            .iter()
            .any(|r| matches!(r.state, State::Preparing | State::Recording)),
        sessions: live
            .into_iter()
            .filter(|r| r.state != State::Idle)
            .map(|r| Session {
                app: r.app,
                session_id: r.session_id.unwrap_or_default(),
                state: r.state,
                captures: r.captures,
                epoch: r.epoch,
            })
            .collect(),
    }
}

/// Reads the folder with a cap per file, never through a link, and derives it with this machine's
/// processes. A missing folder is nobody recording.
pub fn read_dir(dir: &Path) -> Derived {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_listed(&name) || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let Ok(file) = std::fs::File::open(entry.path()) else {
            continue;
        };
        let mut bytes = Vec::new();
        use std::io::Read;
        if file
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_ok()
        {
            files.insert(name, bytes);
        }
    }
    derive(&files, &process)
}

/// The identity of a file's writer and its `seq`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
    pub pid: u32,
    pub process_start_us: u64,
    pub seq: u64,
}

/// §4: act on a notification only if its file has a higher `seq` for the same process.
pub fn is_newer(seen: Option<Seen>, file: Seen) -> bool {
    match seen {
        Some(s) if s.pid == file.pid && s.process_start_us == file.process_start_us => {
            file.seq > s.seq
        }
        _ => true,
    }
}

/// §5: µs since the epoch, or `None` before it (no event is written then).
pub fn session_time(epoch: &Epoch, now_ns: i64) -> Option<i64> {
    if epoch.timescale <= 0 {
        return None;
    }
    let epoch_ns = (epoch.value as i128 * 1_000_000_000).div_euclid(epoch.timescale as i128);
    let now = now_ns as i128;
    (now >= epoch_ns).then(|| ((now - epoch_ns) / 1000) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_times_are_checked_by_shape() {
        assert!(utc("2026-11-27T18:00:00Z", false));
        assert!(utc("2026-11-27T18:00:00.123456789Z", false));
        assert!(!utc("2026-11-27T18:00:00Z", true));
        assert!(utc("2026-11-27T18:00:00.123Z", true));
        assert!(!utc("2026-11-27T18:00:00+01:00", false));
        assert!(!utc("2026-11-27 18:00:00Z", false));
    }
}
