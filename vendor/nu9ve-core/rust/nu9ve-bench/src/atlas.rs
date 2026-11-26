//! Atlas's own benchmark formats as nu9ve-bench-v1 runs (contract §6.1 and §10), read-only.
//!
//! The same translation as `scripts/bench-from-atlas.py`, with the same `run_id`s (tests/atlas.rs compares
//! both on `data/bench-fixtures/atlas`):
//!
//! - `derived/bench/<stage>/<dataset>/*.json` and `config/bench.json#baselines`: one run each. A run file that
//!   imported a baseline replaces it (same stage, dataset, engine and `time_s`).
//! - `analysis/history/evolucion.json`: one quality-only run per milestone (§6.1), suite `atlas-quality`,
//!   dated by the milestone, else by its iteration in `docs/log/*.md`, else by the file's modification day.
//! - `derived/bench/*.ndjson`: logs Atlas already writes in the suite format. Their runs come first and the
//!   first run of each `run_id` wins (§1), so they replace a translation of the same origin.
//!
//! Nothing is written under `root`. Where the script exits on a bad file, this reports it in
//! [`Translated::problems`] and goes on. One difference: the order of the quality metrics flattened from an
//! Atlas object follows the JSON parser (sorted keys unless serde_json's `preserve_order` is on).

use crate::{CONTRACT, hex_ok, imported_run_id, read_log, run_problem};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Atlas `memory_gb` and `peak_memory_gb`: `hw.memsize / 2**30`.
const GIB: f64 = 1_073_741_824.0;
/// Atlas `disk_gb`: bytes / 1e9.
const GB: f64 = 1e9;
const MAX_NOTE: usize = 500;
/// The largest Atlas file read; a bigger one is reported as `too_large`.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const QUALITY_KEYS: [&str; 7] = [
    "auc",
    "auc_candidates",
    "auc_flat",
    "auc_curb",
    "rho_bombs",
    "judged",
    "adopted",
];

/// What [`translate`] reads from an Atlas repo.
#[derive(Debug, Clone, Default)]
pub struct Translated {
    /// Valid runs, the first of each `run_id`: Atlas's own logs, then run files, baselines and milestones.
    pub runs: Vec<Value>,
    /// What was skipped: where (relative to the root, with `:line` for a log) and why.
    pub problems: Vec<(String, String)>,
    /// Every file read, for lineage.
    pub files: Vec<PathBuf>,
}

impl Translated {
    fn problem(&mut self, place: impl Into<String>, reason: impl Into<String>) {
        self.problems.push((place.into(), reason.into()));
    }

    /// The text of a file under the size cap, remembered in `files`; a problem otherwise.
    fn read(&mut self, root: &Path, path: &Path) -> Option<String> {
        let place = relative(root, path);
        match fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_FILE_BYTES => {
                self.problem(place, "too_large");
                return None;
            }
            Ok(_) => {}
            Err(_) => {
                self.problem(place, "unreadable");
                return None;
            }
        }
        match fs::read_to_string(path) {
            Ok(text) => {
                self.files.push(path.to_path_buf());
                Some(text)
            }
            Err(_) => {
                self.problem(place, "unreadable");
                None
            }
        }
    }

    fn read_json(&mut self, root: &Path, path: &Path) -> Option<Value> {
        let text = self.read(root, path)?;
        match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(_) => {
                self.problem(relative(root, path), "not_json");
                None
            }
        }
    }
}

fn relative(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

// ------------------------------------------------------------------------------------------- helpers

/// Python's `str()` of a JSON value, as the script spells names and messages.
fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        other => other.to_string(),
    }
}

/// Python's truthiness of a JSON value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Python's `==`: numbers by value (`14295 == 14295.0`), anything else structurally.
fn py_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// A dataset, stage or engine of Atlas as a part of `case` (§2).
fn slug_text(text: &str) -> String {
    let out: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = out.trim_matches(|c| c == '-' || c == '.');
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn slug(value: &Value) -> String {
    slug_text(&py_str(value))
}

/// The key of a milestone as the last part of `case`; `+` is spelled out so `auto-v6+` is not `auto-v6`.
fn milestone_slug(key: &str) -> String {
    slug_text(&key.replace('+', "-plus"))
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_NOTE {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(MAX_NOTE - 1).collect();
        out.push('…');
        out
    }
}

/// `e702ac2+dirty` → (`"e702ac2"`, `true`); `e702ac2` → (`"e702ac2"`, `false`); anything else → nulls.
fn commit_of(git: Option<&Value>) -> (Value, Value) {
    match git.and_then(Value::as_str) {
        Some(text) if !text.is_empty() => {
            let (commit, flag) = text.split_once('+').unwrap_or((text, ""));
            if hex_ok(commit, 7, 40) {
                (json!(commit), json!(flag == "dirty"))
            } else {
                (Value::Null, Value::Null)
            }
        }
        _ => (Value::Null, Value::Null),
    }
}

/// `value · factor` like Python: an integer stays an integer; a float is a float.
fn scaled(value: &Value, factor: i64) -> Value {
    if let Some(n) = value.as_i64().and_then(|n| n.checked_mul(factor)) {
        json!(n)
    } else {
        json!(value.as_f64().unwrap_or(f64::NAN) * factor as f64)
    }
}

/// Python's `round(value · factor)`: ties to even.
fn rounded(value: &Value, factor: f64) -> Value {
    match value.as_i64() {
        Some(n) if factor.fract() == 0.0 => json!(n.saturating_mul(factor as i64)),
        _ => json!((value.as_f64().unwrap_or(f64::NAN) * factor).round_ties_even() as i64),
    }
}

/// Python's `int(value · factor)`: truncated.
fn truncated(value: &Value, factor: f64) -> Value {
    match value.as_i64() {
        Some(n) if factor.fract() == 0.0 => json!(n.saturating_mul(factor as i64)),
        _ => json!((value.as_f64().unwrap_or(f64::NAN) * factor).trunc() as i64),
    }
}

/// Atlas quality as one metric per numeric or boolean leaf, with dotted names.
fn flatten(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(item, &name, out);
            }
        }
        Value::Bool(_) | Value::Number(_) => out.push((prefix.to_owned(), value.clone())),
        _ => {}
    }
}

fn unknown_machine() -> Value {
    json!({"model": null, "chip": null, "cpu_cores": null, "memory_bytes": null, "os": null, "arch": null})
}

// ------------------------------------------------------------------------------------------- run files and baselines

fn to_run(entry: &Map<String, Value>, origin: &str, baseline: bool) -> Value {
    let get = |key: &str| entry.get(key).unwrap_or(&Value::Null);
    let (commit, dirty) = commit_of(entry.get("git"));
    let empty = Map::new();
    let atlas_machine = entry
        .get("machine")
        .and_then(Value::as_object)
        .unwrap_or(&empty);

    let mut params = Map::new();
    params.insert("dataset".into(), get("dataset").clone());
    if let Some(own) = entry.get("params").and_then(Value::as_object) {
        params.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    if let Some(docker) = atlas_machine.get("docker_memory_gb") {
        params.insert("docker_memory_gb".into(), docker.clone());
    }
    if truthy(get("tag")) {
        params.insert("tag".into(), get("tag").clone());
    }
    let memory_bytes = match atlas_machine.get("memory_gb") {
        Some(gb) if gb.is_number() => truncated(gb, GIB),
        _ => Value::Null,
    };

    let mut run = Map::new();
    run.insert("contract".into(), json!(CONTRACT));
    run.insert("run_id".into(), json!(imported_run_id(origin)));
    run.insert("at".into(), get("date").clone());
    run.insert("suite".into(), json!("atlas-pipeline"));
    run.insert(
        "case".into(),
        json!(format!("{}/{}", slug(get("stage")), slug(get("dataset")))),
    );
    run.insert("category".into(), json!(slug(get("stage"))));
    run.insert(
        "subject".into(),
        json!({"app": "atlas", "engine": slug(get("engine")), "version": null, "commit": commit, "dirty": dirty}),
    );
    run.insert(
        "machine".into(),
        json!({
            "model": null,
            "chip": atlas_machine.get("chip").cloned().unwrap_or(Value::Null),
            "cpu_cores": atlas_machine.get("cores").cloned().unwrap_or(Value::Null),
            "memory_bytes": memory_bytes,
            "os": atlas_machine.get("os").cloned().unwrap_or(Value::Null),
            "arch": null,
        }),
    );
    run.insert("inputs".into(), json!([]));
    run.insert("params".into(), Value::Object(params));
    run.insert("cache".into(), json!("unknown"));

    let status = entry.get("status").cloned().unwrap_or_else(|| json!("ok"));
    let time_s = get("time_s");
    if status == "ok" && time_s.is_number() {
        let wall = scaled(time_s, 1000);
        let mut metrics = Map::new();
        metrics.insert(
            "wall_ms".into(),
            json!({"samples": [wall], "median": wall, "p95": wall, "min": wall, "max": wall}),
        );
        let peak = get("peak_memory_gb");
        if peak.is_number() {
            let measure = get("memory_measure")
                .as_str()
                .filter(|m| matches!(*m, "footprint" | "rss"))
                .unwrap_or("unknown");
            metrics.insert(
                "peak_memory".into(),
                json!({"bytes": rounded(peak, GIB), "measure": measure, "scope": "tree"}),
            );
        }
        let disk = get("disk_gb");
        if disk.is_number() {
            metrics.insert(
                "temp_disk_bytes".into(),
                json!({"bytes": rounded(disk, GB)}),
            );
        }
        run.insert("repeats".into(), json!(1));
        run.insert("warmup".into(), json!(0));
        run.insert("status".into(), json!("ok"));
        run.insert("metrics".into(), Value::Object(metrics));
    } else if status == "ok" {
        run.insert("repeats".into(), json!(0));
        run.insert("warmup".into(), json!(0));
        run.insert("status".into(), json!("skipped"));
    } else {
        run.insert("repeats".into(), json!(0));
        run.insert("warmup".into(), json!(0));
        run.insert("status".into(), json!("failed"));
        run.insert(
            "error".into(),
            json!({"code": format!("atlas_{}", slug(&status)), "message": format!("Atlas status: {}", py_str(&status))}),
        );
    }

    let mut leaves = Vec::new();
    if let Some(quality) = entry.get("quality") {
        flatten(quality, "", &mut leaves);
    }
    let quality: Vec<Value> = leaves
        .into_iter()
        .filter(|(name, _)| (1..=128).contains(&name.chars().count()))
        .map(|(name, value)| json!({"metric": name, "value": value, "reference": null, "pass": null}))
        .collect();
    if !quality.is_empty() {
        run.insert("quality".into(), Value::Array(quality));
    }
    // Atlas photos are the person's; only numbers travel, but not as public.
    run.insert("privacy".into(), json!("restricted"));
    run.insert(
        "source".into(),
        json!({"kind": "imported", "origin": origin}),
    );
    let what = if baseline {
        "baseline".to_owned()
    } else if truthy(get("source")) {
        py_str(get("source"))
    } else {
        "run".to_owned()
    };
    let mut note = format!("atlas: {what}");
    if truthy(get("notes")) {
        note.push_str(". ");
        note.push_str(&py_str(get("notes")));
    }
    run.insert("note".into(), json!(shorten(&note)));
    Value::Object(run)
}

/// Covered baselines: stage, dataset, engine and `time_s` of a run file that imported one.
type Key = [Value; 4];

fn key_of(entry: &Map<String, Value>) -> Key {
    ["stage", "dataset", "engine", "time_s"].map(|k| entry.get(k).cloned().unwrap_or(Value::Null))
}

fn covered_by(covered: &[Key], key: &Key) -> bool {
    covered
        .iter()
        .any(|c| c.iter().zip(key).all(|(a, b)| py_eq(a, b)))
}

/// Entries of `dir` that pass `keep`, sorted by name.
fn listed(dir: &Path, keep: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| keep(p))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn ends_with(path: &Path, suffix: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(suffix))
}

/// `derived/bench/*/*/*.json`, in the order of their path components (like Python's sorted paths).
fn run_files(bench: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for stage in listed(bench, |p| p.is_dir()) {
        for dataset in listed(&stage, |p| p.is_dir()) {
            files.extend(listed(&dataset, |p| p.is_file() && ends_with(p, ".json")));
        }
    }
    files.sort_by_key(|p| {
        p.components()
            .map(|c| c.as_os_str().to_owned())
            .collect::<Vec<_>>()
    });
    files
}

fn pipeline_runs(root: &Path, out: &mut Translated) -> Vec<(String, Value)> {
    let mut runs = Vec::new();
    let mut covered: Vec<Key> = Vec::new();
    for path in run_files(&root.join("derived").join("bench")) {
        let Some(value) = out.read_json(root, &path) else {
            continue;
        };
        let Some(entry) = value.as_object() else {
            continue;
        };
        if !["stage", "dataset", "engine", "date"]
            .iter()
            .all(|k| entry.contains_key(*k))
        {
            continue;
        }
        let place = relative(root, &path);
        runs.push((
            place.clone(),
            to_run(entry, &format!("atlas {place}"), false),
        ));
        if py_str(entry.get("source").unwrap_or(&json!(""))).starts_with("imported") {
            covered.push(key_of(entry));
        }
    }
    let config = root.join("config").join("bench.json");
    if config.is_file()
        && let Some(value) = out.read_json(root, &config)
    {
        let baselines = value.get("baselines").and_then(Value::as_array);
        for (index, entry) in baselines.into_iter().flatten().enumerate() {
            let place = format!("config/bench.json#baselines/{index}");
            let Some(entry) = entry.as_object() else {
                out.problem(place, "not_object");
                continue;
            };
            if covered_by(&covered, &key_of(entry)) {
                continue;
            }
            if !["stage", "dataset", "engine", "date"]
                .iter()
                .all(|k| entry.contains_key(*k))
            {
                out.problem(place, "missing_field");
                continue;
            }
            runs.push((
                place.clone(),
                to_run(entry, &format!("atlas {place}"), true),
            ));
        }
    }
    runs
}

// ------------------------------------------------------------------------------------------- milestones

/// `## Iteración <n> · <AAAA-MM-DD>` at the start of a line, the date not followed by a word character.
fn iteration_heading(line: &str) -> Option<(u64, String)> {
    let rest = line.strip_prefix("## Iteración ")?;
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let number: u64 = rest[..digits_end].parse().ok()?;
    let rest = rest[digits_end..].strip_prefix(" · ")?;
    let day = rest.get(..10)?;
    if !crate::day_ok(day) {
        return None;
    }
    let boundary = rest[10..]
        .chars()
        .next()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    boundary.then(|| (number, day.to_owned()))
}

/// Iteration number → day, from the headings of Atlas's log; the first heading of each number wins.
fn log_dates(root: &Path, out: &mut Translated) -> BTreeMap<u64, String> {
    let mut dates = BTreeMap::new();
    let dir = root.join("docs").join("log");
    for path in listed(&dir, |p| p.is_file() && ends_with(p, ".md")) {
        let Some(text) = out.read(root, &path) else {
            continue;
        };
        for line in text.lines() {
            if let Some((number, day)) = iteration_heading(line) {
                dates.entry(number).or_insert(day);
            }
        }
    }
    dates
}

fn modified_day(path: &Path) -> Option<String> {
    let seconds = fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(crate::time::day_utc(seconds))
}

fn milestone_run(kind: &str, entry: &Map<String, Value>, key: &str, at: &str) -> Option<Value> {
    let origin = format!("atlas analysis/history/evolucion.json#{kind}/{key}");
    let version = match entry.get("label") {
        Some(Value::String(label)) if !label.is_empty() => json!(label),
        _ => Value::Null,
    };
    let quality: Vec<Value> = QUALITY_KEYS
        .iter()
        .filter_map(|name| {
            let value = entry.get(*name)?;
            (value.is_boolean() || value.is_number())
                .then(|| json!({"metric": name, "value": value, "reference": null, "pass": null}))
        })
        .collect();
    if quality.is_empty() {
        return None; // a milestone with every metric null measured nothing
    }
    let mut run = json!({
        "contract": CONTRACT,
        "run_id": imported_run_id(&origin),
        "at": at,
        "suite": "atlas-quality",
        "case": format!("{kind}/{}", milestone_slug(key)),
        "category": "quality",
        "subject": {"app": "atlas", "engine": "model", "version": version, "commit": null, "dirty": null},
        "machine": unknown_machine(),
        "inputs": [],
        "params": {"iteration": entry.get("iteration").cloned().unwrap_or(Value::Null)},
        "cache": "unknown",
        "repeats": 0,
        "warmup": 0,
        "status": "ok",
        "quality": quality,
        "privacy": "restricted",
        "source": {"kind": "imported", "origin": origin},
    });
    if let Some(what) = entry.get("what").and_then(Value::as_str)
        && !what.is_empty()
    {
        run["note"] = json!(shorten(what));
    }
    Some(run)
}

fn iteration_of(entry: &Map<String, Value>) -> Option<u64> {
    let value = entry.get("iteration")?;
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|f| f.fract() == 0.0 && *f >= 0.0)
            .map(|f| f as u64)
    })
}

fn milestone_runs(root: &Path, out: &mut Translated) -> Vec<(String, Value)> {
    let path = root.join("analysis").join("history").join("evolucion.json");
    if !path.is_file() {
        return Vec::new();
    }
    let Some(history) = out.read_json(root, &path) else {
        return Vec::new();
    };
    let dates = log_dates(root, out);
    let modified = modified_day(&path);
    let mut runs = Vec::new();
    for kind in ["evidence", "gravity"] {
        let entries = history.get(kind).and_then(Value::as_array);
        for entry in entries.into_iter().flatten() {
            let Some(entry) = entry.as_object() else {
                continue;
            };
            let Some(key) = entry.get("key").and_then(Value::as_str) else {
                continue;
            };
            let own = entry
                .get("date")
                .and_then(Value::as_str)
                .filter(|d| crate::day_ok(d));
            let at = own
                .map(str::to_owned)
                .or_else(|| iteration_of(entry).and_then(|n| dates.get(&n).cloned()))
                .or_else(|| modified.clone());
            let place = format!("analysis/history/evolucion.json#{kind}/{key}");
            let Some(at) = at else {
                out.problem(place, "no_date");
                continue;
            };
            if let Some(run) = milestone_run(kind, entry, key, &at) {
                runs.push((place, run));
            }
        }
    }
    runs
}

// ------------------------------------------------------------------------------------------- the repo

/// `derived/bench/*.ndjson`, read as logs (§1).
fn suite_logs(root: &Path, out: &mut Translated) -> Vec<Value> {
    let mut runs = Vec::new();
    let dir = root.join("derived").join("bench");
    for path in listed(&dir, |p| p.is_file() && ends_with(p, ".ndjson")) {
        let Some(text) = out.read(root, &path) else {
            continue;
        };
        let log = read_log(&text);
        let place = relative(root, &path);
        for problem in log.problems {
            out.problem(format!("{place}:{}", problem.line), problem.problem);
        }
        runs.extend(log.runs);
    }
    runs
}

/// Translates the Atlas repo at `root`, read-only. A missing piece is simply absent; a file that cannot be
/// read or a run that would be invalid is reported in `problems`.
pub fn translate(root: &Path) -> Translated {
    let mut out = Translated::default();
    let written = suite_logs(root, &mut out);
    let mut translated = pipeline_runs(root, &mut out);
    translated.extend(milestone_runs(root, &mut out));
    let mut seen = HashSet::new();
    let mut runs = Vec::new();
    for run in written {
        if seen.insert(run["run_id"].as_str().unwrap_or_default().to_owned()) {
            runs.push(run);
        }
    }
    for (place, run) in translated {
        if let Some(problem) = run_problem(&run) {
            out.problem(place, problem);
            continue;
        }
        if seen.insert(run["run_id"].as_str().unwrap_or_default().to_owned()) {
            runs.push(run);
        }
    }
    out.runs = runs;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_like_the_script() {
        assert_eq!(slug(&json!("Day 45m/All")), "day-45m-all");
        assert_eq!(slug(&json!("--")), "unknown");
        assert_eq!(slug(&Value::Null), "none");
        assert_eq!(milestone_slug("auto-v6+"), "auto-v6-plus");
        assert_eq!(milestone_slug("A2b"), "a2b");
    }

    #[test]
    fn reads_commits_and_numbers_like_python() {
        assert_eq!(
            commit_of(Some(&json!("e702ac2+dirty"))),
            (json!("e702ac2"), json!(true))
        );
        assert_eq!(commit_of(Some(&json!("main"))), (Value::Null, Value::Null));
        assert_eq!(scaled(&json!(1260), 1000), json!(1_260_000));
        assert_eq!(rounded(&json!(0.5), 1.0), json!(0));
        assert_eq!(rounded(&json!(1.5), 1.0), json!(2));
        assert_eq!(truncated(&json!(16), GIB), json!(17_179_869_184_i64));
    }

    #[test]
    fn finds_iteration_headings() {
        assert_eq!(
            iteration_heading("## Iteración 39 · 2026-11-21: tus juicios"),
            Some((39, "2026-11-21".to_owned()))
        );
        assert_eq!(iteration_heading("## Iteración 39 · 2026-11-211"), None);
        assert_eq!(iteration_heading("### Iteración 39 · 2026-11-21"), None);
    }
}
