//! nu9ve-bench-v1 (contracts/nu9ve-bench-v1.md): one benchmark run per line of an append-only NDJSON log.
//!
//! - [`run_problem`] and [`line_problem`] validate a run with the same codes as the Python reference
//!   (`scripts/nu9ve_bench.py`), checked against `data/bench-examples-v1.json`.
//! - [`read_log`] reads a log the way §1 asks; [`append`] writes one under its lock.
//! - [`compare`] is §8; [`distribution`] the §5 summary.
//! - [`atlas`] translates Atlas's earlier formats, read-only (§10).
//! - [`measure`] has the helpers to time a function or a process, run a soak and describe the machine.
//! - [`memory_slope`] is the leak signal of a soak run (§5.1, minor version of 2026-11-25).

pub mod atlas;
mod log;
pub mod measure;
mod time;

pub use log::{Log, LogProblem, append, read_log};

use serde_json::{Map, Value, json};

pub const CONTRACT: &str = "nu9ve-bench/1";
pub const MAX_LINE: usize = 64 * 1024;
const MAX_SAFE: u64 = (1 << 53) - 1;
const DISTRIBUTIONS: [&str; 3] = ["wall_ms", "cpu_ms", "startup_ms"];
const METRICS: [&str; 7] = [
    "wall_ms",
    "cpu_ms",
    "startup_ms",
    "peak_memory",
    "temp_disk_bytes",
    "throughput",
    "resources",
];
/// §5.1: at most this many samples in each series of `resources`.
pub const MAX_RESOURCE_SAMPLES: usize = 10_000;
/// §5.2: the quality metric of a soak with the memory growth per iteration, and its companion.
pub const SOAK_SLOPE: &str = "memory_slope_bytes_per_iter";
pub const SOAK_GROWTH: &str = "rss_end_minus_start_bytes";

// ------------------------------------------------------------------------------------------- patterns

fn lower_alnum(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// `^[a-z][a-z0-9-]{0,max-1}$`
fn name_ok(text: &str, max: usize) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && text.len() <= max
        && chars.all(|c| lower_alnum(c) || c == '-')
}

/// `^[a-z0-9][a-z0-9._-]*(/[a-z0-9][a-z0-9._-]*)*$`
fn case_ok(text: &str) -> bool {
    text.split('/').all(|part| {
        let mut chars = part.chars();
        matches!(chars.next(), Some(c) if lower_alnum(c))
            && chars.all(|c| lower_alnum(c) || matches!(c, '.' | '_' | '-'))
    })
}

fn hex_ok(text: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

/// `^[0-9]{4}-[0-9]{2}-[0-9]{2}$`
fn day_ok(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 3
        && [4, 2, 2]
            .iter()
            .zip(&parts)
            .all(|(n, p)| p.len() == *n && digits(p))
}

/// `^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]{1,9})?Z$`
fn utc_ok(text: &str) -> bool {
    let Some(rest) = text.strip_suffix('Z') else {
        return false;
    };
    let Some((day, clock)) = rest.split_once('T') else {
        return false;
    };
    let (clock, fraction) = match clock.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (clock, None),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    day_ok(day)
        && parts.len() == 3
        && parts.iter().all(|p| p.len() == 2 && digits(p))
        && fraction.is_none_or(|f| (1..=9).contains(&f.len()) && digits(f))
}

/// No control characters and no whitespace but the plain space (`printable` in the reference).
fn printable(text: &str) -> bool {
    text.chars().all(|c| {
        c == ' ' || (c >= ' ' && !('\u{7f}'..='\u{9f}').contains(&c) && !c.is_whitespace())
    })
}

fn chars_between(text: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&text.chars().count())
}

/// §9: a string that starts like a path of the person, or carries a Windows path.
fn leaks_path(text: &str) -> bool {
    if text.starts_with('/') || text.starts_with('~') || text.contains("\\\\") {
        return true;
    }
    let bytes = text.as_bytes();
    bytes
        .windows(3)
        .any(|w| w[0].is_ascii_alphabetic() && w[1] == b':' && w[2] == b'\\')
}

// ------------------------------------------------------------------------------------------- values

fn is_int(value: &Value) -> bool {
    value.is_i64() || value.is_u64()
}

fn is_count(value: &Value) -> bool {
    value.as_u64().is_some_and(|n| n <= MAX_SAFE)
}

fn is_number(value: &Value) -> bool {
    match value {
        Value::Number(n) => n
            .as_u64()
            .map(|u| u <= MAX_SAFE)
            .or_else(|| n.as_i64().map(|i| i.unsigned_abs() <= MAX_SAFE))
            .unwrap_or_else(|| n.as_f64().is_some_and(f64::is_finite)),
        _ => false,
    }
}

fn numbers_ok(value: &Value) -> bool {
    match value {
        Value::Number(_) => is_number(value),
        Value::Array(items) => items.iter().all(numbers_ok),
        Value::Object(map) => map.values().all(numbers_ok),
        _ => true,
    }
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

fn nullable(value: Option<&Value>, check: impl Fn(&Value) -> bool) -> bool {
    match value {
        None => false,
        Some(Value::Null) => true,
        Some(v) => check(v),
    }
}

fn str_with(value: &Value, check: impl Fn(&str) -> bool) -> bool {
    value.as_str().is_some_and(check)
}

fn strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(s) => out.push(s),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => {
            for (key, item) in map {
                out.push(key);
                strings(item, out);
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------------------------------------------- §5

fn sorted(samples: &[f64]) -> Vec<f64> {
    let mut ordered = samples.to_vec();
    ordered.sort_by(f64::total_cmp);
    ordered
}

/// §5: the summary of `samples` (at least one). Odd count: the middle one; even: `(a + b) / 2`.
pub fn median(samples: &[f64]) -> f64 {
    let ordered = sorted(samples);
    let n = ordered.len();
    if n % 2 == 1 {
        ordered[n / 2]
    } else {
        (ordered[n / 2 - 1] + ordered[n / 2]) / 2.0
    }
}

/// §5: nearest rank, `k = (95 · n + 99) div 100` counted from 1.
pub fn p95(samples: &[f64]) -> f64 {
    let ordered = sorted(samples);
    let k = (95 * ordered.len()).div_ceil(100);
    ordered[k - 1]
}

/// §5: `{"samples", "median", "p95", "min", "max"}`. Panics on an empty slice.
pub fn distribution(samples: &[f64]) -> Value {
    assert!(
        !samples.is_empty(),
        "a distribution needs at least one sample"
    );
    let ordered = sorted(samples);
    json!({
        "samples": samples,
        "median": median(samples),
        "p95": p95(samples),
        "min": ordered[0],
        "max": ordered[ordered.len() - 1],
    })
}

fn distribution_problem(value: &Value, repeats: u64) -> Option<&'static str> {
    let Some(samples) = value.get("samples").and_then(Value::as_array) else {
        return Some("bad_metrics");
    };
    if !samples.iter().all(|s| is_number(s) && number(s) >= 0.0) {
        return Some("bad_metrics");
    }
    if samples.len() as u64 != repeats || samples.is_empty() {
        return Some("samples_mismatch");
    }
    let keys = ["median", "p95", "min", "max"];
    if !keys.iter().all(|k| value.get(k).is_some_and(is_number)) {
        return Some("bad_metrics");
    }
    let values: Vec<f64> = samples.iter().map(number).collect();
    let expected = distribution(&values);
    if keys
        .iter()
        .any(|k| number(&value[k]) != number(&expected[k]))
    {
        return Some("summary_mismatch");
    }
    None
}

/// §5.1: `{"interval_ms", "rss_bytes", "cpu_ms"}`, integers, the same number of samples (1 to 10 000) in both
/// series, and the CPU time never going back.
fn resources_ok(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    let interval_ok = map
        .get("interval_ms")
        .is_some_and(|i| is_count(i) && i.as_u64().is_some_and(|i| i >= 1));
    let (Some(rss), Some(cpu)) = (
        map.get("rss_bytes").and_then(Value::as_array),
        map.get("cpu_ms").and_then(Value::as_array),
    ) else {
        return false;
    };
    map.len() == 3
        && interval_ok
        && (1..=MAX_RESOURCE_SAMPLES).contains(&rss.len())
        && rss.len() == cpu.len()
        && rss.iter().all(is_count)
        && cpu.iter().all(is_count)
        && cpu.windows(2).all(|w| w[0].as_u64() <= w[1].as_u64())
}

fn metrics_problem(metrics: &Value, repeats: u64) -> Option<&'static str> {
    let Some(map) = metrics.as_object() else {
        return Some("bad_metrics");
    };
    if map.keys().any(|k| !METRICS.contains(&k.as_str())) {
        return Some("bad_metrics");
    }
    for key in DISTRIBUTIONS {
        if let Some(value) = map.get(key) {
            if !value.is_object() {
                return Some("bad_metrics");
            }
            if let Some(problem) = distribution_problem(value, repeats) {
                return Some(problem);
            }
        }
    }
    if let Some(memory) = map.get("peak_memory") {
        let ok = memory.get("bytes").is_some_and(is_count)
            && memory
                .get("measure")
                .and_then(Value::as_str)
                .is_some_and(|m| matches!(m, "rss" | "footprint" | "unknown"))
            && memory
                .get("scope")
                .and_then(Value::as_str)
                .is_some_and(|s| matches!(s, "process" | "tree"));
        if !memory.is_object() || !ok {
            return Some("bad_metrics");
        }
    }
    if let Some(disk) = map.get("temp_disk_bytes")
        && (!disk.is_object() || !disk.get("bytes").is_some_and(is_count))
    {
        return Some("bad_metrics");
    }
    if map.get("resources").is_some_and(|r| !resources_ok(r)) {
        return Some("bad_metrics");
    }
    if let Some(rate) = map.get("throughput") {
        if !rate.is_object() {
            return Some("bad_metrics");
        }
        let value_ok = rate
            .get("value")
            .is_some_and(|v| is_number(v) && number(v) > 0.0);
        let unit_ok = rate.get("unit").is_some_and(|u| {
            str_with(u, |u| (1..=32).contains(&u.chars().count()) && printable(u))
        });
        if !value_ok || !unit_ok {
            return Some("bad_metrics");
        }
    }
    None
}

// ------------------------------------------------------------------------------------------- §6

fn quality_problem(quality: &Value) -> Option<&'static str> {
    let Some(items) = quality.as_array() else {
        return Some("bad_quality");
    };
    for item in items {
        let Some(map) = item.as_object() else {
            return Some("bad_quality");
        };
        let metric_ok = map
            .get("metric")
            .is_some_and(|m| str_with(m, |m| chars_between(m, 1, 128) && printable(m)));
        if !metric_ok {
            return Some("bad_quality");
        }
        let value = map.get("value").unwrap_or(&Value::Null);
        if !(value.is_boolean() || is_number(value)) {
            return Some("bad_quality");
        }
        let reference_ok = nullable(map.get("reference"), |r| {
            str_with(r, |r| r.chars().count() <= 128 && case_ok(r))
        });
        if !reference_ok {
            return Some("bad_quality");
        }
        if !nullable(map.get("pass"), Value::is_boolean) {
            return Some("bad_quality");
        }
        if let Some(limit) = map.get("limit") {
            if limit.is_boolean() != value.is_boolean() || !(limit.is_boolean() || is_number(limit))
            {
                return Some("bad_quality");
            }
            let op = map.get("op").and_then(Value::as_str);
            let passed = match (op, value.as_bool(), limit.as_bool()) {
                (Some("eq"), Some(v), Some(l)) => v == l,
                (Some(_), Some(_), _) => return Some("bad_quality"),
                (Some("le"), None, None) => number(value) <= number(limit),
                (Some("ge"), None, None) => number(value) >= number(limit),
                (Some("eq"), None, None) => number(value) == number(limit),
                _ => return Some("bad_quality"),
            };
            if map.get("pass").and_then(Value::as_bool) != Some(passed) {
                return Some("quality_mismatch");
            }
        } else if map.contains_key("op") {
            return Some("bad_quality");
        }
    }
    None
}

// ------------------------------------------------------------------------------------------- §5.2

/// §5.2: the least-squares slope of `rss` on `iterations` (bytes per iteration), the leak signal of a soak.
/// `None` with fewer than two samples or when every sample is at the same iteration. The sums go in order, in
/// `f64`, exactly like the Python reference, so both give the same bits.
pub fn memory_slope(iterations: &[f64], rss: &[f64]) -> Option<f64> {
    let n = iterations.len();
    if n < 2 || rss.len() != n {
        return None;
    }
    let mean_x = iterations.iter().sum::<f64>() / n as f64;
    let mean_y = rss.iter().sum::<f64>() / n as f64;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, y) in iterations.iter().zip(rss) {
        sxy += (x - mean_x) * (y - mean_y);
        sxx += (x - mean_x) * (x - mean_x);
    }
    (sxx > 0.0).then(|| sxy / sxx)
}

/// §5.2: an `ok` soak carries `resources`, the slope with its `limit` and `op: le`, and the growth.
fn soak_problem(run: &Value) -> Option<&'static str> {
    if run["category"] != "soak" || run["status"] != "ok" {
        return None;
    }
    if run
        .get("metrics")
        .and_then(|m| m.get("resources"))
        .is_none()
    {
        return Some("bad_soak");
    }
    let items = run.get("quality").and_then(Value::as_array);
    let find = |name: &str| {
        items
            .into_iter()
            .flatten()
            .find(|q| q.get("metric").and_then(Value::as_str) == Some(name))
    };
    let slope_ok = find(SOAK_SLOPE).is_some_and(|q| {
        q.get("value").is_some_and(is_number)
            && q.get("limit").is_some_and(is_number)
            && q.get("op").and_then(Value::as_str) == Some("le")
    });
    let growth_ok = find(SOAK_GROWTH).is_some_and(|q| q.get("value").is_some_and(is_int));
    (!(slope_ok && growth_ok)).then_some("bad_soak")
}

// ------------------------------------------------------------------------------------------- §2 to §9

fn kind_ok(value: &Value, kind: char) -> bool {
    match kind {
        's' => value.is_string(),
        'o' => value.is_object(),
        'l' => value.is_array(),
        _ => is_int(value),
    }
}

/// §6.1 (minor version of 2026-11-24): an `ok` run of category `quality` that carries quality and may carry no
/// time at all, like a model milestone.
pub fn quality_only(run: &Value) -> bool {
    run.get("category").and_then(Value::as_str) == Some("quality")
        && run
            .get("quality")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
}

/// `None` when `run` follows the contract, otherwise the code of the first problem, in the same order as the
/// Python reference.
pub fn run_problem(run: &Value) -> Option<&'static str> {
    let Some(map) = run.as_object() else {
        return Some("not_object");
    };
    let required = [
        ("contract", 's'),
        ("run_id", 's'),
        ("at", 's'),
        ("suite", 's'),
        ("case", 's'),
        ("category", 's'),
        ("subject", 'o'),
        ("machine", 'o'),
        ("inputs", 'l'),
        ("cache", 's'),
        ("repeats", 'i'),
        ("warmup", 'i'),
        ("status", 's'),
        ("privacy", 's'),
        ("source", 'o'),
    ];
    for (name, kind) in required {
        match map.get(name) {
            None => return Some("missing_field"),
            Some(value) if !kind_ok(value, kind) => return Some("wrong_type"),
            Some(_) => {}
        }
    }
    if !numbers_ok(run) {
        return Some("bad_number");
    }
    let text = |key: &str| map[key].as_str().unwrap_or_default();
    if text("contract") != CONTRACT {
        return Some("unknown_contract");
    }
    if !hex_ok(text("run_id"), 32, 32) {
        return Some("bad_run_id");
    }
    if !(utc_ok(text("at")) || day_ok(text("at"))) {
        return Some("bad_time");
    }
    if !name_ok(text("suite"), 64) || !name_ok(text("category"), 64) {
        return Some("bad_name");
    }
    if text("case").chars().count() > 128 || !case_ok(text("case")) {
        return Some("bad_case");
    }

    let subject = &map["subject"];
    let subject_ok = subject
        .get("app")
        .is_some_and(|a| str_with(a, |a| name_ok(a, 32)))
        && subject
            .get("engine")
            .is_some_and(|e| str_with(e, |e| name_ok(e, 32)))
        && nullable(subject.get("version"), |v| {
            str_with(v, |v| chars_between(v, 1, 64))
        })
        && nullable(subject.get("commit"), |c| str_with(c, |c| hex_ok(c, 7, 40)))
        && nullable(subject.get("dirty"), Value::is_boolean);
    if !subject_ok {
        return Some("bad_subject");
    }

    let machine = &map["machine"];
    let short_text = |v: &Value| str_with(v, |s| chars_between(s, 1, 128));
    let machine_ok = ["model", "chip", "os"]
        .iter()
        .all(|k| nullable(machine.get(k), short_text))
        && nullable(machine.get("arch"), |a| {
            str_with(a, |a| a == "aarch64" || a == "x86_64")
        })
        && nullable(machine.get("cpu_cores"), |c| {
            is_int(c) && c.as_i64().unwrap_or(0) >= 1
        })
        && nullable(machine.get("memory_bytes"), |m| {
            is_count(m) && m.as_u64().unwrap_or(0) >= 1
        });
    if !machine_ok {
        return Some("bad_machine");
    }

    for item in map["inputs"].as_array().into_iter().flatten() {
        let ok = item
            .get("name")
            .is_some_and(|n| str_with(n, |n| n.chars().count() <= 128 && case_ok(n)))
            && item
                .get("sha256")
                .is_some_and(|h| str_with(h, |h| hex_ok(h, 64, 64)))
            && nullable(item.get("bytes"), is_count);
        if !ok {
            return Some("bad_inputs");
        }
    }
    if map.get("params").is_some_and(|p| !p.is_object()) {
        return Some("bad_params");
    }
    if !matches!(text("cache"), "cold" | "warm" | "unknown") {
        return Some("bad_cache");
    }
    if !is_count(&map["repeats"]) || !is_count(&map["warmup"]) {
        return Some("bad_count");
    }
    let repeats = map["repeats"].as_u64().unwrap_or_default();

    let status = text("status");
    if !matches!(status, "ok" | "failed" | "not_ported" | "skipped") {
        return Some("bad_status");
    }
    let metrics = map.get("metrics");
    if status == "ok"
        && !quality_only(run)
        && !metrics.is_some_and(|m| m.get("wall_ms").is_some() && m.is_object())
    {
        return Some("missing_metrics");
    }
    if matches!(status, "not_ported" | "skipped") && (metrics.is_some() || repeats != 0) {
        return Some("unexpected_metrics");
    }
    if let Some(problem) = metrics.and_then(|m| metrics_problem(m, repeats)) {
        return Some(problem);
    }
    if status == "failed" {
        let error = map.get("error");
        let ok = error
            .and_then(|e| e.get("code"))
            .is_some_and(|c| str_with(c, |c| !c.is_empty()))
            && error
                .and_then(|e| e.get("message"))
                .is_some_and(Value::is_string);
        if !ok {
            return Some("missing_error");
        }
    }
    if let Some(problem) = map.get("quality").and_then(quality_problem) {
        return Some(problem);
    }
    if let Some(problem) = soak_problem(run) {
        return Some(problem);
    }

    if !matches!(text("privacy"), "public" | "restricted" | "personal") {
        return Some("bad_privacy");
    }
    let source = &map["source"];
    let source_ok = match source.get("kind").and_then(Value::as_str) {
        Some("measured") => source.get("tool").is_some_and(short_text),
        Some("imported") => source
            .get("origin")
            .is_some_and(|o| str_with(o, |o| chars_between(o, 1, 512) && !leaks_path(o))),
        _ => false,
    };
    if !source_ok {
        return Some("bad_source");
    }
    if map
        .get("note")
        .is_some_and(|n| !str_with(n, |n| n.chars().count() <= 500))
    {
        return Some("bad_note");
    }

    let mut exposed: Vec<&str> = Vec::new();
    for item in map["inputs"].as_array().into_iter().flatten() {
        exposed.extend(item.get("name").and_then(Value::as_str));
    }
    if let Some(params) = map.get("params") {
        strings(params, &mut exposed);
    }
    exposed.extend(map.get("note").and_then(Value::as_str));
    exposed.extend(
        map.get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str),
    );
    if exposed.iter().any(|s| leaks_path(s)) {
        return Some("personal_path");
    }
    None
}

/// A line of the log without its newline, as a reader sees it (§1).
pub fn line_problem(line: &str) -> Option<&'static str> {
    if line.len() > MAX_LINE {
        return Some("too_large");
    }
    match serde_json::from_str::<Value>(line) {
        Ok(value) => run_problem(&value),
        Err(_) => Some("not_json"),
    }
}

// ------------------------------------------------------------------------------------------- §8

/// §8: whether two runs compare, the speedup of `candidate` over `baseline`, and what differs.
#[derive(Debug, Clone, PartialEq)]
pub enum Comparison {
    NotComparable {
        reason: &'static str,
    },
    Comparable {
        speedup: Option<f64>,
        flags: Vec<&'static str>,
    },
}

fn inputs_key(run: &Value) -> Vec<(String, String)> {
    let mut key: Vec<(String, String)> = run["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| {
            (
                i["name"].as_str().unwrap_or_default().to_owned(),
                i["sha256"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    key.sort();
    key
}

fn machine_key(run: &Value) -> [&Value; 3] {
    let m = &run["machine"];
    [&m["model"], &m["chip"], &m["memory_bytes"]]
}

pub fn compare(baseline: &Value, candidate: &Value) -> Comparison {
    for run in [baseline, candidate] {
        if run_problem(run).is_some() || run["status"] != "ok" {
            return Comparison::NotComparable { reason: "not_ok" };
        }
    }
    if baseline["suite"] != candidate["suite"] || baseline["case"] != candidate["case"] {
        return Comparison::NotComparable {
            reason: "other_case",
        };
    }
    if inputs_key(baseline) != inputs_key(candidate) {
        return Comparison::NotComparable {
            reason: "other_inputs",
        };
    }
    let mut flags = Vec::new();
    if machine_key(baseline) != machine_key(candidate) {
        flags.push("machine");
    }
    if baseline["cache"] != candidate["cache"] {
        flags.push("cache");
    }
    let empty = Value::Object(Map::new());
    if baseline.get("params").unwrap_or(&empty) != candidate.get("params").unwrap_or(&empty) {
        flags.push("params");
    }
    if baseline["subject"]["dirty"] != false || candidate["subject"]["dirty"] != false {
        flags.push("dirty");
    }
    let speedup = match (wall_median(baseline), wall_median(candidate)) {
        (Some(a), Some(b)) if b != 0.0 => Some(a / b),
        _ => None,
    };
    Comparison::Comparable { speedup, flags }
}

/// The median wall time of a run, `None` for a quality-only run (§6.1) or a run without `wall_ms`.
pub fn wall_median(run: &Value) -> Option<f64> {
    run.get("metrics")?.get("wall_ms")?.get("median")?.as_f64()
}

/// A fresh run id: 32 hexadecimal digits from the system generator.
pub fn new_run_id() -> nu9ve_json::Result<String> {
    Ok(nu9ve_json::hex(&nu9ve_json::random_bytes::<16>()?))
}

/// The id of an imported run (§7): the first 32 hexadecimal digits of the SHA-256 of its origin.
pub fn imported_run_id(origin: &str) -> String {
    nu9ve_json::sha256_hex(origin.as_bytes())[..32].to_owned()
}
