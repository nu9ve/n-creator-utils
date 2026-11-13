//! Verified, resumable transfers; source cleanup is gated by durable receipts. Port of `media/transfer.py`
//! (`Transfer.apply`, `_run`, `copy_many`, `validate_source`, `_index`, progress) and `progress.ActionMeter`.
//!
//! Guarantees, as Python:
//! - a copy lands as `.<name>.ncpartial`, is flushed (`F_FULLFSYNC` on macOS) with its folder, read back and
//!   hashed (SHA-256) against the bytes read from the source, and only then renamed to its name, never over a file;
//! - identical bytes already at a destination are adopted; anything else there is a `destination_conflict`;
//! - each verified copy is journaled (`members`, `copies`) before the next file, so an interrupted job resumes
//!   from the catalog, reading a verified replica instead of the card a second time;
//! - the card is cleaned only after every destination is verified, receipts are written and the source is
//!   rescanned unchanged; each original is re-read and must hash like its copies before it is removed; Sony
//!   camera indexes are never removed (the job waits for the camera's format).
//!
//! Stalker release after ingest and published finals cleanup require Clipper exclusion on Unix.
//! Cancellation from the client is
//! a process kill: the journal
//! makes it resumable (the job stays in its last state; Python would record `paused`).

use crate::catalog::{Catalog, WritableCatalog};
use crate::layout::sony_index_member;
use crate::planner::{destination_path, preflight, validate_plan};
use crate::scan::scan;
use crate::tree::{Root, stamp_json};
use crate::volumes::Backend;
use nu9ve_json::{Error, NAMESPACE_URL, Result, canonical, digest, require, uuid5};
use nu9ve_volumes::fs;
use rusqlite::OptionalExtension;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const CHUNK_SIZE: usize = 4 * 1024 * 1024;
const LOG_LIMIT: usize = 60;
const FINAL_STATES: [&str; 4] = [
    "completed",
    "camera_action_required",
    "source_retained",
    "source_has_new_material",
];

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// What a test or the client hooks between steps (Python `hook(stage, payload)`); an error stops the job there.
pub type Hook<'a> = &'a dyn Fn(&str, &Value) -> Result<()>;

/// Python `progress.ActionMeter`: one overall measure that never goes back, the file in hand and a short log.
#[derive(Default)]
pub(crate) struct Meter {
    pub(crate) total: u64,
    done: u64,
    pub(crate) files_total: usize,
    pub(crate) files_done: usize,
    pub(crate) current: Map<String, Value>,
    log: Vec<Value>,
}

impl Meter {
    pub(crate) fn reach(&mut self, value: u64) {
        self.done = self.done.max(value.min(self.total));
    }

    pub(crate) fn holding(&mut self, path: &str, origin: &str, target: &str, action: &str) {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.current = json!({"name": name, "from": origin, "to": target, "action": action})
            .as_object()
            .cloned()
            .unwrap_or_default();
    }

    pub(crate) fn finished(&mut self, kind: &str, path: &str, target: &str) {
        self.files_done = if self.files_total > 0 {
            self.files_total.min(self.files_done + 1)
        } else {
            self.files_done + 1
        };
        let name = path.rsplit('/').next().unwrap_or(path);
        self.log.push(
            json!({"at": nu9ve_time::now_utc(), "kind": kind, "name": name, "target": target}),
        );
        if self.log.len() > LOG_LIMIT {
            self.log.drain(..self.log.len() - LOG_LIMIT);
        }
    }

    pub(crate) fn fields(&self) -> Map<String, Value> {
        json!({"overall_done": self.done, "overall_total": self.total, "current": self.current, "log": self.log})
            .as_object()
            .cloned()
            .unwrap_or_default()
    }
}

/// SHA-256 of a whole open file from its start (Python `hash_fd`), counting bytes for progress.
pub(crate) fn hash_file(
    file: &mut (impl Read + Seek),
    mut counted: impl FnMut(u64),
) -> Result<String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| Error::new("operation_failed", format!("[OSError] {e}")))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| Error::new("operation_failed", format!("[OSError] {e}")))?;
        if n == 0 {
            return Ok(nu9ve_json::hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..n]);
        counted(n as u64);
    }
}

/// Write a whole block (Python `write_all`).
fn write_all(file: &mut std::fs::File, block: &[u8]) -> Result<()> {
    file.write_all(block)
        .map_err(|e| Error::new("write_failed", format!("Short write without progress: {e}")))
}

/// The partial name of a destination: `<folder>/.<name>.ncpartial` (these names belong to this tool).
fn partial_of(rel: &str) -> String {
    match rel.rsplit_once('/') {
        Some((folder, name)) => format!("{folder}/.{name}.ncpartial"),
        None => format!(".{rel}.ncpartial"),
    }
}

/// One destination of a copy: its open root, the relative destination and its registered volume.
pub struct Target<'a> {
    pub root: &'a Root,
    pub rel: String,
    pub record: &'a Value,
}

pub struct Transfer<'a> {
    home: PathBuf,
    catalog: &'a WritableCatalog,
    backend: &'a dyn Backend,
    hook: Hook<'a>,
    volume_tokens: HashMap<String, Value>,
    source_token: Option<Value>,
    progress: Option<Map<String, Value>>,
    meter: Meter,
    started: Instant,
    flushed: Instant,
    /// The weakest way a verification read reached its bytes in this run (`fs::verify_read`).
    verify_cache: fs::Cache,
    clipper_guard: Option<&'a crate::clipper_release::Guard>,
}

impl<'a> Transfer<'a> {
    pub fn new(
        home: &Path,
        catalog: &'a WritableCatalog,
        backend: &'a dyn Backend,
        hook: Hook<'a>,
    ) -> Self {
        Transfer {
            home: home.to_path_buf(),
            catalog,
            backend,
            hook,
            volume_tokens: HashMap::new(),
            source_token: None,
            progress: None,
            meter: Meter::default(),
            started: Instant::now(),
            flushed: Instant::now(),
            verify_cache: fs::Cache::Bypassed,
            clipper_guard: None,
        }
    }

    fn check_volume(&mut self, record: &Value, full: bool) -> Result<Value> {
        let id = text(record, "id").to_owned();
        match self.volume_tokens.get(&id) {
            Some(token) if !full => {
                self.backend
                    .fast_check(Path::new(text(record, "root")), token)?;
            }
            _ => {
                let token = crate::volumes::check(self.backend, record)?;
                self.volume_tokens.insert(id.clone(), token);
            }
        }
        Ok(self.volume_tokens[&id].clone())
    }

    /// `Some("dropped" | "os")` when a verification read of this run may not have come from the device.
    pub fn weak_verification(&self) -> Option<&'static str> {
        (self.verify_cache != fs::Cache::Bypassed).then(|| self.verify_cache.as_str())
    }

    fn checkpoint(&self, stage: &str, payload: &Value) -> Result<()> {
        (self.hook)(stage, payload)
    }

    /// Python `source_check`: the source is the volume/folder the plan was made from.
    fn source_check(&mut self, plan: &Value, full: bool) -> Result<()> {
        let source = Path::new(text(plan, "source"));
        let info = match (&self.source_token, full) {
            (Some(token), false) => self.backend.fast_check(source, token)?,
            _ => {
                let external = plan["cleanup"] == true && plan["source_kind"] == "card";
                let info = self.backend.describe(source, external)?;
                self.source_token = Some(info.clone());
                info
            }
        };
        let identity = plan["source_identity"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        require(
            identity.iter().all(|(k, v)| info.get(k) == Some(v)),
            "source_replaced",
            "Source volume/directory identity changed",
        )?;
        let at_root = || {
            let mount = text(&info, "mount")
                .trim_end_matches(['/', '\\'])
                .to_lowercase();
            let source = text(plan, "source")
                .trim_end_matches(['/', '\\'])
                .to_lowercase();
            truthy(&info["external"])
                && if cfg!(windows) {
                    mount == source
                } else {
                    text(&info, "mount").trim_end_matches('/')
                        == text(plan, "source").trim_end_matches('/')
                }
        };
        require(
            plan["cleanup"] != true || plan["source_kind"] != "card" || at_root(),
            "card_root_required",
            "Cleanup requires the registered card root identity",
        )
    }

    fn removed_members(&self, job_id: &str) -> Result<BTreeSet<String>> {
        let mut statement = self
            .catalog
            .db()
            .prepare("SELECT relative_path FROM members WHERE job_id=? AND cleaned IN (1,-1)")
            .map_err(sql)?;
        let rows = statement
            .query_map([job_id], |r| r.get::<_, String>(0))
            .map_err(sql)?;
        rows.collect::<std::result::Result<_, _>>().map_err(sql)
    }

    /// Python `validate_source` for cards: the same members with the same stamps; discarded and already archived
    /// files may stay; only files this job removed may be missing.
    fn validate_source(&mut self, plan: &Value) -> Result<BTreeMap<String, Value>> {
        self.source_check(plan, true)?;
        let stalker = plan["source_kind"] == "stalker";
        let delivery = plan["source_kind"] == "delivery" && plan.get("layout").is_some();
        let actual = scan(Path::new(text(plan, "source")), stalker, stalker, false)?;
        let bundles = actual.bundles;
        let actual: BTreeMap<String, Value> = actual
            .files
            .into_iter()
            .map(|f| (text(&f, "path").to_owned(), f))
            .collect();
        let expected: BTreeMap<String, &Value> = plan["files"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| (text(f, "path").to_owned(), f))
            .collect();
        if stalker {
            for (key, bundle) in plan["bundles"].as_object().into_iter().flatten() {
                require(
                    bundles.get(key) == Some(bundle),
                    "source_changed",
                    "Stalker manifest changed since planning",
                )?;
                let prefix = if key == "." {
                    String::new()
                } else {
                    format!("{key}/")
                };
                require(
                    actual
                        .keys()
                        .filter(|path| path.starts_with(&prefix))
                        .eq(expected.keys().filter(|path| path.starts_with(&prefix))),
                    "source_changed",
                    "Stalker bundle membership changed",
                )?;
            }
        } else {
            let removed = self.removed_members(text(plan, "id"))?;
            let mut discarded = BTreeMap::new();
            for item in plan["discarded"].as_array().into_iter().flatten() {
                for (path, stamp) in item["stamps"].as_object().into_iter().flatten() {
                    discarded.insert(path.clone(), stamp.clone());
                }
            }
            let archived: BTreeSet<String> = plan["already_archived"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p.as_str().map(str::to_owned))
                .collect();
            let known = |p: &String| {
                expected.contains_key(p) || discarded.contains_key(p) || archived.contains(p)
            };
            require(
                (delivery || actual.keys().all(known))
                    && expected
                        .keys()
                        .all(|p| actual.contains_key(p) || removed.contains(p)),
                "source_changed",
                "Source membership changed; new/missing files require review",
            )?;
            for (path, stamp) in &discarded {
                require(
                    actual.get(path).is_none_or(|f| &f["stamp"] == stamp),
                    "source_changed",
                    format!("Discarded file changed: {path}"),
                )?;
            }
        }
        for (path, file) in &expected {
            if let Some(found) = actual.get(path) {
                require(
                    found["stamp"] == file["stamp"],
                    "source_changed",
                    format!("Source changed: {path}"),
                )?;
            }
        }
        Ok(actual)
    }

    /// Python `report`: progress for observers, written at most once per second (`progress:<job>`).
    fn report(&mut self, force: bool, changes: Value) -> Result<()> {
        let Some(progress) = self.progress.as_mut() else {
            return Ok(());
        };
        for (k, v) in changes.as_object().into_iter().flatten() {
            progress.insert(k.clone(), v.clone());
        }
        let now = Instant::now();
        if !force && now.duration_since(self.flushed).as_secs_f64() < 1.0 {
            return Ok(());
        }
        let elapsed = now.duration_since(self.started).as_secs_f64().max(0.001);
        let n = |p: &Map<String, Value>, k: &str| p.get(k).and_then(Value::as_u64).unwrap_or(0);
        let copied = n(progress, "copied_bytes");
        progress.insert("updated_at".into(), json!(nu9ve_time::now_utc()));
        progress.insert(
            "bytes_per_second".into(),
            json!((copied as f64 / elapsed) as u64),
        );
        if progress.contains_key("global_total") {
            let measured = copied.min(n(progress, "bytes_total"))
                + n(progress, "verified_bytes").min(n(progress, "verify_total"))
                + n(progress, "cleanup_bytes");
            let done = n(progress, "global_done").max(measured);
            progress.insert("global_done".into(), json!(done));
            progress.insert(
                "global_fraction".into(),
                json!((done as f64 / n(progress, "global_total").max(1) as f64).min(0.999)),
            );
            let remaining = n(progress, "backup_budget").saturating_sub(measured);
            let eta = if elapsed >= 3.0 && measured > 0 && remaining > 0 {
                json!((remaining as f64 * elapsed / measured as f64) as u64)
            } else {
                Value::Null
            };
            progress.insert("eta_seconds".into(), eta);
            let scope = if n(progress, "release_budget") > 0 {
                "backup"
            } else {
                "operation"
            };
            progress.insert("eta_scope".into(), json!(scope));
            self.meter.reach(done);
        }
        for (k, v) in self.meter.fields() {
            progress.insert(k, v);
        }
        progress.insert(
            "files_done".into(),
            progress.get("files_done").cloned().unwrap_or(json!(0)),
        );
        let snapshot = Value::Object(progress.clone());
        self.flushed = now;
        self.catalog.set(
            &format!("progress:{}", text(&snapshot, "job_id")),
            &snapshot,
        )
    }

    fn add_progress(&mut self, key: &str, amount: u64) {
        if let Some(p) = self.progress.as_mut() {
            let current = p.get(key).and_then(Value::as_u64).unwrap_or(0);
            p.insert(key.into(), json!(current + amount));
        }
    }

    /// Python `copy_many`: one source read fans out to every target's `.name.ncpartial`; each is verified, then
    /// renamed without replacing anything. Returns `(sha256, stamp)` per target.
    pub fn copy_many(
        &mut self,
        source: &Root,
        source_rel: &str,
        targets: &[Target<'_>],
        expected: Option<&str>,
        source_stamp: Option<&Value>,
    ) -> Result<Vec<(String, Value)>> {
        for t in targets {
            self.check_volume(t.record, false)?;
            t.root.check()?;
        }
        source.check()?;
        let mut src = fs::uncached(source.open_read(source_rel)?.ok_or_else(|| {
            Error::new(
                "operation_failed",
                format!("[FileNotFoundError] {source_rel}"),
            )
        })?);
        let initial = source.stat(source_rel)?.ok_or_else(|| {
            Error::new(
                "operation_failed",
                format!("[FileNotFoundError] {source_rel}"),
            )
        })?;
        let initial_json = stamp_json(&initial);
        if let Some(stamp) = source_stamp {
            require(
                &initial_json == stamp,
                "source_changed",
                "Source file changed before reading",
            )?;
        }
        let unchanged = |root: &Root| -> Result<()> {
            let now = root.stat(source_rel)?.map(|s| stamp_json(&s));
            require(
                now.as_ref() == Some(&initial_json),
                "source_changed",
                "Source changed during copy",
            )
        };
        let mut results: Vec<Option<(String, Value)>> = vec![None; targets.len()];
        let mut wanted = expected.map(str::to_owned);
        for (i, t) in targets.iter().enumerate() {
            let Some(existing) = t.root.open_read(&t.rel)? else {
                continue;
            };
            // Identical bytes (an earlier run or ingest) are adopted; anything else is never touched.
            if wanted.is_none() {
                wanted = Some(hash_file(&mut src, |_| {})?);
            }
            let mut verified = 0;
            let mut existing = fs::verify_read(existing);
            let found = hash_file(&mut existing, |n| verified += n)?;
            self.verify_cache = self.verify_cache.max(existing.cache());
            self.add_progress("verified_bytes", verified);
            require(
                Some(&found) == wanted.as_ref(),
                "destination_conflict",
                format!("A different file already exists at {}", t.rel),
            )?;
            unchanged(source)?;
            let stamp = t
                .root
                .stat(&t.rel)?
                .map(|s| stamp_json(&s))
                .unwrap_or(Value::Null);
            results[i] = Some((found, stamp));
        }
        let pending: Vec<usize> = (0..targets.len())
            .filter(|i| results[*i].is_none())
            .collect();
        if pending.is_empty() {
            return Ok(results.into_iter().map(Option::unwrap).collect());
        }
        let partials: HashMap<usize, String> = pending
            .iter()
            .map(|i| (*i, partial_of(&targets[*i].rel)))
            .collect();
        for i in &pending {
            let t = &targets[*i];
            match t.root.unlink(&partials[i], None) {
                Ok(()) => {}
                Err(e) if crate::tree::absent(&e) => {}
                Err(e) => return Err(e),
            }
        }
        let mut outputs: Vec<(usize, std::fs::File)> = Vec::new();
        for i in &pending {
            outputs.push((*i, fs::uncached(targets[*i].root.create_new(&partials[i])?)));
        }
        src.seek(SeekFrom::Start(0))
            .map_err(|e| fs::io_error(e, &source.path))?;
        let mut hasher = Sha256::new();
        let mut read = 0u64;
        let mut block = vec![0u8; CHUNK_SIZE];
        let mut next = vec![0u8; CHUNK_SIZE];
        let mut filled = src
            .read(&mut block)
            .map_err(|e| fs::io_error(e, &source.path))?;
        loop {
            self.checkpoint("copy_block", &json!({"path": source_rel}))?;
            if filled == 0 {
                break;
            }
            hasher.update(&block[..filled]);
            // Every destination is written in parallel while the next block is read (two disks, two writers).
            let (data, size) = (&block[..filled], filled);
            let reading = std::thread::scope(|scope| -> Result<usize> {
                let writers: Vec<_> = outputs
                    .iter_mut()
                    .map(|(_, file)| scope.spawn(move || write_all(file, data)))
                    .collect();
                let got = src
                    .read(&mut next)
                    .map_err(|e| fs::io_error(e, &source.path));
                for writer in writers {
                    writer
                        .join()
                        .map_err(|_| Error::new("write_failed", "Writer thread failed"))??;
                }
                got
            })?;
            read += size as u64;
            self.add_progress("copied_bytes", size as u64);
            let total = initial.size;
            self.report(false, json!({"file_done": read, "file_total": total}))?;
            std::mem::swap(&mut block, &mut next);
            filled = reading;
        }
        // Python `finish_partial`: the copy keeps the source times, then is flushed.
        let meta = src.metadata().map_err(|e| fs::io_error(e, &source.path))?;
        let mut times = std::fs::FileTimes::new();
        if let Ok(modified) = meta.modified() {
            times = times.set_modified(modified);
        }
        if let Ok(accessed) = meta.accessed() {
            times = times.set_accessed(accessed);
        }
        for (_, file) in &outputs {
            file.set_times(times)
                .map_err(|e| fs::io_error(e, &source.path))?;
            fs::full_sync(file)?;
        }
        drop(outputs);
        for i in &pending {
            targets[*i].root.sync_parent(&partials[i])?;
        }
        unchanged(source)?;
        let sha = nu9ve_json::hex(&hasher.finalize());
        require(
            expected.is_none_or(|e| e == sha),
            "source_changed",
            "Copy source differs from recorded content",
        )?;
        for i in &pending {
            let payload =
                json!({"path": partials[i], "root": targets[*i].root.path.display().to_string()});
            self.checkpoint("before_verify", &payload)?;
        }
        if self.progress.is_some() {
            self.meter.current.insert("action".into(), json!("verify"));
            self.report(false, json!({"phase": "verifying"}))?;
        }
        // Read every partial back in parallel and hash it, from the device rather than the pages just written.
        let checks: Vec<Result<(String, u64, fs::Cache)>> = std::thread::scope(|scope| {
            let jobs: Vec<_> = pending
                .iter()
                .map(|i| {
                    let (root, partial) = (targets[*i].root, &partials[i]);
                    scope.spawn(move || -> Result<(String, u64, fs::Cache)> {
                        let mut file =
                            fs::verify_read(root.open_read(partial)?.ok_or_else(|| {
                                Error::new(
                                    "checksum_mismatch",
                                    format!("Partial vanished: {partial}"),
                                )
                            })?);
                        let size = file
                            .metadata()
                            .map_err(|e| fs::io_error(e, &root.path))?
                            .len();
                        let hash = hash_file(&mut file, |_| {})?;
                        Ok((hash, size, file.cache()))
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|j| {
                    j.join()
                        .unwrap_or_else(|_| Err(Error::new("checksum_mismatch", "Verifier failed")))
                })
                .collect()
        });
        for (k, i) in pending.iter().enumerate() {
            let (hash, size, cache) = checks[k].clone()?;
            self.verify_cache = self.verify_cache.max(cache);
            self.add_progress("verified_bytes", size);
            require(
                hash == sha && size == initial.size,
                "checksum_mismatch",
                format!("Destination verification failed: {}", targets[*i].rel),
            )?;
        }
        for i in &pending {
            let t = &targets[*i];
            self.check_volume(t.record, false)?;
            t.root.check()?;
            t.root.rename_new(&partials[i], &t.rel)?;
            // Some filesystems (exFAT) derive file ids from the directory entry: stamp the final name.
            let stamp = t.root.stat(&t.rel)?.ok_or_else(|| {
                Error::new(
                    "checksum_mismatch",
                    format!("Destination vanished: {}", t.rel),
                )
            })?;
            require(
                stamp.size == initial.size,
                "checksum_mismatch",
                format!("Destination size changed: {}", t.rel),
            )?;
            results[*i] = Some((sha.clone(), stamp_json(&stamp)));
        }
        if self.progress.is_some() {
            self.meter.current.insert("action".into(), json!("copy"));
            self.report(false, json!({"phase": "copying"}))?;
        }
        Ok(results.into_iter().map(Option::unwrap).collect())
    }

    fn job(&self, id: &str) -> Result<Value> {
        crate::reads::job(&Catalog::open(&self.home)?, &json!({"job_id": id}))
    }

    /// Python `Transfer.apply(plan, expected_digest)`. The caller holds the writer lock.
    pub fn apply(&mut self, plan: &Value, expected_digest: &str) -> Result<Value> {
        validate_plan(plan)?;
        require(
            expected_digest == text(plan, "digest"),
            "plan_changed",
            "Apply requires the reviewed plan digest",
        )?;
        require(
            plan["release_stalker_source"] != true || cfg!(unix),
            "not_ported",
            "Stalker release after ingest requires Clipper exclusion on Unix",
        )?;
        let reader = Catalog::open(&self.home)?;
        crate::planner::check_retitle(&reader, plan)?;
        if plan["source_kind"] == "delivery" && plan["cleanup"] == true {
            self.check_finals_consumers(plan)?;
        }
        let id = text(plan, "id").to_owned();
        let reviewed = self.catalog.get(&format!("reviewed-plan:{id}"))?;
        require(
            reviewed.as_ref().is_none_or(|r| r["cancelled"] != true),
            "plan_cancelled",
            "Review a new plan before applying",
        )?;
        let existing: Option<(String, String)> = self
            .catalog
            .db()
            .query_row("SELECT digest,state FROM jobs WHERE id=?", [&id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
            .map_err(sql)?;
        require(
            existing.as_ref().is_none_or(|(d, _)| d == expected_digest),
            "job_conflict",
            "Job ID reused with another plan",
        )?;
        if let Some((_, state)) = &existing
            && FINAL_STATES.contains(&state.as_str())
        {
            // Returning a prior receipt does not repeat destructive actions.
            return self.job(&id);
        }
        preflight(
            &Catalog::open(&self.home)?,
            self.backend,
            plan,
            existing.is_none(),
        )?;
        if existing.is_none() {
            let now = nu9ve_time::now_utc();
            let document = String::from_utf8(canonical(plan)).unwrap_or_default();
            self.catalog
                .db()
                .execute(
                    "INSERT INTO jobs VALUES(?,?,?,?,?,?,?)",
                    (
                        &id,
                        expected_digest,
                        document,
                        "planned",
                        None::<String>,
                        &now,
                        &now,
                    ),
                )
                .map_err(sql)?;
        }
        match self.run(plan) {
            Ok(job) => Ok(job),
            Err(error) => {
                let code = fs::python_exception(&error)
                    .map(str::to_owned)
                    .unwrap_or_else(|| error.code.clone());
                let state = if code == "cancelled" {
                    "paused"
                } else {
                    "failed"
                };
                self.catalog.state(&id, state, Some(&code))?;
                Err(error)
            }
        }
    }

    fn check_finals_consumers(&self, plan: &Value) -> Result<()> {
        let guard = self.clipper_guard.ok_or_else(|| {
            Error::new(
                "not_ported",
                "Published finals cleanup requires Clipper exclusion",
            )
        })?;
        let report = guard.inspect(&[(text(plan, "source").to_owned(), None)])?;
        let blockers = report["blockers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(3)
            .collect::<Vec<_>>();
        require(
            blockers.is_empty(),
            "source_in_use",
            format!("Clipper uses these finals: {}", blockers.join("; ")),
        )
    }

    fn member_sha(&self, job: &str, path: &str) -> Result<Option<String>> {
        self.catalog
            .db()
            .query_row(
                "SELECT sha256 FROM members WHERE job_id=? AND relative_path=?",
                [job, path],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)
    }

    fn run(&mut self, plan: &Value) -> Result<Value> {
        self.validate_source(plan)?;
        let id = text(plan, "id").to_owned();
        let units: BTreeMap<String, Value> = plan["units"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|u| (text(u, "id").to_owned(), u.clone()))
            .collect();
        let mut records = Vec::new();
        if let Some(work) = plan["work_id"].as_str() {
            records.push(self.catalog.volume(work)?);
        }
        records.push(self.catalog.volume(text(plan, "archive_id"))?);
        let files: Vec<Value> = plan["files"].as_array().cloned().unwrap_or_default();
        let size = |f: &Value| f["stamp"]["size"].as_u64().unwrap_or(0);
        let unit_of = |f: &Value| {
            units
                .get(text(f, "unit_id"))
                .cloned()
                .unwrap_or(Value::Null)
        };
        let dest = |record: &Value, f: &Value| {
            destination_path(record, plan, &unit_of(f), text(f, "path"), true)
        };
        self.catalog.state(&id, "copying", None)?;
        let total: u64 = files.iter().map(size).sum();
        let mut destinations = Vec::new();
        let mut verify_total = 0u64;
        for record in &records {
            let mut count = 0;
            for f in &files {
                if dest(record, f)?.is_some() {
                    count += 1;
                    verify_total += size(f);
                }
            }
            if count > 0 {
                destinations.push(
                    json!({"id": record["id"], "name": record["name"], "role": record["role"],
                                         "files_total": count, "files_done": 0}),
                );
            }
        }
        let sony = plan["cleanup_profile"] == "sony_clips_then_format";
        let cleanable = |f: &&Value| !sony_index_member(text(f, "path")) || !sony;
        let cleanup = plan["cleanup"] == true;
        let cleanup_total: u64 = if cleanup {
            files.iter().filter(cleanable).map(size).sum()
        } else {
            0
        };
        let backup_budget = total + verify_total + cleanup_total;
        let release_budget: u64 = if plan["release_stalker_source"] == true {
            files
                .iter()
                .filter(|file| unit_of(file)["kind"] == "stalker_session")
                .map(size)
                .sum()
        } else {
            0
        };
        let previous = self
            .catalog
            .get(&format!("progress:{id}"))?
            .unwrap_or(json!({}));
        let previous_done = previous["global_done"].as_u64().unwrap_or(0);
        self.progress = Some(
            json!({"job_id": id, "phase": "copying", "files_total": files.len(), "files_done": 0, "bytes_total": total,
                   "copied_bytes": 0, "verified_bytes": 0, "verify_total": verify_total, "destinations": destinations,
                   "global_total": backup_budget + release_budget, "backup_budget": backup_budget, "release_budget": release_budget,
                   "cleanup_bytes": 0, "global_done": previous_done})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        );
        let source_name = Path::new(text(plan, "source"))
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let target_names = records
            .iter()
            .map(|r| text(r, "name"))
            .collect::<Vec<_>>()
            .join(" + ");
        let cleanable_count = if cleanup_total > 0 {
            files.iter().filter(cleanable).count()
        } else {
            0
        };
        self.meter = Meter {
            total: backup_budget,
            files_total: files.len() + cleanable_count,
            ..Meter::default()
        };
        self.meter.reach(previous_done);
        self.started = Instant::now();
        self.report(true, json!({}))?;
        let source = Root::open(Path::new(text(plan, "source")))?;
        for record in &records {
            self.check_volume(record, true)?;
        }
        let roots: Vec<Root> = records
            .iter()
            .map(|r| Root::open(Path::new(text(r, "root"))))
            .collect::<Result<_>>()?;
        let mut claimed = std::collections::BTreeSet::new();
        let mut receipts: Vec<Vec<Value>> = vec![Vec::new(); records.len()];
        let mut verified_in_run: Vec<(usize, String, Value, String)> = Vec::new();
        for file in &files {
            let path = text(file, "path").to_owned();
            self.checkpoint("before_file", file)?;
            self.meter
                .holding(&path, &source_name, &target_names, "copy");
            let mut expected = self.member_sha(&id, &path)?;
            let mut done: BTreeMap<usize, (String, String, Value)> = BTreeMap::new();
            let mut pending: Vec<(usize, String)> = Vec::new();
            for (r, record) in records.iter().enumerate() {
                let Some(target) = dest(record, file)? else {
                    continue;
                };
                if plan.get("layout").is_none() {
                    let (prefix, _) = target.split_once(&format!("/{id}/")).ok_or_else(|| {
                        Error::new("invalid_plan", "Missing destination job folder")
                    })?;
                    let base = format!("{prefix}/{id}");
                    if claimed.insert((r, base.clone())) {
                        crate::restore::claim_directory(
                            &roots[r],
                            &base,
                            &json!({
                                "job_id": id, "digest": plan["digest"], "volume_id": record["id"],
                            }),
                        )?;
                    }
                }
                let prior: Option<String> = self
                    .catalog
                    .db()
                    .query_row(
                        "SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?",
                        [&id, text(record, "id"), &path],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sql)?;
                if let Some(prior) = prior {
                    let data: Value = serde_json::from_str(&prior)
                        .map_err(|e| Error::new("catalog_corrupt", e.to_string()))?;
                    let mut fd = roots[r].open_read(&target)?.ok_or_else(|| {
                        Error::new("operation_failed", format!("[FileNotFoundError] {target}"))
                    })?;
                    let mut counted = 0;
                    let found = hash_file(&mut fd, |n| counted += n)?;
                    self.add_progress("verified_bytes", counted);
                    require(
                        found == text(&data, "sha256"),
                        "checksum_mismatch",
                        "Previously verified copy is corrupt",
                    )?;
                    let stamp = roots[r]
                        .stat(&target)?
                        .map(|s| stamp_json(&s))
                        .unwrap_or(Value::Null);
                    done.insert(r, (target, found, stamp));
                } else {
                    pending.push((r, target));
                }
            }
            if !pending.is_empty() {
                // After an interruption, a verified replica from this job is the source, not the card a second time.
                let previous: Option<String> = self
                    .catalog
                    .db()
                    .query_row(
                        "SELECT payload FROM copies WHERE job_id=? AND relative_path=? LIMIT 1",
                        [&id, &path],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sql)?;
                let targets: Vec<Target<'_>> = pending
                    .iter()
                    .map(|(r, rel)| Target {
                        root: &roots[*r],
                        rel: rel.clone(),
                        record: &records[*r],
                    })
                    .collect();
                let outcomes = match previous {
                    Some(previous) => {
                        let previous: Value = serde_json::from_str(&previous)
                            .map_err(|e| Error::new("catalog_corrupt", e.to_string()))?;
                        let volume = self.catalog.volume(text(&previous, "volume_id"))?;
                        crate::volumes::check(self.backend, &volume)?;
                        let tree = Root::open(Path::new(text(&volume, "root")))?;
                        self.copy_many(
                            &tree,
                            text(&previous, "destination"),
                            &targets,
                            expected.as_deref(),
                            None,
                        )?
                    }
                    None => self.copy_many(
                        &source,
                        &path,
                        &targets,
                        expected.as_deref(),
                        Some(&file["stamp"]),
                    )?,
                };
                for ((r, rel), (sha, stamp)) in pending.iter().zip(outcomes) {
                    require(
                        expected.as_ref().is_none_or(|e| *e == sha),
                        "checksum_mismatch",
                        "Replica hashes differ",
                    )?;
                    expected = Some(sha.clone());
                    done.insert(*r, (rel.clone(), sha, stamp));
                }
                self.catalog
                    .db()
                    .execute(
                        "INSERT OR IGNORE INTO members(job_id,relative_path,sha256) VALUES(?,?,?)",
                        [&id, &path, expected.as_deref().unwrap_or_default()],
                    )
                    .map_err(sql)?;
            }
            for (r, record) in records.iter().enumerate() {
                let Some((target, sha, stamp)) = done.get(&r) else {
                    continue;
                };
                let receipt = json!({"job_id": id, "volume_id": record["id"], "path": path, "destination": target,
                    "sha256": sha, "bytes": file["stamp"]["size"], "verified_at": nu9ve_time::now_utc(), "stamp": stamp,
                    "receipt_id": uuid5(&NAMESPACE_URL, &format!("{id}:{}", text(record, "id")))});
                let payload = String::from_utf8(canonical(&receipt)).unwrap_or_default();
                self.catalog
                    .db()
                    .execute(
                        "INSERT OR REPLACE INTO copies VALUES(?,?,?,?,?)",
                        [&id, text(record, "id"), &path, sha, &payload],
                    )
                    .map_err(sql)?;
                receipts[r].push(receipt.clone());
                verified_in_run.push((r, target.clone(), stamp.clone(), path.clone()));
                self.checkpoint("after_file_verified", &receipt)?;
                if let Some(d) = destinations.iter_mut().find(|d| d["id"] == record["id"]) {
                    d["files_done"] = json!(d["files_done"].as_u64().unwrap_or(0) + 1);
                }
            }
            if pending.is_empty() {
                self.add_progress("copied_bytes", size(file));
            }
            let names = records
                .iter()
                .enumerate()
                .filter(|(r, _)| done.contains_key(r))
                .map(|(_, rec)| text(rec, "name"))
                .collect::<Vec<_>>()
                .join(" + ");
            self.meter.finished("copied", &path, &names);
            let files_done = self
                .progress
                .as_ref()
                .and_then(|p| p["files_done"].as_u64())
                .unwrap_or(0)
                + 1;
            self.report(
                false,
                json!({"files_done": files_done, "file": path, "destinations": destinations}),
            )?;
        }
        let support = plan["units"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|u| truthy(&u["support"]));
        let mut receipt_units = Vec::new();
        for unit in plan["units"].as_array().into_iter().flatten() {
            let mut u = unit.clone();
            u["project_id"] = if truthy(&unit["support"]) {
                Value::Null
            } else {
                json!(
                    self.catalog
                        .project_for(text(unit, "profile"), text(unit, "project"))?
                )
            };
            receipt_units.push(u);
        }
        for (r, record) in records.iter().enumerate() {
            self.check_volume(record, false)?;
            self.check_volume(record, true)?;
            let schema = if support {
                "nu9ve-transfer-receipt/2"
            } else {
                "nu9ve-transfer-receipt/1"
            };
            let mut receipt = json!({"schema": schema, "receipt_id": uuid5(&NAMESPACE_URL, &format!("{id}:{}", text(record, "id"))),
                    "job_id": id, "plan_digest": plan["digest"], "volume_id": record["id"],
                    "verified_at": nu9ve_time::now_utc(), "files": receipts[r], "units": receipt_units, "plan": plan,
                    "country": plan["country"], "city": plan["city"], "work_delivery_status": plan["work_delivery_status"]});
            // Only a weaker verification is written, so a receipt stays byte-compatible with Python's otherwise.
            if let Some(cache) = self.weak_verification() {
                receipt["verify_cache"] = json!(cache);
            }
            roots[r].write_json(&format!(".ncreator/receipts/{id}.json"), &receipt)?;
        }
        self.checkpoint("after_receipts", plan)?;
        self.validate_source(plan)?;
        self.catalog.state(&id, "backup_verified", None)?;
        self.index(plan)?;
        let final_state = if !cleanup {
            "source_retained".to_owned()
        } else if plan["cleanup_profile"] == "camera_action_required" {
            "camera_action_required".to_owned()
        } else {
            self.clean(
                plan,
                &source,
                &records,
                &roots,
                &files,
                &verified_in_run,
                &source_name,
            )?
        };
        self.catalog.state(&id, &final_state, None)?;
        self.report(
            true,
            json!({"phase": final_state, "destinations": destinations}),
        )?;
        let total = self.meter.total;
        self.meter.reach(total);
        self.meter.current = Map::new();
        if let Some(p) = self.progress.as_mut() {
            p.insert("global_done".into(), json!(total));
            p.insert(
                "global_fraction".into(),
                json!(if release_budget == 0 {
                    1.0
                } else {
                    total as f64 / (backup_budget + release_budget).max(1) as f64
                }),
            );
            p.insert("eta_seconds".into(), Value::Null);
            for (k, v) in self.meter.fields() {
                p.insert(k, v);
            }
            let snapshot = Value::Object(p.clone());
            self.catalog.set(&format!("progress:{id}"), &snapshot)?;
        }
        self.job(&id)
    }

    /// The cleanup part of `_run`: verified copies unchanged, each original re-read and hashed like its copies,
    /// then removed (journaled `-1` before, `1` after); discards last; a final scan says what is left.
    #[allow(clippy::too_many_arguments)]
    fn clean(
        &mut self,
        plan: &Value,
        source: &Root,
        records: &[Value],
        roots: &[Root],
        files: &[Value],
        verified_in_run: &[(usize, String, Value, String)],
        source_name: &str,
    ) -> Result<String> {
        let id = text(plan, "id").to_owned();
        self.catalog.state(&id, "cleaning_source", None)?;
        self.report(true, json!({"phase": "cleaning"}))?;
        self.checkpoint("before_cleanup", plan)?;
        self.validate_source(plan)?;
        // Copies verified during this run: rehash only those whose stamps changed.
        for (r, target, stamp, _) in verified_in_run {
            self.check_volume(&records[*r], false)?;
            let current = roots[*r].stat(target)?.map(|s| stamp_json(&s));
            if current.as_ref() != Some(stamp) {
                let sha: Option<String> = self
                    .catalog
                    .db()
                    .query_row(
                        "SELECT sha256 FROM copies WHERE job_id=? AND volume_id=? AND json_extract(payload,'$.destination')=?",
                        [&id, text(&records[*r], "id"), target],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sql)?;
                let mut fd = roots[*r].open_read(target)?.ok_or_else(|| {
                    Error::new("operation_failed", format!("[FileNotFoundError] {target}"))
                })?;
                let found = hash_file(&mut fd, |_| {})?;
                require(
                    sha.as_deref() == Some(found.as_str()),
                    "checksum_mismatch",
                    "Verified destination changed before cleanup",
                )?;
            }
        }
        let mut per_source: BTreeMap<&str, Vec<(usize, &str, &Value)>> = BTreeMap::new();
        for (r, target, stamp, original) in verified_in_run {
            per_source
                .entry(original.as_str())
                .or_default()
                .push((*r, target.as_str(), stamp));
        }
        let sony = plan["cleanup_profile"] == "sony_clips_then_format";
        let removable: Vec<&Value> = files
            .iter()
            .filter(|f| !sony || !sony_index_member(text(f, "path")))
            .collect();
        for file in &removable {
            let path = text(file, "path");
            self.checkpoint("cleanup_file", file)?;
            if plan["source_kind"] == "delivery" {
                self.check_finals_consumers(plan)?;
            }
            self.meter.holding(path, source_name, "", "release");
            self.source_check(plan, false)?;
            for record in records {
                self.check_volume(record, false)?;
            }
            for (r, target, stamp) in per_source.get(path).cloned().unwrap_or_default() {
                if roots[r].stat(target)?.map(|s| stamp_json(&s)).as_ref() != Some(stamp) {
                    let expected = self.member_sha(&id, path)?;
                    let mut fd = roots[r].open_read(target)?.ok_or_else(|| {
                        Error::new("operation_failed", format!("[FileNotFoundError] {target}"))
                    })?;
                    require(
                        expected.as_deref() == Some(hash_file(&mut fd, |_| {})?.as_str()),
                        "checksum_mismatch",
                        "Destination changed during cleanup",
                    )?;
                }
            }
            let cleaned: Option<i64> = self
                .catalog
                .db()
                .query_row(
                    "SELECT cleaned FROM members WHERE job_id=? AND relative_path=?",
                    [&id, path],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sql)?;
            let cleaned = cleaned
                .ok_or_else(|| Error::new("missing_receipt", "No verified member for cleanup"))?;
            match source.stat(path)? {
                None => require(
                    cleaned == -1 || cleaned == 1,
                    "source_changed",
                    "Unexpected missing source during cleanup",
                )?,
                Some(st) => {
                    require(
                        cleaned != 1,
                        "source_changed",
                        "New file appeared at a cleaned source path",
                    )?;
                    require(
                        stamp_json(&st) == file["stamp"],
                        "source_changed",
                        "Source changed before cleanup",
                    )?;
                    // Replicas may all descend from one card read; a fresh read must agree before the only original goes.
                    let expected = self.member_sha(&id, path)?;
                    let mut fd = fs::verify_read(source.open_read(path)?.ok_or_else(|| {
                        Error::new("operation_failed", format!("[FileNotFoundError] {path}"))
                    })?);
                    let mut counted = 0;
                    let found = hash_file(&mut fd, |n| counted += n)?;
                    self.verify_cache = self.verify_cache.max(fd.cache());
                    self.add_progress("cleanup_bytes", counted);
                    require(
                        expected.as_deref() == Some(found.as_str()),
                        "checksum_mismatch",
                        format!("Source re-read differs from verified copies: {path}"),
                    )?;
                    require(
                        source.stat(path)?.map(|s| stamp_json(&s)).as_ref() == Some(&file["stamp"]),
                        "source_changed",
                        "Source changed while re-reading",
                    )?;
                    drop(fd);
                    self.catalog
                        .db()
                        .execute(
                            "UPDATE members SET cleaned=-1 WHERE job_id=? AND relative_path=?",
                            [&id, path],
                        )
                        .map_err(sql)?;
                    source.unlink(path, Some(&file["stamp"]))?;
                }
            }
            self.catalog
                .db()
                .execute(
                    "UPDATE members SET cleaned=1 WHERE job_id=? AND relative_path=?",
                    [&id, path],
                )
                .map_err(sql)?;
            self.meter.finished("cleaned", path, source_name);
            self.report(false, json!({}))?;
        }
        // Discarded in the review: no copy by design. They leave the card only now, and only if unchanged.
        let removing: BTreeSet<String> = {
            let mut statement = self
                .catalog
                .db()
                .prepare("SELECT payload FROM events WHERE job_id=? AND kind='discard_removing'")
                .map_err(sql)?;
            let rows = statement
                .query_map([&id], |r| r.get::<_, String>(0))
                .map_err(sql)?;
            rows.filter_map(|r| r.ok())
                .filter_map(|p| serde_json::from_str::<Value>(&p).ok())
                .map(|v| text(&v, "path").to_owned())
                .collect()
        };
        for item in plan["discarded"].as_array().into_iter().flatten() {
            let stamps: BTreeMap<&String, &Value> =
                item["stamps"].as_object().into_iter().flatten().collect();
            for (path, stamp) in stamps {
                self.checkpoint("cleanup_discarded", &json!({"path": path}))?;
                self.source_check(plan, false)?;
                match source.stat(path)? {
                    None => {
                        require(
                            removing.contains(path),
                            "source_changed",
                            format!("A discarded file disappeared before cleanup: {path}"),
                        )?;
                        continue;
                    }
                    Some(st) => require(
                        &stamp_json(&st) == stamp,
                        "source_changed",
                        format!("A discarded file changed since review: {path}"),
                    )?,
                }
                self.catalog.event(
                    &id,
                    "discard_removing",
                    &json!({"path": path, "stamp": stamp}),
                )?;
                source.unlink(path, Some(stamp))?;
                self.catalog.event(
                    &id,
                    "discard_removed",
                    &json!({"path": path, "bytes": stamp["size"]}),
                )?;
                self.meter.finished("cleaned", path, source_name);
            }
        }
        if sony {
            self.report(true, json!({"phase": "checking_card"}))?;
            self.source_check(plan, true)?;
        }
        let remaining = scan(Path::new(text(plan, "source")), false, false, false)?;
        if sony {
            self.source_check(plan, true)?;
        }
        // Planned members are verified and cleaned; material that appeared meanwhile is not backed up.
        let kept: BTreeMap<&str, &Value> = files
            .iter()
            .filter(|f| !removable.iter().any(|r| r == f))
            .map(|f| (text(f, "path"), &f["stamp"]))
            .collect();
        let mut left: BTreeSet<String> = plan["already_archived"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect();
        for d in plan["discarded"].as_array().into_iter().flatten() {
            left.extend(
                d["paths"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| p.as_str().map(str::to_owned)),
            );
        }
        let unexpected = remaining.files.iter().any(|f| {
            kept.get(text(f, "path")) != Some(&&f["stamp"]) && !left.contains(text(f, "path"))
        });
        Ok(if unexpected {
            "source_has_new_material"
        } else if sony {
            "camera_action_required"
        } else {
            "completed"
        }
        .to_owned())
    }

    /// Python `_index`: assets, revisions, projects and work state of the verified units.
    pub(crate) fn index(&mut self, plan: &Value) -> Result<()> {
        let id = text(plan, "id");
        for unit in plan["units"].as_array().into_iter().flatten() {
            if truthy(&unit["support"]) {
                continue; // durable members/copies/receipts, not an editorial project
            }
            let mut hashes = Vec::new();
            for path in unit["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let sha = self
                    .member_sha(id, path)?
                    .ok_or_else(|| Error::new("missing_receipt", "Missing member receipt"))?;
                hashes.push(json!({"path": path, "sha256": sha}));
            }
            let project_id = self
                .catalog
                .project_for(text(unit, "profile"), text(unit, "project"))?;
            let mut asset = unit.clone();
            for (k, v) in [
                ("project_id", json!(project_id)),
                ("job_id", json!(id)),
                ("source", plan["source"].clone()),
                ("revision", json!(digest(&json!(hashes)))),
                ("country", plan["country"].clone()),
                ("city", plan["city"].clone()),
            ] {
                asset[k] = v;
            }
            let payload = String::from_utf8(canonical(&asset)).unwrap_or_default();
            let unit_id = text(unit, "id");
            let db = self.catalog.db();
            let tx = db.unchecked_transaction().map_err(sql)?;
            tx.execute(
                "INSERT OR IGNORE INTO asset_revisions VALUES(?,?,?)",
                [unit_id, text(&asset, "revision"), &payload],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT OR IGNORE INTO assets VALUES(?,?,?)",
                [unit_id, id, &payload],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT OR IGNORE INTO asset_projects VALUES(?,?)",
                [unit_id, &project_id],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT OR IGNORE INTO asset_jobs VALUES(?,?)",
                [unit_id, id],
            )
            .map_err(sql)?;
            if let (Some(session), Some(manifest)) = (
                unit["session_id"].as_str(),
                unit["capture_manifest_digest"].as_str(),
            ) {
                tx.execute(
                    "INSERT OR IGNORE INTO sessions VALUES(?,?,?)",
                    [session, manifest, unit_id],
                )
                .map_err(sql)?;
            }
            let current: String = tx
                .query_row("SELECT payload FROM assets WHERE id=?", [unit_id], |r| {
                    r.get(0)
                })
                .map_err(sql)?;
            let current: Value = serde_json::from_str(&current)
                .map_err(|e| Error::new("catalog_corrupt", e.to_string()))?;
            if current["revision"] == asset["revision"] {
                // Camera indexes/thumbnails have no work copy in a named layout; never leave them pending.
                let work_probe = json!({"role": "work", "prefix": "x"});
                let mut needs_work = false;
                for path in unit["paths"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    needs_work |= destination_path(&work_probe, plan, unit, path, true)?.is_some();
                }
                let waiting = unit["mission"]["state"] == "pending_export";
                let state = if needs_work {
                    text(plan, "work_delivery_status")
                } else if waiting {
                    "pending"
                } else {
                    "skipped"
                };
                let now = nu9ve_time::now_utc();
                tx.execute(
                    "INSERT OR IGNORE INTO work VALUES(?,?,?,?)",
                    (unit_id, state, plan["expected_work_id"].as_str(), &now),
                )
                .map_err(sql)?;
                if plan["work_delivery_status"] == "ready" && needs_work {
                    tx.execute(
                        "UPDATE work SET state=?,target=?,updated_at=? WHERE asset_id=?",
                        ("ready", plan["work_id"].as_str(), &now, unit_id),
                    )
                    .map_err(sql)?;
                }
            }
            tx.commit().map_err(sql)?;
        }
        Ok(())
    }
}

/// The `apply` RPC (Python `Service._apply_ingest` for cards): under the writer lock, apply the reviewed plan;
/// then the consumer's units map for drone missions and a metadata backup, whose failures never hide the ingest.
pub fn apply(
    home: &Path,
    backend: &dyn Backend,
    plan: &Value,
    reviewed_digest: &str,
    hook: Hook<'_>,
) -> Result<Value> {
    let plan = match plan {
        Value::String(text) => {
            require(
                text.len() <= 12 * 1024 * 1024,
                "plan_too_large",
                "Plan document exceeds 12 MiB",
            )?;
            serde_json::from_str(text).map_err(|e| Error::new("operation_failed", e.to_string()))?
        }
        other => other.clone(),
    };
    validate_plan(&plan)?;
    require(
        reviewed_digest == text(&plan, "digest"),
        "plan_changed",
        "Apply requires the reviewed plan digest",
    )?;
    let guard = if plan["source_kind"] == "delivery" && plan["cleanup"] == true && cfg!(unix) {
        Some(crate::clipper_release::Guard::acquire(home)?)
    } else {
        None
    };
    let lock = crate::catalog::writer_lock(home)?;
    let catalog = WritableCatalog::open(home)?;
    let mut transfer = Transfer::new(home, &catalog, backend, hook);
    transfer.clipper_guard = guard.as_ref();
    let mut result = transfer.apply(&plan, reviewed_digest)?;
    if let Some(cache) = transfer.weak_verification() {
        result["verify_cache"] = json!(cache);
    }
    drop(transfer);
    drop(lock);
    drop(guard);
    let state = text(&result, "state").to_owned();
    let missions = plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|u| truthy(&u["mission"]));
    if missions
        && matches!(
            state.as_str(),
            "source_retained" | "completed" | "camera_action_required"
        )
    {
        // The consumer's map (units.ndjson); a failure here must not hide a completed ingest.
        match crate::units_map::write_units_map(home, backend, &plan) {
            Ok(status) => result["units_map"] = json!(status),
            Err(e) => {
                result["units_map"] = json!("failed");
                result["units_map_error"] = json!(e.message);
            }
        }
    }
    if plan["release_stalker_source"] == true
        && matches!(state.as_str(), "source_retained" | "completed")
    {
        let summary = crate::release::release_stalker_sources(home, backend, &plan)?;
        let cache = result.get("verify_cache").cloned();
        result = crate::reads::job(&Catalog::open(home)?, &json!({"job_id": plan["id"]}))?;
        if let Some(cache) = cache {
            result["verify_cache"] = cache;
        }
        result["stalker_release"] = summary;
        if truthy(&result["stalker_release"]["released"]) {
            return Ok(result);
        }
    }
    // A metadata-backup failure must not hide a successfully completed ingest.
    match crate::units_map::backup_metadata(home, backend, text(&plan, "archive_id")) {
        Ok(receipt) => result["metadata_backup"] = receipt,
        Err(e) => result["metadata_backup_error"] = json!(e.message),
    }
    Ok(result)
}

/// The `resume` RPC: apply a job's own plan again; a recovered archive job is read-only.
pub fn resume(home: &Path, backend: &dyn Backend, job_id: &str) -> Result<Value> {
    let job = crate::reads::job(&Catalog::open(home)?, &json!({"job_id": job_id}))?;
    require(
        job["state"] != "recovered",
        "source_not_resumable",
        "Recovered archive jobs are read-only; use restore or deliver-work",
    )?;
    apply(
        home,
        backend,
        &job["plan"],
        text(&job, "digest"),
        &|_, _| Ok(()),
    )
}

/// Ports of tests/test_media_engine.py (MediaFixture): temporary card, archive and work with fixture identities.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::test_backend::{Setup, setup};
    use std::cell::Cell;

    fn fixture() -> Setup {
        let s = setup();
        std::fs::create_dir_all(s.card.join("DCIM")).unwrap();
        std::fs::write(s.card.join("DCIM/photo.JPG"), b"photo-data").unwrap();
        std::fs::write(s.card.join("DCIM/photo.DNG"), b"raw-data").unwrap();
        s
    }

    fn plan(s: &Setup, extra: Value) -> Value {
        let mut params = json!({"source": s.card.display().to_string(), "archive": "MEDIA", "work": "ESSD",
                                "project": "intro", "profile": "e64", "cleanup": true});
        for (k, v) in extra.as_object().unwrap() {
            params[k] = v.clone();
        }
        crate::planner::build_plan(&s.home, &s.backend, &params).unwrap()
    }

    /// Python `run_plan`: `Transfer(catalog, backend, cancel, hook).apply(plan, plan['digest'])`.
    fn run(s: &Setup, plan: &Value, hook: Hook<'_>) -> Result<Value> {
        let _lock = crate::catalog::writer_lock(&s.home)?;
        let catalog = WritableCatalog::open(&s.home)?;
        let mut transfer = Transfer::new(&s.home, &catalog, &s.backend, hook);
        let mut result = transfer.apply(plan, text(plan, "digest"))?;
        // As the `apply` RPC records it.
        if let Some(cache) = transfer.weak_verification() {
            result["verify_cache"] = json!(cache);
        }
        Ok(result)
    }

    fn ok() -> impl Fn(&str, &Value) -> Result<()> {
        |_: &str, _: &Value| Ok(())
    }

    fn os_error(message: &str) -> Error {
        Error::new("operation_failed", format!("[OSError] {message}"))
    }

    fn count(root: &Path, suffix: &str) -> usize {
        fn walk(dir: &Path, suffix: &str, n: &mut usize) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, suffix, n);
                } else if path.to_string_lossy().ends_with(suffix) {
                    *n += 1;
                }
            }
        }
        let mut n = 0;
        walk(root, suffix, &mut n);
        n
    }

    #[cfg(unix)]
    #[test]
    fn finals_cleanup_holds_clipper_exclusion_and_keeps_new_deliveries() {
        use std::os::unix::fs::PermissionsExt;
        let s = crate::volumes::test_backend::setup();
        let folder = s.work.join("002_finales/e64/subidos");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("26-11-12-final.mov"), b"final").unwrap();
        let clipper = s.home.join("clipper-fixture");
        std::fs::write(&clipper, r#"#!/bin/sh
echo '{"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],"projects":[],"offline_projects":[]}'
"#).unwrap();
        std::fs::set_permissions(&clipper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let catalog = WritableCatalog::open(&s.home).unwrap();
        catalog.set("clipper-executable", &json!(clipper)).unwrap();
        let plan = crate::planner::build_plan(&s.home, &s.backend, &json!({
            "source": folder, "archive": "MEDIA", "project": "finales", "profile": "e64",
            "archive_only": true, "confirm_archive_only": true, "work_decision": "skipped",
            "cleanup": true, "source_kind": "delivery", "finals_route": true, "camera": "generic",
        })).unwrap();
        let guard = crate::clipper_release::Guard::acquire_at(
            &s.home,
            s.home.with_file_name("clipper-home"),
        )
        .unwrap();
        let _lock = crate::catalog::writer_lock(&s.home).unwrap();
        let hook = |stage: &str, _: &Value| {
            if stage == "before_cleanup" {
                std::fs::write(folder.join("26-11-12-new.mov"), b"new").unwrap();
            }
            Ok(())
        };
        let mut transfer = Transfer::new(&s.home, &catalog, &s.backend, &hook);
        transfer.clipper_guard = Some(&guard);
        assert_eq!(
            transfer.apply(&plan, text(&plan, "digest")).unwrap()["state"],
            "source_has_new_material"
        );
        assert!(!folder.join("26-11-12-final.mov").exists());
        assert_eq!(
            std::fs::read(folder.join("26-11-12-new.mov")).unwrap(),
            b"new"
        );
        assert_eq!(
            std::fs::read(s.archive.join("002_finales/e64/2026/26-11-12-final.mov")).unwrap(),
            b"final"
        );
    }

    #[test]
    fn verified_both_destinations_then_cleanup_and_idempotent_retry() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let result = run(&s, &plan, &ok()).unwrap();
        assert_eq!(result["state"], "completed");
        assert_eq!(count(&s.card, ".JPG") + count(&s.card, ".DNG"), 0);
        // A local NTFS/APFS volume verifies from the device: nothing weaker is recorded (Python's shape).
        #[cfg(any(windows, target_os = "macos"))]
        assert!(result.get("verify_cache").is_none());
        for root in [&s.archive, &s.work] {
            assert_eq!(count(root, ".JPG"), 1);
            let receipt = root.join(format!(".ncreator/receipts/{}.json", text(&plan, "id")));
            assert!(receipt.is_file());
            #[cfg(any(windows, target_os = "macos"))]
            assert!(
                !std::fs::read_to_string(&receipt)
                    .unwrap()
                    .contains("verify_cache")
            );
        }
        assert_eq!(
            (count(&s.archive, ".DNG"), count(&s.work, ".DNG")),
            (1, 1),
            "generic stills: both copies"
        );
        let again = run(&s, &plan, &ok()).unwrap();
        assert_eq!(again["state"], "completed");
        assert_eq!(count(&s.archive, ".JPG"), 1);
        let library = crate::snapshot::library(&Catalog::open(&s.home).unwrap()).unwrap();
        assert_eq!(library.len(), 1, "JPG+DNG stay paired");
        assert_eq!(library[0]["work_delivery_status"], "ready");
        assert_eq!(result["progress"]["files_done"], 2);
        assert_eq!(result["progress"]["global_fraction"], 1.0);
        assert_eq!(count(&s.archive, ".ncpartial"), 0);
    }

    #[test]
    fn corruption_with_same_size_never_cleans_source() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let corrupt = |stage: &str, data: &Value| {
            if stage == "before_verify" {
                let path = Path::new(text(data, "root")).join(text(data, "path"));
                let size = std::fs::metadata(&path).unwrap().len() as usize;
                std::fs::write(&path, vec![b'x'; size]).unwrap();
            }
            Ok(())
        };
        let error = run(&s, &plan, &corrupt).unwrap_err();
        assert!(
            error.message.contains("verification failed"),
            "{}",
            error.message
        );
        assert_eq!(
            std::fs::read(s.card.join("DCIM/photo.JPG")).unwrap(),
            b"photo-data"
        );
        let job = crate::reads::job(
            &Catalog::open(&s.home).unwrap(),
            &json!({"job_id": plan["id"]}),
        )
        .unwrap();
        assert_eq!(
            (job["state"].as_str(), job["error"].as_str()),
            (Some("failed"), Some("checksum_mismatch"))
        );
    }

    #[test]
    fn interrupted_copy_and_a_crash_before_rename_resume_without_duplicates() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let fail = |stage: &str, _: &Value| {
            if stage == "after_file_verified" {
                Err(os_error("simulated disconnect"))
            } else {
                Ok(())
            }
        };
        assert!(run(&s, &plan, &fail).is_err());
        assert!(s.card.join("DCIM/photo.JPG").exists());
        let job = crate::reads::job(
            &Catalog::open(&s.home).unwrap(),
            &json!({"job_id": plan["id"]}),
        )
        .unwrap();
        assert_eq!(
            job["error"], "OSError",
            "the job records the Python exception name"
        );
        assert_eq!(run(&s, &plan, &ok()).unwrap()["state"], "completed");
        assert_eq!(count(&s.archive, ".JPG"), 1);
        assert_eq!(count(&s.archive, ".ncpartial"), 0);

        let s = fixture();
        let plan = super::tests::plan(&s, json!({}));
        let crash = |stage: &str, _: &Value| {
            if stage == "before_verify" {
                Err(os_error("power loss before rename"))
            } else {
                Ok(())
            }
        };
        assert!(run(&s, &plan, &crash).is_err());
        assert!(count(&s.work, ".ncpartial") > 0);
        assert_eq!(run(&s, &plan, &ok()).unwrap()["state"], "completed");
        assert_eq!(count(&s.work, ".ncpartial"), 0);
    }

    #[test]
    fn partial_cleanup_resumes_from_journal() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let seen = Cell::new(0);
        let fail = |stage: &str, _: &Value| {
            if stage == "cleanup_file" {
                seen.set(seen.get() + 1);
                if seen.get() == 2 {
                    return Err(os_error("power loss"));
                }
            }
            Ok(())
        };
        assert!(run(&s, &plan, &fail).is_err());
        assert_eq!(count(&s.card, ".JPG") + count(&s.card, ".DNG"), 1);
        assert_eq!(run(&s, &plan, &ok()).unwrap()["state"], "completed");
    }

    #[test]
    fn new_files_before_or_during_cleanup() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let card = s.card.clone();
        let add = |stage: &str, _: &Value| {
            if stage == "before_cleanup" {
                std::fs::write(card.join("new.mp4"), b"new recording").unwrap();
            }
            Ok(())
        };
        assert!(
            run(&s, &plan, &add)
                .unwrap_err()
                .message
                .contains("membership changed")
        );
        assert!(s.card.join("DCIM/photo.JPG").exists() && s.card.join("new.mp4").exists());

        let s = fixture();
        let plan = super::tests::plan(&s, json!({}));
        let card = s.card.clone();
        let add = |stage: &str, _: &Value| {
            if stage == "cleanup_file" && !card.join("new.mp4").exists() {
                std::fs::write(card.join("new.mp4"), b"new recording").unwrap();
            }
            Ok(())
        };
        assert_eq!(
            run(&s, &plan, &add).unwrap()["state"],
            "source_has_new_material"
        );
        assert!(!s.card.join("DCIM/photo.JPG").exists());
        assert_eq!(
            std::fs::read(s.card.join("new.mp4")).unwrap(),
            b"new recording"
        );
        assert_eq!(
            run(&s, &plan, &ok()).unwrap()["state"],
            "source_has_new_material",
            "terminal"
        );
        assert!(s.card.join("new.mp4").exists());
    }

    #[test]
    fn foreign_destinations_are_never_overwritten_and_identical_ones_are_adopted() {
        let s = fixture();
        let plan = plan(&s, json!({"cleanup": false}));
        let unit = &plan["units"][0];
        let file = &plan["files"][0];
        let work = Catalog::open(&s.home).unwrap().volume("ESSD").unwrap();
        let rel = destination_path(&work, &plan, unit, text(file, "path"), true)
            .unwrap()
            .unwrap();
        let target = s.work.join(&rel);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let size = file["stamp"]["size"].as_u64().unwrap() as usize;
        let mut other = b"other!".to_vec();
        other.resize(size, b'x');
        std::fs::write(&target, &other).unwrap();
        assert!(
            run(&s, &plan, &ok())
                .unwrap_err()
                .message
                .contains("different file")
        );
        assert!(std::fs::read(&target).unwrap().starts_with(b"other!"));
        assert!(s.card.join("DCIM/photo.JPG").exists());

        let s = fixture();
        let plan = super::tests::plan(&s, json!({}));
        let archive = Catalog::open(&s.home).unwrap().volume("MEDIA").unwrap();
        for file in plan["files"].as_array().unwrap() {
            let unit = plan["units"]
                .as_array()
                .unwrap()
                .iter()
                .find(|u| u["id"] == file["unit_id"])
                .unwrap();
            let rel = destination_path(&archive, &plan, unit, text(file, "path"), true)
                .unwrap()
                .unwrap();
            let target = s.archive.join(rel);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(s.card.join(text(file, "path")), target).unwrap();
        }
        assert_eq!(run(&s, &plan, &ok()).unwrap()["state"], "completed");
        assert_eq!(count(&s.archive, ".JPG"), 1);
    }

    #[test]
    fn a_changed_source_or_a_missing_destination_keeps_the_card() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        std::fs::write(s.card.join("DCIM/photo.JPG"), b"new version").unwrap();
        assert!(
            run(&s, &plan, &ok())
                .unwrap_err()
                .message
                .contains("changed")
        );
        assert_eq!(count(&s.archive, ".JPG"), 0);

        let s = fixture();
        let plan = super::tests::plan(&s, json!({}));
        let archive = s.archive.clone();
        let gone = archive.with_file_name("archive-away");
        let offline = |stage: &str, _: &Value| {
            if stage == "before_cleanup" {
                std::fs::rename(&archive, &gone).unwrap();
            }
            Ok(())
        };
        assert!(run(&s, &plan, &offline).is_err());
        assert!(s.card.join("DCIM/photo.JPG").exists());

        let s = fixture();
        let plan = super::tests::plan(&s, json!({}));
        let archive = s.archive.clone();
        let gone = archive.with_file_name("archive-away");
        let during = |stage: &str, _: &Value| {
            if stage == "cleanup_file" && archive.exists() {
                std::fs::rename(&archive, &gone).unwrap();
            }
            Ok(())
        };
        assert!(run(&s, &plan, &during).is_err());
        assert!(s.card.join("DCIM/photo.JPG").exists() && s.card.join("DCIM/photo.DNG").exists());
    }

    #[test]
    fn corruption_during_cleanup_blocks_deletion_of_the_affected_source() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let archive = Catalog::open(&s.home).unwrap().volume("MEDIA").unwrap();
        let root = s.archive.clone();
        let corrupt = |stage: &str, file: &Value| {
            if stage == "cleanup_file" {
                let unit = plan["units"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|u| u["id"] == file["unit_id"])
                    .unwrap();
                let rel = destination_path(&archive, &plan, unit, text(file, "path"), true)
                    .unwrap()
                    .unwrap();
                let path = root.join(rel);
                let size = std::fs::metadata(&path).unwrap().len() as usize;
                std::fs::write(&path, vec![b'x'; size + 1]).unwrap();
            }
            Ok(())
        };
        assert!(
            run(&s, &plan, &corrupt)
                .unwrap_err()
                .message
                .contains("changed during cleanup")
        );
        assert!(s.card.join("DCIM/photo.JPG").exists() && s.card.join("DCIM/photo.DNG").exists());
    }

    #[test]
    fn a_sony_card_loses_media_but_keeps_camera_indexes() {
        let s = fixture();
        let clip = s.card.join("PRIVATE/M4ROOT/CLIP");
        std::fs::create_dir_all(&clip).unwrap();
        std::fs::write(clip.join("C0001.MP4"), b"sony").unwrap();
        std::fs::write(s.card.join("PRIVATE/M4ROOT/MEDIAPRO.XML"), b"<index/>").unwrap();
        std::fs::create_dir_all(s.card.join("AVF_INFO")).unwrap();
        std::fs::write(s.card.join("AVF_INFO/AVIN0001.BNP"), b"db").unwrap();
        let plan = plan(&s, json!({}));
        assert_eq!(plan["cleanup_profile"], "sony_clips_then_format");
        assert_eq!(
            run(&s, &plan, &ok()).unwrap()["state"],
            "camera_action_required"
        );
        assert!(!clip.join("C0001.MP4").exists());
        assert!(
            !s.card.join("DCIM/photo.JPG").exists(),
            "stills in DCIM go too"
        );
        assert_eq!(
            std::fs::read(s.card.join("PRIVATE/M4ROOT/MEDIAPRO.XML")).unwrap(),
            b"<index/>"
        );
        assert!(s.card.join("AVF_INFO/AVIN0001.BNP").exists());
    }

    #[test]
    fn confirm_sony_cleanup_requires_a_changed_index_and_preserves_new_clips() {
        let s = fixture();
        let clip = s.card.join("PRIVATE/M4ROOT/CLIP/C0001.MP4");
        let index = s.card.join("PRIVATE/M4ROOT/MEDIAPRO.XML");
        std::fs::create_dir_all(clip.parent().unwrap()).unwrap();
        std::fs::write(&clip, b"sony").unwrap();
        std::fs::write(&index, b"<index/>").unwrap();
        let plan = plan(&s, json!({}));
        assert_eq!(
            run(&s, &plan, &ok()).unwrap()["state"],
            "camera_action_required"
        );
        let params = json!({"source": s.card, "job_id": plan["id"]});
        let confirm = || crate::sony::confirm(&s.home, &s.backend, &params);
        assert_eq!(confirm().unwrap_err().code, "card_not_formatted");
        std::fs::write(&index, b"<new-index/>").unwrap();
        std::fs::write(&clip, b"sony").unwrap();
        assert_eq!(confirm().unwrap_err().code, "source_not_clean");
        std::fs::write(&clip, b"new!").unwrap();
        assert_eq!(confirm().unwrap()["state"], "completed");
        assert_eq!(std::fs::read(&clip).unwrap(), b"new!");
    }

    #[test]
    fn sony_reconcile_distinguishes_removal_reappearance_and_format_after_failed_check() {
        let s = fixture();
        let rel = "PRIVATE/M4ROOT/CLIP/C0001.MP4";
        let clip = s.card.join(rel);
        let index = s.card.join("PRIVATE/M4ROOT/MEDIAPRO.XML");
        std::fs::create_dir_all(clip.parent().unwrap()).unwrap();
        std::fs::write(&clip, b"sony").unwrap();
        std::fs::write(&index, b"<index/>").unwrap();
        let plan = plan(&s, json!({}));
        assert_eq!(
            run(&s, &plan, &ok()).unwrap()["state"],
            "camera_action_required"
        );
        let id = text(&plan, "id");
        let writer = WritableCatalog::open(&s.home).unwrap();
        writer
            .state(id, "failed", Some("FileNotFoundError"))
            .unwrap();
        writer
            .set(
                &format!("progress:{id}"),
                &json!({"phase": "checking_card"}),
            )
            .unwrap();
        let catalog = Catalog::open(&s.home).unwrap();
        let job = crate::reads::jobs(&catalog).unwrap().remove(0);
        assert_eq!(job["sony_removal_recorded"], true);
        assert_eq!(job["sony_stage"], "removal_recorded");
        let params = json!({"source": s.card, "identity": crate::volumes::card_identity(&s.backend.describe(&s.card, true).unwrap())});
        let check = || crate::sony::reconcile(&s.home, &s.backend, &params).unwrap();
        assert_eq!(check()["checked"][id], "waiting_camera");
        assert_eq!(check()["updated"], json!([]));
        std::fs::write(&clip, b"sony").unwrap();
        std::fs::write(&index, b"<new-index/>").unwrap();
        let present = check();
        assert_eq!(present["checked"][id], "originals_present");
        assert_eq!(present["reappeared"][id], json!([rel]));
        std::fs::write(&clip, b"new!").unwrap();
        assert_eq!(check()["checked"][id], "ready");
        let completed = crate::reads::job(&catalog, &json!({"job_id": id})).unwrap();
        assert_eq!(completed["state"], "completed");
        assert_eq!(completed["progress"]["phase"], "completed");
        assert_eq!(std::fs::read(&clip).unwrap(), b"new!");
    }

    #[test]
    fn discarded_files_leave_the_card_only_after_the_kept_ones_are_verified() {
        let s = fixture();
        std::fs::write(s.card.join("DCIM/other.JPG"), b"discard me").unwrap();
        let plan = plan(
            &s,
            json!({"assignments": {"DCIM/other.JPG": {"discard": true}}}),
        );
        assert_eq!(run(&s, &plan, &ok()).unwrap()["state"], "completed");
        assert!(!s.card.join("DCIM/other.JPG").exists());
        assert_eq!(
            count(&s.archive, ".JPG"),
            1,
            "a discarded file is never copied"
        );
    }

    #[test]
    fn a_changed_or_cancelled_plan_and_a_busy_lock_are_refused() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let mut changed = plan.clone();
        changed["cleanup"] = json!(false);
        assert_eq!(run(&s, &changed, &ok()).unwrap_err().code, "plan_changed");
        let lock = crate::catalog::writer_lock(&s.home).unwrap();
        assert_eq!(
            super::apply(&s.home, &s.backend, &plan, text(&plan, "digest"), &ok())
                .unwrap_err()
                .code,
            "job_busy"
        );
        drop(lock);
        let catalog = WritableCatalog::open(&s.home).unwrap();
        catalog
            .set(
                &format!("reviewed-plan:{}", text(&plan, "id")),
                &json!({"id": plan["id"], "cancelled": true}),
            )
            .unwrap();
        drop(catalog);
        assert_eq!(run(&s, &plan, &ok()).unwrap_err().code, "plan_cancelled");
    }

    #[test]
    fn apply_backs_up_the_catalog_and_resume_finishes_a_failed_job() {
        let s = fixture();
        let plan = plan(&s, json!({}));
        let fail = |stage: &str, _: &Value| {
            if stage == "after_receipts" {
                Err(os_error("simulated"))
            } else {
                Ok(())
            }
        };
        assert!(super::apply(&s.home, &s.backend, &plan, text(&plan, "digest"), &fail).is_err());
        let resumed = super::resume(&s.home, &s.backend, text(&plan, "id")).unwrap();
        assert_eq!(resumed["state"], "completed");
        let backup = &resumed["metadata_backup"];
        assert_eq!(backup["schema"], "nu9ve-catalog-backup/1");
        let copy = s.archive.join(text(backup, "path"));
        assert!(
            copy.is_file()
                && s.archive
                    .join(format!("{}.json", text(backup, "path")))
                    .is_file()
        );
        let check: String = rusqlite::Connection::open(&copy)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .map(|n| n.to_string())
            .unwrap();
        assert_eq!(check, "1");
    }
}
