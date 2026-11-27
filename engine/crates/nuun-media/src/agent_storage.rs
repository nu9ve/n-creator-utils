//! Python `media/agent_storage.py`: `storage_collections`, the space held by folders that are not media,
//! starting with the agents' (`~/.claude`, `~/.codex`, `~/Library/Application Support/Claude`).
//!
//! Only metadata: folders are listed and every entry is `lstat`ed; no file is opened, no link is followed and
//! the walk never leaves the root's device. Only each root's first-level names are returned; deeper names (which
//! carry repository paths and session ids) are summed into their first-level part. Freeing a cache is a hint
//! for a reviewed plan, never an action. Weekly growth counts each file in the week it appeared (macOS birth
//! time, else mtime). tests/test_agent_storage.py keeps this equal to Python on a fixture.

use nu9ve_json::{Result, require};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CLASSES: [&str; 5] = ["history", "state", "cache", "logs", "other"];
const WEEKS: i64 = 8;
const DAY: i64 = 86400;

type Rules = &'static [(&'static str, &'static str)];

/// agent_storage.py `ROOTS`: the first matching glob wins; a name that matches none is `other`.
const ROOTS: [(&str, &str, &[&str], Rules); 3] = [
    (
        "claude_code",
        "Claude Code",
        &[".claude"],
        &[
            ("projects", "history"),
            ("file-history", "history"),
            ("history.jsonl", "history"),
            ("todos", "history"),
            ("plans", "history"),
            ("transcripts", "history"),
            ("shell-snapshots", "cache"),
            ("session-env", "cache"),
            ("statsig", "cache"),
            ("cache", "cache"),
            ("paste-cache", "cache"),
            ("image-cache", "cache"),
            ("ide", "cache"),
            ("debug", "logs"),
            ("logs", "logs"),
            ("telemetry", "logs"),
            ("settings*.json", "state"),
            ("*.md", "state"),
            ("agents", "state"),
            ("commands", "state"),
            ("skills", "state"),
            ("plugins", "state"),
            ("hooks", "state"),
            ("keybindings.json", "state"),
            (".credentials.json", "state"),
            ("memory", "state"),
            (".claude.json", "state"),
            ("backups", "state"),
            ("state", "state"),
            (".last-cleanup", "state"),
            ("sessions", "history"),
            ("mcp-needs-auth-cache.json", "cache"),
        ],
    ),
    (
        "codex",
        "Codex",
        &[".codex"],
        &[
            ("sessions", "history"),
            ("archived_sessions", "history"),
            ("history.jsonl", "history"),
            ("thread_history*.sqlite*", "history"),
            ("log", "logs"),
            ("logs", "logs"),
            ("logs_*.sqlite*", "logs"),
            ("cache", "cache"),
            ("tmp", "cache"),
            (".tmp", "cache"),
            ("models_cache.json", "cache"),
            ("config.toml", "state"),
            ("auth.json", "state"),
            ("*.md", "state"),
            ("prompts", "state"),
            ("skills", "state"),
            ("rules", "state"),
            ("memories", "state"),
            ("version.json", "state"),
            ("state*.sqlite*", "state"),
            ("session_index.jsonl*", "history"),
            ("attachments", "history"),
            ("generated_images", "history"),
            ("visualizations", "history"),
            ("dictation-history", "history"),
            ("transcription-history.jsonl", "history"),
            ("*.tmp-*", "cache"),
            ("shell_snapshots", "cache"),
            ("vendor_imports", "cache"),
            ("plugins", "state"),
            ("goals_*.sqlite*", "state"),
            ("memories_*.sqlite*", "state"),
            ("queue_*.sqlite*", "state"),
            (".codex-global-state.json*", "state"),
            ("*.lock", "state"),
        ],
    ),
    (
        "claude_app",
        "Claude (app)",
        &["Library", "Application Support", "Claude"],
        &[
            ("Cache", "cache"),
            ("Code Cache", "cache"),
            ("GPUCache", "cache"),
            ("*Dawn*Cache", "cache"),
            ("GPUPersistentCache", "cache"),
            ("VideoDecodeStats", "cache"),
            ("fcache", "cache"),
            ("claude-code-spare", "cache"),
            ("ShaderCache", "cache"),
            ("GrShaderCache", "cache"),
            ("Service Worker", "cache"),
            ("Shared Dictionary", "cache"),
            ("claude-code", "cache"),
            ("claude-code-vm", "cache"),
            ("logs", "logs"),
            ("Crashpad", "logs"),
            ("sentry", "logs"),
            ("IndexedDB", "state"),
            ("Local Storage", "state"),
            ("Session Storage", "state"),
            ("WebStorage", "state"),
            ("blob_storage", "state"),
            ("Preferences", "state"),
            ("*.json", "state"),
            ("Cookies*", "state"),
            ("Network Persistent State", "state"),
            ("TransportSecurity", "state"),
            ("Trust Tokens*", "state"),
            ("local-agent-mode-sessions", "history"),
            ("claude-code-sessions", "history"),
            ("side-session-notes", "history"),
            ("DIPS*", "state"),
            ("Local State", "state"),
            ("Extension State", "state"),
            ("shared_proto_db", "state"),
        ],
    ),
];

/// `fnmatch.fnmatchcase` for the patterns above, which only use `*`.
pub(crate) fn glob(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.as_bytes(), name.as_bytes());
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && p[i] == b'*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if i < p.len() && p[i] == n[j] {
            i += 1;
            j += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|c| *c == b'*')
}

/// (part, class): names matched by a glob share one part named by the glob; an unmatched name is its own
/// `other` part.
pub fn classify(rules: Rules, name: &str) -> (String, &'static str) {
    rules
        .iter()
        .find(|(pattern, _)| glob(pattern, name))
        .map_or((name.to_string(), "other"), |(pattern, kind)| {
            let key = if pattern.contains('*') { pattern } else { name };
            (key.to_string(), kind)
        })
}

fn week_start(day: i64) -> i64 {
    day - (day + 3).rem_euclid(7)
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn date(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("{y:04}-{m:02}-{d:02}")
}

fn iso(seconds: i64) -> Value {
    if seconds == 0 {
        return Value::Null;
    }
    let (day, rest) = (seconds.div_euclid(DAY), seconds.rem_euclid(DAY));
    json!(format!(
        "{}T{:02}:{:02}:{:02}Z",
        date(day),
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    ))
}

#[derive(Default, Clone, Copy)]
struct Total {
    files: u64,
    bytes: u64,
    allocated: u64,
}

impl Total {
    fn add(&mut self, other: Self) {
        self.files += other.files;
        self.bytes += other.bytes;
        self.allocated += other.allocated;
    }
    fn view(self) -> Value {
        json!({"files": self.files, "bytes": self.bytes, "allocated": self.allocated})
    }
}

#[cfg(unix)]
fn identity(stat: &Metadata) -> (u64, u64, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (stat.dev(), stat.ino(), stat.nlink(), stat.blocks() * 512)
}

#[cfg(not(unix))]
fn identity(_: &Metadata) -> (u64, u64, u64, u64) {
    (0, 0, 1, 0)
}

fn seconds(time: std::io::Result<SystemTime>) -> f64 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs_f64())
}

fn mtime(stat: &Metadata) -> f64 {
    seconds(stat.modified())
}

/// agent_storage.py `_created`: macOS birth time, else mtime (Python has `st_birthtime` only there).
fn created(stat: &Metadata) -> f64 {
    if cfg!(target_os = "macos") {
        seconds(stat.created().or_else(|_| stat.modified()))
    } else {
        mtime(stat)
    }
}

struct Walk {
    seen: HashSet<(u64, u64)>,
    unreadable: u64,
    first_week: i64,
    weeks: HashMap<i64, [u64; 5]>,
}

struct Part {
    total: Total,
    newest: i64,
}

impl Walk {
    fn list(path: &Path) -> std::io::Result<Vec<(PathBuf, Metadata)>> {
        let mut children = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            children.push((entry.path(), fs::symlink_metadata(entry.path())?));
        }
        children.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(children)
    }

    fn entry(&mut self, path: &Path, stat: &Metadata, device: u64, kind: usize, part: &mut Part) {
        let (dev, ino, nlink, blocks) = identity(stat);
        if stat.is_dir() {
            if dev != device {
                return;
            }
            match Self::list(path) {
                Ok(children) => {
                    for (child, child_stat) in children {
                        self.entry(&child, &child_stat, device, kind, part);
                    }
                }
                Err(_) => self.unreadable += 1,
            }
            return;
        }
        if nlink > 1 && !self.seen.insert((dev, ino)) {
            return;
        }
        let allocated = if blocks > 0 { blocks } else { stat.len() };
        part.total.add(Total {
            files: 1,
            bytes: stat.len(),
            allocated,
        });
        part.newest = part.newest.max(mtime(stat) as i64);
        let start = week_start((created(stat) / DAY as f64).floor() as i64);
        if start >= self.first_week {
            self.weeks.entry(start).or_default()[kind] += allocated;
        }
    }

    fn root(&mut self, home: &Path, spec: &(&str, &str, &[&str], Rules)) -> (Value, [Total; 5]) {
        let (id, name, rel, rules) = *spec;
        let mut classes = [Total::default(); 5];
        let mut view = json!({"id": id, "name": name, "path": format!("~/{}", rel.join("/")), "state": "ok",
                              "files": 0, "bytes": 0, "allocated": 0, "newest": null,
                              "classes": CLASSES.iter().map(|k| (k.to_string(), Total::default().view())).collect::<serde_json::Map<_, _>>(),
                              "parts": [], "unreadable": 0});
        let path = rel.iter().fold(home.to_path_buf(), |p, part| p.join(part));
        let top = match fs::metadata(&path) {
            Ok(top) if top.is_dir() => top,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                view["state"] = json!("missing");
                return (view, classes);
            }
            _ => {
                view["state"] = json!("unreadable");
                return (view, classes);
            }
        };
        let Ok(children) = Self::list(&path) else {
            view["state"] = json!("unreadable");
            return (view, classes);
        };
        let device = identity(&top).0;
        let before = self.unreadable;
        let (mut total, mut newest) = (Total::default(), 0);
        let mut keyed: Vec<(String, &str, Part)> = Vec::new();
        for (child, stat) in children {
            let child_name = child
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let (key, kind) = classify(rules, &child_name);
            let index = CLASSES.iter().position(|k| *k == kind).unwrap_or(4);
            let slot = match keyed.iter().position(|p| p.0 == key) {
                Some(slot) => slot,
                None => {
                    keyed.push((
                        key,
                        kind,
                        Part {
                            total: Total::default(),
                            newest: 0,
                        },
                    ));
                    keyed.len() - 1
                }
            };
            self.entry(&child, &stat, device, index, &mut keyed[slot].2);
        }
        let mut parts = Vec::new();
        for (key, kind, part) in keyed {
            let index = CLASSES.iter().position(|k| *k == kind).unwrap_or(4);
            total.add(part.total);
            classes[index].add(part.total);
            newest = newest.max(part.newest);
            parts.push((part.total.allocated, key, kind, part));
        }
        parts.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        view["files"] = json!(total.files);
        view["bytes"] = json!(total.bytes);
        view["allocated"] = json!(total.allocated);
        view["newest"] = iso(newest);
        view["unreadable"] = json!(self.unreadable - before);
        for (i, kind) in CLASSES.iter().enumerate() {
            view["classes"][*kind] = classes[i].view();
        }
        view["parts"] = parts
            .into_iter()
            .map(|(_, name, kind, part)| {
                json!({"name": name, "class": kind, "files": part.total.files, "bytes": part.total.bytes,
                       "allocated": part.total.allocated, "newest": iso(part.newest)})
            })
            .collect();
        (view, classes)
    }
}

/// The agents' folders under `home` that exist, as `(id, name, path)`, without measuring them (F7b: the Agentes
/// collection in `data_catalog`).
pub(crate) fn roots(home: &Path) -> Vec<(&'static str, &'static str, PathBuf)> {
    ROOTS
        .iter()
        .map(|(id, name, parts, _)| {
            (
                *id,
                *name,
                parts
                    .iter()
                    .fold(home.to_path_buf(), |p, part| p.join(part)),
            )
        })
        .filter(|(_, _, path)| path.is_dir())
        .collect()
}

/// The `agents` collection under `home`, measured at `now` (seconds since the epoch).
pub fn collections(home: &Path, now: f64) -> Value {
    let this_week = week_start((now / DAY as f64).floor() as i64);
    let first_week = this_week - 7 * WEEKS;
    let mut walk = Walk {
        seen: HashSet::new(),
        unreadable: 0,
        first_week,
        weeks: HashMap::new(),
    };
    let mut classes = [Total::default(); 5];
    let mut roots = Vec::new();
    for spec in &ROOTS {
        let (view, totals) = walk.root(home, spec);
        for (i, total) in totals.iter().enumerate() {
            classes[i].add(*total);
        }
        roots.push(view);
    }
    let mut weeks = Vec::new();
    let mut start = first_week;
    while start <= this_week {
        let row = walk.weeks.get(&start).copied().unwrap_or_default();
        weeks.push((start < this_week, row, json!({
            "start": date(start), "complete": start < this_week, "allocated": row.iter().sum::<u64>(),
            "classes": CLASSES.iter().zip(row).map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
        })));
        start += 7;
    }
    let full: Vec<_> = weeks.iter().filter(|w| w.0).map(|w| w.1).collect();
    let full = &full[full.len().saturating_sub(4)..];
    let count = full.len().max(1) as u64;
    let mut total = Total::default();
    for class in classes {
        total.add(class);
    }
    let agents = json!({
        "id": "agents", "name": "Agentes", "basis": "metadata", "measured_at": iso(now as i64),
        "files": total.files, "bytes": total.bytes, "allocated": total.allocated,
        "classes": CLASSES.iter().zip(classes).map(|(k, v)| (k.to_string(), v.view())).collect::<serde_json::Map<_, _>>(),
        "roots": roots,
        "growth": {"basis": if cfg!(target_os = "macos") { "created" } else { "mtime" }, "weeks": weeks.into_iter().map(|w| w.2).collect::<Vec<_>>(),
                   "weekly_average": full.iter().map(|r| r.iter().sum::<u64>()).sum::<u64>() / count,
                   "history_weekly_average": full.iter().map(|r| r[0]).sum::<u64>() / count},
        "release": {"automatic": false, "candidates": classes[2].allocated, "needs": "reviewed_plan"},
    });
    json!({"collections": [agents]})
}

/// The `storage_collections` method: the home folder is `HOME` (`USERPROFILE` on Windows), as Python's
/// `os.path.expanduser('~')`.
pub fn call(params: &Value) -> Result<Value> {
    require(
        params.as_object().is_none_or(|p| p.is_empty()),
        "invalid_params",
        "storage_collections accepts an empty params object.",
    )?;
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    Ok(collections(&home, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_match_like_fnmatchcase() {
        assert!(glob(
            "thread_history*.sqlite*",
            "thread_history_1.sqlite-wal"
        ));
        assert!(glob("Dawn*Cache", "DawnGraphiteCache"));
        assert!(!glob("*.md", "notes.mdx"));
        assert!(!glob("Cache", "cache"));
        assert_eq!(
            classify(ROOTS[1].3, "logs_2.sqlite"),
            ("logs_*.sqlite*".into(), "logs")
        );
        assert_eq!(
            classify(ROOTS[1].3, "sessions"),
            ("sessions".into(), "history")
        );
        assert_eq!(
            classify(ROOTS[0].3, "unknown-dir"),
            ("unknown-dir".into(), "other")
        );
    }

    #[test]
    fn dates_are_civil_utc() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(week_start(0)), "1969-12-29");
        assert_eq!(iso(1_763_510_400), json!("2025-11-19T00:00:00Z"));
    }
}
