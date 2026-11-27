//! The record of a long action (`nu9ve-action-v1`, vendor/nu9ve-core/contracts/nu9ve-action-v1.md) in the catalog's
//! `metadata`, under `action-progress:<id>` (closeout, `closure-progress:<id>`).
//!
//! An engine call is one action per `progress_id`. Its first [`ActionProgress`] opens the record (`running`), and
//! later ones in the same call, such as each collection of a conversations backup or the release after a closeout,
//! continue it without moving the bar back. [`within`] wraps the engine call and closes every action it opened with
//! the call's result: `complete` (with its warnings), `failed` or `cancelled`. While the action runs, a thread with its
//! own connection moves `updated_at` every few seconds (the heartbeat, §3.3), also during steps that report nothing.
//! The closed record stays (§2) until the same id runs again or [`prune`] retires it a day later.

use crate::catalog::WritableCatalog;
use crate::transfer::Meter;
use nu9ve_json::{Error, Result, require};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const CONTRACT: &str = "nu9ve-action/1";
pub(crate) const PREFIX: &str = "action-progress:";
pub(crate) const CLOSURE_PREFIX: &str = "closure-progress:";
/// §3.3: at least every 10 s while running.
const HEARTBEAT: Duration = Duration::from_secs(5);
/// §3.3: no more than 4 writes a second, always on a new stage or a finished file.
const THROTTLE: Duration = Duration::from_millis(250);
/// A running record whose heartbeat is older than this belongs to a process that is gone (§3.5, §4).
const STALE_SECONDS: i64 = 60;
/// Closed records outlive the 10 minutes of §2 by a day, then go.
const KEEP_SECONDS: i64 = 24 * 3600;

/// The verb, whether it can be cancelled and whether a failure resumes where it stopped (a journaled apply) or
/// starts again (a plan), given by the step that opens the action.
#[derive(Clone, Copy)]
pub(crate) struct Action {
    pub verb: &'static str,
    pub cancellable: bool,
    pub resumable: bool,
}

impl Action {
    pub(crate) const fn new(verb: &'static str) -> Self {
        Self {
            verb,
            cancellable: false,
            resumable: true,
        }
    }

    pub(crate) const fn cancellable(verb: &'static str) -> Self {
        Self {
            verb,
            cancellable: true,
            resumable: true,
        }
    }

    /// Reviewing what an action would do: after a failure it is planned again.
    pub(crate) const fn planning(verb: &'static str) -> Self {
        Self {
            verb,
            cancellable: false,
            resumable: false,
        }
    }
}

/// §1: `^[a-z0-9][a-z0-9._:-]{0,127}$`.
pub fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    id.len() <= 128
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._:-".contains(c))
}

/// §2 `action_progress`: the record of `progress_id`, or `state: unknown` when there is none.
///
/// `legacy` is the parameter of the method this one replaces (`plan_id` for the `closure_progress` alias, §2),
/// accepted alone in place of `progress_id`. Closures keep their record under `closure-progress:`, read after
/// `action-progress:`.
pub fn progress_method(
    catalog: &crate::catalog::Catalog,
    params: &Value,
    legacy: Option<&str>,
) -> Result<Value> {
    let invalid = || Error::new("invalid_params", "action_progress takes one progress_id.");
    let fields = params.as_object().ok_or_else(invalid)?;
    let old = legacy.and_then(|name| fields.get(name));
    let id = match (fields.get("progress_id"), old, fields.len()) {
        (Some(id), None, 1) | (None, Some(id), 1) => id.as_str().ok_or_else(invalid)?,
        _ => return Err(invalid()),
    };
    require(
        valid_id(id),
        "invalid_params",
        "progress_id does not follow the nu9ve-action/1 pattern.",
    )?;
    for prefix in [PREFIX, CLOSURE_PREFIX] {
        if let Some(record) = catalog.get(&format!("{prefix}{id}"))? {
            return Ok(record);
        }
    }
    Ok(json!({"contract": CONTRACT, "progress_id": id, "state": "unknown"}))
}

/// §3.2: this engine's step names in the common vocabulary, or under `nuun.`.
pub(crate) fn common_stage(stage: &str) -> String {
    match stage {
        "preparing" | "listing" | "scanning" | "reviewing" | "copying" | "verifying"
        | "rendering" | "analyzing" | "releasing" | "writing" | "finishing" => stage,
        "listing_source" => "listing",
        "indexing_archive" => "scanning",
        "review_inventory" | "review_final" => "reviewing",
        "delivering_work" | "copying_finals" | "copying_transcripts" => "copying",
        "hashing_source" | "verifying_archive" => "verifying",
        "releasing_originals" | "cleaning" => "releasing",
        "complete" | "completed" | "release_complete" => "finishing",
        other => return format!("nuun.{other}"),
    }
    .to_owned()
}

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn read(db: &Connection, key: &str) -> Result<Option<(String, Value)>> {
    let raw: Option<String> = db
        .query_row("SELECT value FROM metadata WHERE key=?", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(sql)?;
    Ok(raw.and_then(|text| {
        let value = serde_json::from_str(&text).ok()?;
        Some((text, value))
    }))
}

fn write(db: &Connection, key: &str, value: &Value) -> Result<()> {
    let text = String::from_utf8(nu9ve_json::canonical(value)).unwrap_or_default();
    db.execute("INSERT OR REPLACE INTO metadata VALUES(?,?)", (key, text))
        .map_err(sql)?;
    Ok(())
}

fn seconds_since(instant: &Value) -> Option<i64> {
    let text = instant.as_str()?.replace('Z', "+00:00");
    let then = nu9ve_time::DateTime::fromisoformat(&text)?.timestamp()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(now - then)
}

fn running(record: &Value) -> bool {
    record["contract"] == CONTRACT && record["state"] == "running"
}

fn fraction(record: &Value) -> Option<f64> {
    let total = record["overall_total"].as_u64().filter(|t| *t > 0)?;
    Some(record["overall_done"].as_u64().unwrap_or(0) as f64 / total as f64)
}

/// Retire closed records a day old (and running ones whose process left them a day ago).
pub(crate) fn prune(db: &Connection) -> Result<()> {
    let mut statement = db
        .prepare(
            "SELECT key, value FROM metadata WHERE substr(key, 1, 16)='action-progress:' \
             OR substr(key, 1, 17)='closure-progress:'",
        )
        .map_err(sql)?;
    let old: Vec<String> = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql)?
        .filter_map(|row| row.ok())
        .filter(|(_, text)| {
            serde_json::from_str::<Value>(text).is_ok_and(|record| {
                record["contract"] == CONTRACT
                    && seconds_since(&record["updated_at"]).is_some_and(|age| age > KEEP_SECONDS)
            })
        })
        .map(|(key, _)| key)
        .collect();
    for key in old {
        db.execute("DELETE FROM metadata WHERE key=?", [key])
            .map_err(sql)?;
    }
    Ok(())
}

/// An action this thread opened in the current engine call.
struct Open {
    path: PathBuf,
    key: String,
    resumable: bool,
    stop: Option<mpsc::Sender<()>>,
    beat: Option<JoinHandle<()>>,
}

impl Open {
    fn stop(&mut self) {
        self.stop.take();
        if let Some(beat) = self.beat.take() {
            let _ = beat.join();
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.stop();
    }
}

thread_local! {
    static OPEN: RefCell<Vec<Open>> = const { RefCell::new(Vec::new()) };
    static INGEST: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Run an ingest whose card transfer reports under `progress_id` (the job keeps its own `progress:<job>`).
pub(crate) fn with_ingest<T>(progress_id: Option<&str>, run: impl FnOnce() -> T) -> T {
    let previous =
        INGEST.with(|id| id.replace(progress_id.filter(|id| !id.is_empty()).map(str::to_owned)));
    let result = run();
    INGEST.with(|id| *id.borrow_mut() = previous);
    result
}

pub(crate) fn ingest_id() -> Option<String> {
    INGEST.with(|id| id.borrow().clone())
}

fn is_open(path: &Path, key: &str) -> bool {
    OPEN.with(|open| {
        open.borrow()
            .iter()
            .any(|item| item.path == path && item.key == key)
    })
}

/// Move `updated_at` of a running record without touching anything else; skip when the step just wrote it.
fn touch(db: &Connection, key: &str) {
    let Ok(Some((before, mut record))) = read(db, key) else {
        return;
    };
    if !running(&record) {
        return;
    }
    record["updated_at"] = json!(nu9ve_time::now_utc());
    let after = String::from_utf8(nu9ve_json::canonical(&record)).unwrap_or_default();
    let _ = db.execute(
        "UPDATE metadata SET value=? WHERE key=? AND value=?",
        (after, key, before),
    );
}

fn heartbeat(path: PathBuf, key: String) -> (mpsc::Sender<()>, JoinHandle<()>) {
    let (stop, wait) = mpsc::channel::<()>();
    let beat = std::thread::spawn(move || {
        let Ok(db) = Connection::open(&path) else {
            return;
        };
        // A step holding a write transaction wins: this beat waits a little and tries at the next one.
        let _ = db.busy_timeout(Duration::from_secs(1));
        while let Err(mpsc::RecvTimeoutError::Timeout) = wait.recv_timeout(HEARTBEAT) {
            touch(&db, &key);
        }
    });
    (stop, beat)
}

/// Run one engine call; every action it opened is closed with its result. A failure to write the closing record
/// never hides the call's own answer.
pub fn within(call: impl FnOnce() -> Result<Value>) -> Result<Value> {
    let start = OPEN.with(|open| open.borrow().len());
    let result = call();
    let opened: Vec<Open> = OPEN.with(|open| open.borrow_mut().drain(start..).collect());
    for mut item in opened {
        item.stop();
        let _ = close(&item.path, &item.key, item.resumable, &result);
    }
    result
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn amount(record: &Value) -> String {
    let files = record["files_done"].as_u64().unwrap_or(0);
    let files = if files == 1 {
        "1 file".to_owned()
    } else {
        format!("{files} files")
    };
    match record["overall_done"].as_u64() {
        Some(bytes) if bytes > 0 && record["unit"] == "bytes" => {
            format!("{files}, {}", human_bytes(bytes))
        }
        _ => files,
    }
}

fn cut(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// What a completed call left pending, from its answer (§3.4): changed files, a working copy or a metadata backup
/// that did not happen.
pub(crate) fn warnings(answer: &Value) -> Vec<Value> {
    let mut found = Vec::new();
    let mut changed = answer["result"]["changed_since_review"]
        .as_u64()
        .unwrap_or(0);
    for entry in answer["results"].as_array().into_iter().flatten() {
        changed += entry["result"]["changed_since_review"]
            .as_u64()
            .unwrap_or(0);
        if entry["working"]["state"] == "failed" {
            found.push(
                json!({"code": "working_copy_failed", "message": cut(&format!(
                "The working copy of {} did not happen: {}",
                entry["collection"].as_str().unwrap_or("a collection"),
                entry["working"]["message"].as_str().unwrap_or_default()), 500)}),
            );
        }
    }
    if changed > 0 {
        found.insert(0, json!({"code": "changed_during_copy", "message": format!(
            "{changed} file{} changed during the copy: {} in the next backup",
            if changed == 1 { "" } else { "s" }, if changed == 1 { "it goes" } else { "they go" })}));
    }
    if let Some(message) = answer["metadata_backup_error"].as_str() {
        found.push(json!({"code": "metadata_backup_failed",
            "message": cut(&format!("The catalog backup did not happen: {message}"), 500)}));
    }
    if answer["units_map"] == "failed" {
        found.push(json!({"code": "units_map_failed", "message": cut(&format!(
            "The missions map was not written: {}", answer["units_map_error"].as_str().unwrap_or_default()), 500)}));
    }
    found.truncate(20);
    found
}

fn close(path: &Path, key: &str, resumable: bool, result: &Result<Value>) -> Result<()> {
    let db = Connection::open(path).map_err(sql)?;
    db.busy_timeout(Duration::from_secs(15)).map_err(sql)?;
    let Some((_, mut record)) = read(&db, key)? else {
        return Ok(());
    };
    if !running(&record) {
        return Ok(());
    }
    match result {
        Ok(answer) => {
            if let Some(total) = record["overall_total"].as_u64().filter(|t| *t > 0) {
                record["overall_done"] = json!(total);
            }
            record["state"] = json!("complete");
            record["stage"] = json!("finishing");
            record["outcome"] = json!({"summary": format!("Done: {}.", amount(&record)),
                "warnings": warnings(answer), "receipt": null});
        }
        Err(error) => {
            let did = record["files_done"].as_u64().unwrap_or(0) > 0;
            if error.code == "cancelled" {
                record["state"] = json!("cancelled");
                record["outcome"] = json!({"summary": if did {
                    format!("Cancelled after {}.", amount(&record))
                } else {
                    "Cancelled before any file was done.".to_owned()
                }, "warnings": [], "receipt": null});
            } else {
                record["state"] = json!("failed");
                record["error"] = json!({"code": error.code, "message": cut(&error.message, 500)});
                if did {
                    record["outcome"] = json!({"summary": format!("Done before the error: {}.", amount(&record)),
                        "warnings": [], "receipt": null});
                }
            }
            record["resumable"] = json!(resumable);
        }
    }
    let object = record.as_object_mut().expect("a record is an object");
    object.remove("current");
    object.insert("cancellable".into(), json!(false));
    object.insert("updated_at".into(), json!(nu9ve_time::now_utc()));
    write(&db, key, &record)
}

/// One step's view of an action: it opens the record when this call has not, otherwise it continues it.
pub(crate) struct ActionProgress<'a> {
    catalog: &'a WritableCatalog,
    key: Option<String>,
    pub(crate) meter: Meter,
    pub(crate) stage: &'static str,
    closure: bool,
    written: Option<(Instant, &'static str, usize)>,
    /// A measure the throttle held back; written when the step ends, so the closing counts it.
    held: bool,
}

impl<'a> ActionProgress<'a> {
    pub(crate) fn new(
        catalog: &'a WritableCatalog,
        id: Option<&str>,
        action: Action,
        stage: &'static str,
    ) -> Result<Self> {
        Self::with_prefix(catalog, id, action, stage, false)
    }

    /// A closeout's record, read by `closure_progress` (it also counts `nuun_released_files`).
    pub(crate) fn closure(
        catalog: &'a WritableCatalog,
        id: Option<&str>,
        action: Action,
        stage: &'static str,
    ) -> Result<Self> {
        Self::with_prefix(catalog, id, action, stage, true)
    }

    fn with_prefix(
        catalog: &'a WritableCatalog,
        id: Option<&str>,
        action: Action,
        stage: &'static str,
        closure: bool,
    ) -> Result<Self> {
        let id = id.filter(|id| !id.is_empty());
        let prefix = if closure { CLOSURE_PREFIX } else { PREFIX };
        let mut progress = Self {
            catalog,
            key: id.map(|id| format!("{prefix}{id}")),
            meter: Meter::default(),
            stage,
            closure,
            written: None,
            held: false,
        };
        if let (Some(id), Some(key)) = (id, progress.key.clone()) {
            progress.open(id, &key, action)?;
        }
        Ok(progress)
    }

    fn open(&mut self, id: &str, key: &str, action: Action) -> Result<()> {
        let db = self.catalog.db();
        let path = PathBuf::from(db.path().unwrap_or_default());
        if is_open(&path, key) {
            // A later step of the same action: the log goes on where the previous step left it.
            if let Some((_, record)) = read(db, key)? {
                self.meter.log = record["log"].as_array().cloned().unwrap_or_default();
            }
            return Ok(());
        }
        require(
            valid_id(id),
            "invalid_params",
            "'progress_id' must match ^[a-z0-9][a-z0-9._:-]{0,127}$",
        )?;
        if let Some((_, record)) = read(db, key)? {
            require(
                !running(&record)
                    || seconds_since(&record["updated_at"]).is_none_or(|age| age > STALE_SECONDS),
                "action_running",
                format!("An action with progress_id {id} is still running"),
            )?;
        }
        prune(db)?;
        let now = nu9ve_time::now_utc();
        let mut record = json!({"contract": CONTRACT, "progress_id": id, "action": action.verb,
            "state": "running", "stage": common_stage(self.stage), "overall_done": 0, "overall_total": null,
            "unit": "bytes", "files_done": 0, "files_total": 0, "log": [], "cancellable": action.cancellable,
            "started_at": now, "progressed_at": now, "updated_at": now});
        if self.closure {
            record["nuun_released_files"] = json!(0);
        }
        write(db, key, &record)?;
        let (stop, beat) = heartbeat(path.clone(), key.to_owned());
        OPEN.with(|open| {
            open.borrow_mut().push(Open {
                path,
                key: key.to_owned(),
                resumable: action.resumable,
                stop: Some(stop),
                beat: Some(beat),
            })
        });
        self.written = Some((Instant::now(), self.stage, 0));
        Ok(())
    }

    /// Write this step's measure into the action's record (at most 4 times a second; always on a new stage or a
    /// finished file). The fraction never goes back, and 100 % waits for the close.
    pub(crate) fn report(&mut self) -> Result<()> {
        let Some(key) = &self.key else {
            return Ok(());
        };
        if let Some((at, stage, files)) = self.written
            && stage == self.stage
            && files == self.meter.files_done
            && at.elapsed() < THROTTLE
        {
            self.held = true;
            return Ok(());
        }
        self.held = false;
        let db = self.catalog.db();
        let Some((_, mut record)) = read(db, key)? else {
            return Ok(());
        };
        if !running(&record) {
            return Ok(());
        }
        let now = nu9ve_time::now_utc();
        let before = fraction(&record);
        let (done, total) = (self.meter.done(), self.meter.total);
        if total > 0 {
            let floor = before.map_or(0, |f| (f * total as f64).ceil() as u64);
            record["overall_total"] = json!(total);
            record["overall_done"] = json!(done.max(floor).min(total - 1));
        } else if before.is_none() {
            record["overall_total"] = Value::Null;
            record["overall_done"] = json!(0);
        }
        if fraction(&record).unwrap_or(0.0) > before.unwrap_or(0.0) {
            record["progressed_at"] = json!(now);
        }
        record["stage"] = json!(common_stage(self.stage));
        record["files_done"] = json!(self.meter.files_done);
        record["files_total"] = json!(self.meter.files_total);
        record["log"] = json!(self.meter.log);
        let object = record.as_object_mut().expect("a record is an object");
        if self.meter.current.is_empty() {
            object.remove("current");
        } else {
            object.insert("current".into(), Value::Object(self.meter.current.clone()));
        }
        if self.closure {
            let released = if matches!(
                self.stage,
                "releasing_originals" | "metadata_backup_after" | "release_complete"
            ) {
                self.meter.files_done
            } else {
                0
            };
            object.insert("nuun_released_files".into(), json!(released));
        }
        object.insert("updated_at".into(), json!(now));
        write(db, key, &record)?;
        self.written = Some((Instant::now(), self.stage, self.meter.files_done));
        Ok(())
    }
}

impl Drop for ActionProgress<'_> {
    /// The step ends before the close: the measure the throttle held back goes in now.
    fn drop(&mut self) {
        if self.held {
            self.written = None;
            let _ = self.report();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> (tempfile::TempDir, WritableCatalog) {
        let dir = tempfile::tempdir().unwrap();
        let catalog = WritableCatalog::open(dir.path()).unwrap();
        (dir, catalog)
    }

    fn record(catalog: &WritableCatalog, key: &str) -> Value {
        catalog.get(key).unwrap().unwrap()
    }

    #[test]
    fn ids_follow_the_contract_pattern() {
        for id in [
            "a",
            "conversations-now",
            "ingreso-2026-11-26-1",
            "x:y.z_w",
            "0f3c",
        ] {
            assert!(valid_id(id), "{id}");
        }
        for id in ["", "A", "-a", "a b", "ñ", &"a".repeat(129)] {
            assert!(!valid_id(id), "{id}");
        }
    }

    #[test]
    fn the_reader_serves_records_closures_and_unknown_ids() {
        let (dir, writable) = catalog();
        writable
            .set("action-progress:a-1", &json!({"state": "running"}))
            .unwrap();
        writable
            .set("closure-progress:p-1", &json!({"state": "complete"}))
            .unwrap();
        let catalog = crate::catalog::Catalog::open(dir.path()).unwrap();
        let read = |params: Value, legacy| progress_method(&catalog, &params, legacy);
        assert_eq!(
            read(json!({"progress_id": "a-1"}), None).unwrap()["state"],
            "running"
        );
        assert_eq!(
            read(json!({"plan_id": "p-1"}), Some("plan_id")).unwrap()["state"],
            "complete"
        );
        assert_eq!(
            read(json!({"progress_id": "p-1"}), None).unwrap()["state"],
            "complete"
        );
        assert_eq!(
            read(json!({"progress_id": "nope"}), None).unwrap(),
            json!({"contract": CONTRACT, "progress_id": "nope", "state": "unknown"})
        );
        for params in [
            json!({}),
            json!([]),
            json!({"id": "a-1"}),
            json!({"progress_id": "a-1", "id": "a-1"}),
            json!({"progress_id": "a-1", "verbose": true}),
            json!({"plan_id": "p-1"}),
            json!({"progress_id": "A-1"}),
            json!({"progress_id": 7}),
        ] {
            assert_eq!(
                read(params.clone(), None).unwrap_err().code,
                "invalid_params",
                "{params}"
            );
        }
    }

    #[test]
    fn stages_are_common_or_prefixed() {
        assert_eq!(common_stage("listing_source"), "listing");
        assert_eq!(common_stage("verifying_archive"), "verifying");
        assert_eq!(common_stage("release_complete"), "finishing");
        assert_eq!(common_stage("relinking"), "nuun.relinking");
        assert_eq!(
            common_stage("metadata_backup_after"),
            "nuun.metadata_backup_after"
        );
    }

    #[test]
    fn a_call_opens_measures_and_closes_its_action() {
        let (_dir, catalog) = catalog();
        let key = "action-progress:copy-1";
        let answer = within(|| {
            let mut progress =
                ActionProgress::new(&catalog, Some("copy-1"), Action::new("copy"), "preparing")?;
            let opened = record(&catalog, key);
            assert_eq!(opened["contract"], CONTRACT);
            assert_eq!(opened["state"], "running");
            assert_eq!(opened["overall_total"], Value::Null);
            assert_eq!(opened["started_at"], opened["progressed_at"]);
            progress.meter.total = 100;
            progress.meter.files_total = 2;
            progress.stage = "copying";
            progress.meter.holding("a/one.mp4", "CARD", "MEDIA", "copy");
            progress.meter.reach(100);
            progress.meter.finished("copied", "a/one.mp4", "MEDIA");
            progress.report()?;
            let running = record(&catalog, key);
            // 100 % only arrives with the close.
            assert_eq!(running["overall_done"], 99);
            assert_eq!(running["stage"], "copying");
            assert_eq!(running["current"]["name"], "one.mp4");
            assert_eq!(running["log"][0]["kind"], "copied");
            // A second step of the same call continues the record without moving the bar back.
            let mut later =
                ActionProgress::new(&catalog, Some("copy-1"), Action::new("other"), "copying")?;
            later.meter.total = 1000;
            later.meter.files_total = 2;
            later.meter.files_done = 1;
            later.meter.reach(10);
            later.stage = "verifying";
            later.report()?;
            let continued = record(&catalog, key);
            assert_eq!(continued["action"], "copy");
            assert_eq!(continued["overall_done"], 990);
            assert_eq!(continued["log"].as_array().unwrap().len(), 1);
            Ok(json!({"result": {"changed_since_review": 2}}))
        })
        .unwrap();
        assert_eq!(answer["result"]["changed_since_review"], 2);
        let closed = record(&catalog, key);
        assert_eq!(closed["state"], "complete");
        assert_eq!(closed["overall_done"], 1000);
        assert_eq!(closed["cancellable"], false);
        assert!(closed.get("current").is_none());
        assert_eq!(
            closed["outcome"]["warnings"][0]["code"],
            "changed_during_copy"
        );
        assert_eq!(closed["outcome"]["summary"], "Done: 1 file, 1.0 KB.");
    }

    #[test]
    fn failures_and_cancellations_close_with_what_was_done() {
        let (_dir, catalog) = catalog();
        let failed = within(|| {
            let mut progress =
                ActionProgress::new(&catalog, Some("f"), Action::new("backup"), "copying")?;
            progress.meter.total = 10;
            progress.meter.reach(5);
            progress.meter.finished("copied", "x", "MEDIA");
            progress.report()?;
            Err(Error::new("checksum_mismatch", "The copy did not verify"))
        });
        assert_eq!(failed.unwrap_err().code, "checksum_mismatch");
        let record_f = record(&catalog, "action-progress:f");
        assert_eq!(record_f["state"], "failed");
        assert_eq!(record_f["error"]["code"], "checksum_mismatch");
        assert_eq!(record_f["resumable"], true);
        assert_eq!(
            record_f["outcome"]["summary"],
            "Done before the error: 1 file, 5 B."
        );
        let _ = within(|| {
            ActionProgress::new(
                &catalog,
                Some("c"),
                Action::cancellable("ingest"),
                "copying",
            )?;
            Err(Error::new("cancelled", "Operation paused"))
        });
        let _ = within(|| {
            ActionProgress::new(
                &catalog,
                Some("p"),
                Action::planning("copy"),
                "listing_source",
            )?;
            Err(Error::new("volume_offline", "Destination offline"))
        });
        assert_eq!(record(&catalog, "action-progress:p")["resumable"], false);
        let record_c = record(&catalog, "action-progress:c");
        assert_eq!(record_c["state"], "cancelled");
        assert_eq!(record_c["cancellable"], false);
        assert!(record_c.get("error").is_none());
    }

    #[test]
    fn a_running_id_is_refused_and_a_finished_one_is_reused() {
        let (_dir, catalog) = catalog();
        within(|| {
            ActionProgress::new(&catalog, Some("r"), Action::new("copy"), "copying")?;
            Ok(json!({}))
        })
        .unwrap();
        // Another process still running this id (a fresh heartbeat).
        let mut record_r = record(&catalog, "action-progress:r");
        record_r["state"] = json!("running");
        record_r["updated_at"] = json!(nu9ve_time::now_utc());
        catalog.set("action-progress:r", &record_r).unwrap();
        let refused = within(|| {
            ActionProgress::new(&catalog, Some("r"), Action::new("copy"), "copying")?;
            Ok(json!({}))
        });
        assert_eq!(refused.unwrap_err().code, "action_running");
        // Its process is gone (old heartbeat): the new action replaces it.
        record_r["updated_at"] = json!("2026-01-01T00:00:00Z");
        catalog.set("action-progress:r", &record_r).unwrap();
        within(|| {
            ActionProgress::new(&catalog, Some("r"), Action::new("backup"), "copying")?;
            Ok(json!({}))
        })
        .unwrap();
        assert_eq!(record(&catalog, "action-progress:r")["action"], "backup");
        assert_eq!(record(&catalog, "action-progress:r")["state"], "complete");
        let invalid = within(|| {
            ActionProgress::new(&catalog, Some("Bad Id"), Action::new("copy"), "copying")?;
            Ok(json!({}))
        });
        assert_eq!(invalid.unwrap_err().code, "invalid_params");
    }

    #[test]
    fn the_heartbeat_moves_updated_at_while_a_step_says_nothing() {
        let (_dir, catalog) = catalog();
        within(|| {
            ActionProgress::new(&catalog, Some("h"), Action::new("copy"), "copying")?;
            let mut stale = record(&catalog, "action-progress:h");
            stale["updated_at"] = json!("2026-01-01T00:00:00Z");
            catalog.set("action-progress:h", &stale).unwrap();
            std::thread::sleep(HEARTBEAT + Duration::from_millis(600));
            let beaten = record(&catalog, "action-progress:h");
            assert_ne!(beaten["updated_at"], "2026-01-01T00:00:00Z");
            assert_eq!(beaten["progressed_at"], beaten["started_at"]);
            Ok(json!({}))
        })
        .unwrap();
    }

    #[test]
    fn closed_records_a_day_old_are_pruned() {
        let (_dir, catalog) = catalog();
        catalog
            .set(
                "action-progress:old",
                &json!({"contract": CONTRACT, "state": "complete", "updated_at": "2026-01-01T00:00:00Z"}),
            )
            .unwrap();
        catalog
            .set("action-progress:legacy", &json!({"stage": "copying"}))
            .unwrap();
        prune(catalog.db()).unwrap();
        assert!(catalog.get("action-progress:old").unwrap().is_none());
        assert!(catalog.get("action-progress:legacy").unwrap().is_some());
    }

    /// The records this engine writes, with their instants fixed, for the app's test that reads them with the
    /// core's Swift reader (`engineRecordsPassTheCoreReader`). `NUUN_UPDATE_FIXTURES=1` writes the file again.
    #[test]
    fn engine_records_fixture() {
        let (_dir, catalog) = catalog();
        let mut records = Vec::new();
        let mut keep = |name: &str, key: &str, catalog: &WritableCatalog| {
            records.push(json!({"name": name, "record": fixed(record(catalog, key))}));
        };
        let _ = within(|| {
            let mut progress = ActionProgress::new(
                &catalog,
                Some("copy-1"),
                Action::cancellable("ingest"),
                "copying",
            )?;
            progress.meter.total = 4_000;
            progress.meter.files_total = 2;
            progress
                .meter
                .holding("A/C0001.MP4", "CARD", "MEDIA", "copy");
            progress.meter.reach(1_500);
            progress.meter.finished("copied", "A/C0001.MP4", "MEDIA");
            progress
                .meter
                .holding("A/C0002.MP4", "CARD", "MEDIA", "copy");
            progress.report()?;
            keep("running", "action-progress:copy-1", &catalog);
            Ok(json!({"result": {"changed_since_review": 1}}))
        });
        keep(
            "complete with a warning",
            "action-progress:copy-1",
            &catalog,
        );
        let _ = within(|| {
            let mut progress = ActionProgress::new(
                &catalog,
                Some("f-1"),
                Action::new("backup"),
                "copying_transcripts",
            )?;
            progress.meter.total = 10;
            progress.meter.reach(5);
            progress.report()?;
            Err(Error::new("checksum_mismatch", "The copy did not verify"))
        });
        keep("failed", "action-progress:f-1", &catalog);
        let _ = within(|| {
            ActionProgress::new(
                &catalog,
                Some("c-1"),
                Action::planning("copy"),
                "listing_source",
            )?;
            Err(Error::new("cancelled", "Operation paused"))
        });
        keep("cancelled before any file", "action-progress:c-1", &catalog);
        let _ = within(|| {
            let mut progress = ActionProgress::closure(
                &catalog,
                Some("plan-1"),
                Action::new("closeout"),
                "releasing_originals",
            )?;
            progress.meter.total = 100;
            progress.meter.files_total = 3;
            progress
                .meter
                .finished("released", "renders/final.mov", "EDICION");
            progress.meter.reach(40);
            progress.report()?;
            keep("closure releasing", "closure-progress:plan-1", &catalog);
            Ok(json!({}))
        });
        let text = serde_json::to_string_pretty(&json!({
            "note": "Written by engine_records_fixture in engine/crates/nuun-media/src/action.rs.",
            "records": records,
        }))
        .unwrap()
            + "\n";
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../native/Nuun/Tests/NuunCoreTests/Fixtures/engine-action-records.json");
        if std::env::var_os("NUUN_UPDATE_FIXTURES").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap_or_default(),
            text,
            "run with NUUN_UPDATE_FIXTURES=1 to write {}",
            path.display()
        );
    }

    /// The record with fixed instants, after checking that each one is RFC 3339 UTC with `Z`.
    fn fixed(mut record: Value) -> Value {
        fn instant(text: &Value) -> bool {
            let text = text.as_str().unwrap_or("");
            let bytes = text.as_bytes();
            text.len() >= 20
                && text.ends_with('Z')
                && bytes[4] == b'-'
                && bytes[10] == b'T'
                && nu9ve_time::DateTime::fromisoformat(&text.replace('Z', "+00:00")).is_some()
        }
        for (key, value) in [
            ("started_at", "2026-11-27T10:00:00Z"),
            ("progressed_at", "2026-11-27T10:00:01Z"),
            ("updated_at", "2026-11-27T10:00:02Z"),
        ] {
            assert!(instant(&record[key]), "{key}: {}", record[key]);
            record[key] = json!(value);
        }
        if let Some(log) = record["log"].as_array_mut() {
            for line in log {
                assert!(instant(&line["at"]), "log: {line}");
                line["at"] = json!("2026-11-27T10:00:01Z");
            }
        }
        record
    }
}
