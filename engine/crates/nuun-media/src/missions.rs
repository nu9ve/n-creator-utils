//! Matching drone stills to the archive's mission registry (Atlas A12). Port of the read side of
//! `media/missions.py`: `parse_date`, `MissionIndex` (records by original name, sessions, coverage), `match`,
//! `likely_mission`, `meters` and `index_for`. Review/incorporate of exports and `units.ndjson` stay in Python.
//!
//! Times are civil like Python's: camera wall clocks are naive local times, `capturedAt` (UTC) becomes local with
//! the host zone (`nu9ve-time`, CPython's rule). A registry whose `mediaCreatedAt` carries an offset would make
//! Python compare aware and naive times and fail; here it fails too (`internal_error`), never a silent match.

use nu9ve_json::{Error, Result};
use nu9ve_time::DateTime;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;

/// Python `missions.parse_date`: ISO text (`Z` allowed) as an aware datetime; naive values are UTC.
pub fn parse_date(value: &Value) -> Option<DateTime> {
    let text = value.as_str().filter(|t| !t.is_empty())?;
    let mut parsed = DateTime::fromisoformat(&text.replace('Z', "+00:00"))?;
    parsed.offset.get_or_insert(0);
    Some(parsed)
}

/// Python truthiness of a JSON value.
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

/// Python `x or y`.
fn or<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    if truthy(a) { a } else { b }
}

/// Python `str(value)` for the JSON ids of a plan (`str(p.get('id'))`).
fn py_str(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => nu9ve_json::python_float_repr(f),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

/// Python `Path(name).stem.upper()` of a JSON value, or `None`.
fn stem(name: &Value) -> Option<String> {
    let name = name.as_str().filter(|n| !n.is_empty())?;
    let last = name.rsplit('/').next().unwrap_or(name);
    let base = match last.rfind('.') {
        Some(i) if i > 0 && i + 1 < last.len() => &last[..i],
        _ => last,
    };
    Some(base.to_uppercase())
}

fn naive(text: &str) -> Option<DateTime> {
    DateTime::fromisoformat(text)
}

/// Python `_local(record)`: the camera wall clock of a record, naive.
fn local_of(record: &Value) -> Result<Option<DateTime>> {
    if let Some(value) = record["mediaCreatedAt"].as_str()
        && let Some(found) = naive(&value.replace(' ', "T"))
    {
        if found.offset.is_some() {
            return Err(Error::new(
                "internal_error",
                "can't compare offset-naive and offset-aware datetimes (mediaCreatedAt with an offset)",
            ));
        }
        return Ok(Some(found));
    }
    if let Some(value) = record["capturedAtLocal"].as_str()
        && let Some(found) = naive(value)
    {
        let wall = found.wall.with_nanosecond_zero();
        return Ok(Some(DateTime { wall, offset: None }));
    }
    Ok(parse_date(&record["capturedAt"]).map(|when| {
        let local = when.astimezone();
        DateTime {
            wall: local.wall.with_nanosecond_zero(),
            offset: None,
        }
    }))
}

trait ZeroNanos {
    fn with_nanosecond_zero(self) -> nu9ve_time::Naive;
}

impl ZeroNanos for nu9ve_time::Naive {
    fn with_nanosecond_zero(self) -> nu9ve_time::Naive {
        use nu9ve_time::chrono::Timelike;
        self.with_nanosecond(0).expect("valid")
    }
}

/// Python `isinstance(v, (int, float))`, where a JSON boolean counts as an int (1/0) like Python's `bool`.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
}

fn position(record: &Value) -> Option<(f64, f64)> {
    for (lat, lon) in [
        ("latitude", "longitude"),
        ("actualLatitude", "actualLongitude"),
    ] {
        if let (Some(a), Some(b)) = (number(&record[lat]), number(&record[lon])) {
            return Some((a, b));
        }
    }
    None
}

/// Python `missions.meters`: great-circle distance (mean Earth radius 6 371 008.8 m).
pub fn meters(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lon1, lat2, lon2) = (
        a.0.to_radians(),
        a.1.to_radians(),
        b.0.to_radians(),
        b.1.to_radians(),
    );
    let h = ((lat2 - lat1) / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * ((lon2 - lon1) / 2.0).sin().powi(2);
    6_371_008.8 * 2.0 * h.sqrt().asin()
}

#[derive(Debug, Clone)]
struct Entry {
    when: DateTime,
    exact: bool,
    position: Option<(f64, f64)>,
    plan_id: Value,
    session: Value,
    capture_id: Value,
    plan_name: Value,
}

/// Seconds between two naive wall times, with microseconds (Python `total_seconds()`).
fn seconds_between(a: &DateTime, b: &DateTime) -> f64 {
    (a.wall - b.wall)
        .num_microseconds()
        .map_or(f64::INFINITY, |us| us as f64 / 1e6)
}

/// Python `MissionIndex`: registry records by original file name for the flow's time/name/position rule.
pub struct MissionIndex {
    flow: Value,
    pub history_sha256: Option<String>,
    by_name: HashMap<String, Vec<Entry>>,
    sessions: HashMap<String, DateTime>,
    /// A full export covers every photo taken before it (naive local time).
    pub covered_until: Option<DateTime>,
}

impl MissionIndex {
    pub fn new(history: &Value, flow: &Value, history_sha256: Option<String>) -> Result<Self> {
        let mut plans: Vec<(String, Value)> = Vec::new();
        for plan in history["plans"].as_array().into_iter().flatten() {
            let key = py_str(&plan["id"]);
            match plans.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = plan.clone(),
                None => plans.push((key, plan.clone())),
            }
        }
        // Schema 1–2 exports nest captures inside their plan: they inherit its id (`setdefault`).
        for (_, plan) in plans.iter_mut() {
            let id = plan["id"].clone();
            if let Some(captures) = plan.get_mut("captures").and_then(Value::as_array_mut) {
                for record in captures.iter_mut().filter_map(Value::as_object_mut) {
                    record.entry("planID").or_insert(id.clone());
                }
            }
        }
        let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
        let mut records = list(&history["captureLog"]);
        records.extend(list(&history["captures"]));
        for (_, plan) in &plans {
            records.extend(list(&plan["captures"]));
        }
        let mut by_name: HashMap<String, Vec<Entry>> = HashMap::new();
        let mut sessions: HashMap<String, DateTime> = HashMap::new();
        for record in &records {
            let name = stem(or(&record["imageJPEGReference"], &record["imageReference"]));
            let (Some(name), Some(when)) = (name, local_of(record)?) else {
                continue;
            };
            let plan = plans
                .iter()
                .find(|(k, _)| *k == py_str(&record["planID"]))
                .map(|(_, p)| p.clone())
                .unwrap_or_else(|| json!({}));
            let session = or(
                or(&record["sessionID"], &record["flightRunID"]),
                &record["planID"],
            )
            .clone();
            let plan_name = or(or(&plan["name"], &record["planName"]), &json!("MISION")).clone();
            let entry = Entry {
                when,
                exact: record["mediaCreatedAt"].is_string(),
                position: position(record),
                plan_id: record["planID"].clone(),
                session: session.clone(),
                capture_id: record["captureID"].clone(),
                plan_name,
            };
            by_name.entry(name).or_default().push(entry);
            if truthy(&session) {
                let key = session.to_string();
                if sessions
                    .get(&key)
                    .is_none_or(|existing| when.wall < existing.wall)
                {
                    sessions.insert(key, when);
                }
            }
        }
        let mut fulls = Vec::new();
        for source in history["nuun"]["sources"]
            .as_object()
            .into_iter()
            .flat_map(Map::values)
        {
            if matches!(source["kind"].as_str(), Some("all" | "merged")) {
                // Python: max() over a None (unparsable exportedAt) raises.
                fulls.push(parse_date(&source["exportedAt"]).ok_or_else(|| {
                    Error::new(
                        "internal_error",
                        "'>' not supported between instances of 'NoneType' and 'datetime'",
                    )
                })?);
            }
        }
        let newest = fulls.into_iter().max_by_key(|d| d.timestamp());
        let covered_until = newest.map(|n| n.astimezone().naive());
        Ok(MissionIndex {
            flow: flow.clone(),
            history_sha256,
            by_name,
            sessions,
            covered_until,
        })
    }

    /// An empty registry (no history yet on the archive).
    pub fn empty(flow: &Value) -> Self {
        MissionIndex {
            flow: flow.clone(),
            history_sha256: None,
            by_name: HashMap::new(),
            sessions: HashMap::new(),
            covered_until: None,
        }
    }

    /// Python `MissionIndex.match(photo, original_name)`.
    pub fn matches(&self, photo: &Value, original_name: &str) -> Result<Value> {
        let Some(when) = photo["captured_at"].as_str().filter(|w| !w.is_empty()) else {
            return Ok(json!({"state": "pending_export", "reason": "no_capture_time"}));
        };
        let when = naive(when).ok_or_else(|| {
            Error::new(
                "internal_error",
                format!("Invalid isoformat string: '{when}'"),
            )
        })?;
        let rule = &self.flow["match"];
        let seconds = number(&rule["seconds"]).unwrap_or(0.0);
        let command = number(&rule["seconds_command_time"]).unwrap_or(seconds);
        let limit_m = number(&rule["meters"]).unwrap_or(0.0);
        let mut best: Option<(f64, &Entry)> = None;
        let name = stem(&json!(original_name)).unwrap_or_default();
        for entry in self.by_name.get(&name).into_iter().flatten() {
            let delta = seconds_between(&entry.when, &when).abs();
            if delta > if entry.exact { seconds } else { command } {
                continue;
            }
            let gps = &photo["gps"];
            if truthy(gps)
                && let Some(at) = entry.position
                && meters(
                    (
                        number(&gps["lat"]).unwrap_or(f64::NAN),
                        number(&gps["lon"]).unwrap_or(f64::NAN),
                    ),
                    at,
                ) > limit_m
            {
                continue;
            }
            if best.is_none_or(|(d, _)| delta < d) {
                best = Some((delta, entry));
            }
        }
        if let Some((_, entry)) = best {
            let start = if truthy(&entry.session) {
                self.sessions.get(&entry.session.to_string())
            } else {
                None
            };
            let start = start.unwrap_or(&entry.when);
            return Ok(
                json!({"state": "matched", "planID": entry.plan_id, "sessionID": entry.session,
                             "captureID": entry.capture_id, "plan_name": entry.plan_name,
                             "session_start": start.isoformat()}),
            );
        }
        let likely = likely_mission(photo, &self.flow);
        if let Some(until) = &self.covered_until
            && when.wall <= until.wall
        {
            // No record, but it can still look like a survey: a hint only.
            return Ok(json!({"state": "none", "likely_mission": likely}));
        }
        Ok(json!({"state": "pending_export", "likely_mission": likely}))
    }
}

/// Python `likely_mission`: cheap evidence only (nadir gimbal at mission height), never a classification.
pub fn likely_mission(photo: &Value, flow: &Value) -> bool {
    let hint = &flow["hint"];
    let flight = &photo["flight"];
    let max_pitch = number(&hint["max_gimbal_pitch"]).unwrap_or(-80.0);
    let min_height = number(&hint["min_relative_altitude"]).unwrap_or(15.0);
    matches!((number(&flight["gimbal_pitch"]), number(&flight["relative_altitude"])),
             (Some(p), Some(h)) if p <= max_pitch && h >= min_height)
}

/// Python `index_for(archive_record, flow)`: the archive's registry, or an empty index when none exists yet.
pub fn index_for(archive: &Value, flow: &Value) -> Result<MissionIndex> {
    let area = flow["archive_area"]
        .as_str()
        .unwrap_or_default()
        .trim_matches('/');
    let root = archive["root"].as_str().unwrap_or_default();
    let Some(raw) = crate::tree::read_bytes(
        Path::new(root),
        &format!("{area}/history.json"),
        64 * 1024 * 1024,
    )?
    else {
        return Ok(MissionIndex::empty(flow));
    };
    let history: Value =
        serde_json::from_slice(&raw).map_err(|e| Error::new("internal_error", e.to_string()))?;
    let sha = {
        use sha2::Digest;
        nu9ve_json::hex(&sha2::Sha256::digest(&raw))
    };
    MissionIndex::new(&history, flow, Some(sha))
}

pub(crate) fn check_plan(catalog: &crate::catalog::Catalog, plan: &Value) -> Result<()> {
    let Some(missions) = plan["missions"].as_object().filter(|m| !m.is_empty()) else {
        return Ok(());
    };
    let archive = catalog.volume(plan["archive_id"].as_str().unwrap_or_default())?;
    for (id, planned) in missions {
        let flow = crate::mission_review::flow(catalog, &json!(id))?;
        let current = index_for(&archive, &flow)?;
        nu9ve_json::require(
            planned["history_sha256"] == json!(current.history_sha256),
            "mission_registry_changed",
            "Cambió el registro de misiones. Vuelve a revisar la tarjeta.",
        )?;
    }
    Ok(())
}

pub fn archived_review(
    catalog: &crate::catalog::Catalog,
    backend: &dyn crate::volumes::Backend,
    params: &Value,
) -> Result<Value> {
    let (mut review, _) = archived_candidates(catalog, backend, params)?;
    review["sample"]
        .as_array_mut()
        .expect("sample array")
        .truncate(20);
    Ok(review)
}

fn archived_candidates(
    catalog: &crate::catalog::Catalog,
    backend: &dyn crate::volumes::Backend,
    params: &Value,
) -> Result<(Value, Value)> {
    let archive = catalog.volume(params["archive"].as_str().unwrap_or_default())?;
    nu9ve_json::require(
        archive["role"] == "archive",
        "invalid_role",
        "Elige un disco de respaldo.",
    )?;
    crate::volumes::check(backend, &archive)?;
    let flow = crate::mission_review::flow(catalog, &params["flow_id"])?;
    let flow_id = flow["id"].as_str().unwrap_or_default();
    let index = index_for(&archive, &flow)?;
    let mut matched = 0;
    let mut waiting = 0;
    let mut no_mission = 0;
    let mut incomplete = 0;
    let mut sample = Vec::new();
    let mut files_to_verify = Vec::new();
    let mut recorded = 0;
    let plans = catalog.json_rows(
        "SELECT plan FROM jobs WHERE state IN ('completed','source_retained','camera_action_required','recovered') AND json_extract(plan,'$.archive_id')=? AND json_type(plan,'$.missions')='object' ORDER BY updated_at DESC",
        [archive["id"].as_str().unwrap_or_default()],
    )?;
    let mut seen = std::collections::BTreeSet::new();
    let tree = crate::tree::Root::open(Path::new(archive["root"].as_str().unwrap_or_default()))?;
    for plan in plans {
        if plan["missions"][flow_id]["units"]["pending_export"]
            .as_u64()
            .unwrap_or_default()
            == 0
        {
            continue;
        }
        let job_id = plan["id"].as_str().unwrap_or_default();
        let files: HashMap<_, _> = plan["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|file| file["path"].as_str().map(|path| (path, file)))
            .collect();
        let receipts = catalog.json_rows(
            "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
            [job_id, archive["id"].as_str().unwrap_or_default()],
        )?;
        let copies: HashMap<_, _> = receipts
            .iter()
            .filter_map(|row| row["path"].as_str().map(|path| (path, row)))
            .collect();
        for unit in plan["units"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|unit| {
                unit["mission"]["flow"] == flow_id && unit["mission"]["state"] == "pending_export"
            })
        {
            let id = unit["id"].as_str().unwrap_or_default();
            if catalog
                .get(&format!("mission-association:{id}"))?
                .is_some_and(|row| row["history_sha256"] == json!(index.history_sha256))
            {
                recorded += 1;
                continue;
            }
            let members: Vec<_> = unit["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|path| {
                    matches!(crate::snapshot::kind_of(path), Some("image" | "raw_image"))
                })
                .collect();
            let mut destinations = Vec::new();
            let mut hashes = Vec::new();
            for path in &members {
                if let Some(receipt) = copies.get(path)
                    && let Some(rel) = receipt["destination"].as_str()
                    && receipt["sha256"]
                        .as_str()
                        .is_some_and(|value| value.len() == 64)
                    && tree.stat(rel)?.is_some_and(|s| receipt["bytes"] == s.size)
                {
                    destinations.push(rel);
                    hashes.push(json!({"path": rel, "sha256":receipt["sha256"]}));
                }
            }
            if members.is_empty() || destinations.len() != members.len() {
                incomplete += 1;
                continue;
            }
            destinations.sort_unstable();
            if !seen.insert(
                destinations
                    .iter()
                    .map(|p| (*p).to_owned())
                    .collect::<Vec<_>>(),
            ) {
                continue;
            }
            let Some(path) = members
                .iter()
                .filter(|p| {
                    files
                        .get(*p)
                        .is_some_and(|f| f["photo"]["captured_at"].is_string())
                })
                .min_by_key(|p| crate::snapshot::kind_of(p) != Some("image"))
            else {
                incomplete += 1;
                continue;
            };
            let original = path.rsplit('/').next().unwrap_or_default();
            let result = index.matches(&files[path]["photo"], original)?;
            match result["state"].as_str() {
                Some("matched") => {
                    matched += 1;
                    sample.push(json!({"original":original, "paths":destinations,
                        "unit_id":unit["id"],"mission":result}));
                    files_to_verify.extend(hashes);
                }
                Some("none") => no_mission += 1,
                _ => waiting += 1,
            }
        }
    }
    let digest = nu9ve_json::digest(
        &json!({"archive":archive["id"],"identity":archive["identity"],
        "history_sha256":index.history_sha256, "associations":sample, "verification": files_to_verify}),
    );
    Ok((
        json!({"matched":matched,"waiting":waiting,"no_mission":no_mission,"incomplete":incomplete,
        "recorded":recorded,"sample":sample,"digest":digest,"archive":archive["id"],"flow_id":flow_id,
        "history_sha256":index.history_sha256}),
        json!(files_to_verify),
    ))
}

pub fn archived_apply(
    home: &Path,
    backend: &dyn crate::volumes::Backend,
    params: &Value,
) -> Result<Value> {
    #[cfg(target_os = "macos")]
    let _cancel = crate::interrupt::Cancel::install()?;
    let _write = crate::catalog::writer_lock(home)?;
    let catalog = crate::catalog::Catalog::open(home)?;
    let (review, files) = archived_candidates(&catalog, backend, params)?;
    nu9ve_json::require(
        review["digest"] == params["digest"] && review["matched"].as_u64().unwrap_or_default() > 0,
        "plan_changed",
        "Vuelve a revisar las asociaciones del archivo.",
    )?;
    let archive = catalog.volume(review["archive"].as_str().unwrap_or_default())?;
    let root = crate::tree::Root::open(Path::new(archive["root"].as_str().unwrap_or_default()))?;
    for file in files.as_array().into_iter().flatten() {
        #[cfg(target_os = "macos")]
        nu9ve_json::require(
            !crate::interrupt::cancelled(),
            "cancelled",
            "Revisión cancelada",
        )?;
        let rel = file["path"].as_str().unwrap_or_default();
        let hash = crate::release::checked_hash(&root, rel)?;
        nu9ve_json::require(
            hash.as_ref()
                .is_some_and(|(sha, _)| file["sha256"] == sha.as_str()),
            "checksum_mismatch",
            format!("Cambió la copia de archivo: {rel}"),
        )?;
    }
    crate::volumes::check(backend, &archive)?;
    let flow = crate::mission_review::flow(&catalog, &review["flow_id"])?;
    nu9ve_json::require(
        json!(index_for(&archive, &flow)?.history_sha256) == review["history_sha256"],
        "mission_registry_changed",
        "Cambió el registro de misiones.",
    )?;
    let writer = crate::catalog::WritableCatalog::open(home)?;
    let tx = writer
        .db()
        .unchecked_transaction()
        .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
    for row in review["sample"].as_array().into_iter().flatten() {
        let id = row["unit_id"].as_str().unwrap_or_default();
        writer.set(&format!("mission-association:{id}"), &json!({"archive":review["archive"], "flow":review["flow_id"],
            "path_unchanged":true, "history_sha256":review["history_sha256"], "mission":row["mission"]}))?;
    }
    tx.commit()
        .map_err(|e| Error::new("catalog_unavailable", e.to_string()))?;
    let backup = crate::units_map::backup_metadata_locked(
        home,
        backend,
        review["archive"].as_str().unwrap_or_default(),
        &writer,
    );
    Ok(json!({"associated":review["matched"],"files_moved":0,
        "metadata_backup_error":backup.err().map(|e|e.message)}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_registry_requires_new_review() {
        let fixture = crate::volumes::test_backend::setup();
        let catalog = crate::catalog::Catalog::open(&fixture.home).unwrap();
        let flow = flow();
        let plan = json!({"archive_id":"MEDIA","missions": {
            flow["id"].as_str().unwrap(): {"history_sha256": null}
        }});
        check_plan(&catalog, &plan).unwrap();
        let area = fixture.archive.join(flow["archive_area"].as_str().unwrap());
        std::fs::create_dir_all(&area).unwrap();
        std::fs::write(
            area.join("history.json"),
            b"{\"plans\":[],\"captureLog\":[]}",
        )
        .unwrap();
        assert_eq!(
            check_plan(&catalog, &plan).unwrap_err().code,
            "mission_registry_changed"
        );
    }

    fn flow() -> Value {
        nuun_recipe::template()["missions"][0].clone()
    }

    fn photo(when: &str) -> Value {
        json!({"captured_at": when, "gps": {"lat": 29.0984139, "lon": -110.9622728},
               "flight": {"gimbal_pitch": -89.9, "relative_altitude": 45.1}})
    }

    fn history(records: Value) -> Value {
        json!({"plans": [{"id": "P1", "name": "Calles-45m-1104-1625"}], "captureLog": records,
               "nuun": {"sources": {"s": {"kind": "all", "exportedAt": "2026-11-05T05:00:00Z"}}}})
    }

    fn record(when: &str, name: &str, cid: &str) -> Value {
        json!({"captureID": cid, "planID": "P1", "sessionID": "S1", "capturedAt": "2026-11-04T23:32:13Z",
               "mediaCreatedAt": when, "imageJPEGReference": name, "latitude": 29.0984139, "longitude": -110.9622728})
    }

    #[test]
    fn a_photo_matches_by_name_time_and_place_and_takes_the_session_start() {
        let h = history(json!([
            record("2026-11-04 16:30:00", "DJI_0000.JPG", "C0"),
            record("2026-11-04 16:32:13", "DJI_0001.JPG", "C1")
        ]));
        let index = MissionIndex::new(&h, &flow(), None).unwrap();
        let found = index
            .matches(&photo("2026-11-04T16:32:12"), "DJI_0001.JPG")
            .unwrap();
        assert_eq!(
            found,
            json!({"state": "matched", "planID": "P1", "sessionID": "S1", "captureID": "C1",
                   "plan_name": "Calles-45m-1104-1625", "session_start": "2026-11-04T16:30:00"})
        );
        // 3 s away with an exact camera time: not the same photo.
        let late = index
            .matches(&photo("2026-11-04T16:32:16"), "DJI_0001.JPG")
            .unwrap();
        assert_eq!(
            late,
            json!({"state": "none", "likely_mission": true}),
            "covered by the full export"
        );
    }

    #[test]
    fn far_records_and_uncovered_photos() {
        let mut far = record("2026-11-04 16:32:13", "DJI_0001.JPG", "C1");
        far["latitude"] = json!(29.0994139); // ~110 m
        let index = MissionIndex::new(&history(json!([far])), &flow(), None).unwrap();
        let found = index
            .matches(&photo("2026-11-04T16:32:12"), "DJI_0001.JPG")
            .unwrap();
        // Covered by the full export of 5 Nov (UTC) in any host zone between -12 and +14 h.
        assert_eq!(found, json!({"state": "none", "likely_mission": true}));
        let empty = MissionIndex::empty(&flow());
        assert_eq!(
            empty
                .matches(
                    &json!({"captured_at": "2026-11-04T16:32:12"}),
                    "DJI_0001.JPG"
                )
                .unwrap(),
            json!({"state": "pending_export", "likely_mission": false})
        );
        assert_eq!(
            empty.matches(&json!({}), "x.JPG").unwrap()["reason"],
            "no_capture_time"
        );
    }

    #[test]
    fn nested_captures_inherit_their_plan_and_command_times_get_more_slack() {
        let h = json!({"plans": [{"id": 7, "name": "Viejo", "captures": [
            {"captureID": "C9", "capturedAtLocal": "2026-11-04T16:32:18.250", "imageReference": "dji_0001.jpg"}]}]});
        let index = MissionIndex::new(&h, &flow(), None).unwrap();
        let found = index
            .matches(
                &json!({"captured_at": "2026-11-04T16:32:12"}),
                "DJI_0001.JPG",
            )
            .unwrap();
        assert_eq!(
            (
                found["planID"].clone(),
                found["plan_name"].clone(),
                found["sessionID"].clone()
            ),
            (json!(7), json!("Viejo"), json!(7))
        );
        assert!(index.covered_until.is_none());
    }

    #[test]
    fn meters_and_parse_date_follow_python() {
        assert!(
            (meters((29.0984139, -110.9622728), (29.0994139, -110.9622728)) - 111.19).abs() < 0.01
        );
        assert_eq!(
            parse_date(&json!("2026-11-05T05:00:00Z"))
                .unwrap()
                .isoformat(),
            "2026-11-05T05:00:00+00:00"
        );
        assert_eq!(
            parse_date(&json!("2026-11-05T05:00:00")).unwrap().offset,
            Some(0)
        );
        assert!(parse_date(&json!(5)).is_none());
        let aware = json!({"captureLog": [{"mediaCreatedAt": "2026-11-04 16:32:13-07:00", "imageReference": "a.jpg"}]});
        assert_eq!(
            MissionIndex::new(&aware, &flow(), None).err().unwrap().code,
            "internal_error"
        );
    }

    #[test]
    fn index_for_reads_the_archive_history_or_starts_empty() {
        let root = tempfile::tempdir().unwrap();
        let archive = json!({"root": root.path().display().to_string()});
        assert!(
            index_for(&archive, &flow())
                .unwrap()
                .history_sha256
                .is_none()
        );
        let area = root.path().join("004_datos/subaereo/coverage");
        std::fs::create_dir_all(&area).unwrap();
        std::fs::write(area.join("history.json"), b"{}").unwrap();
        assert_eq!(
            index_for(&archive, &flow())
                .unwrap()
                .history_sha256
                .unwrap(),
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }
}
