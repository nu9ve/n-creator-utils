//! Reviewed Atlas DNG sampling and recoverable moves to the work volume's Trash.

use crate::catalog::{Catalog, WritableCatalog, sql, writer_lock};
use crate::mission_review::{flow, fnmatch};
use crate::release::checked_hash;
use crate::rules::relative;
use crate::transfer::{Target, Transfer};
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend, truthy};
use nu9ve_json::{Error, Result, canonical, digest, require};
use nu9ve_volumes::fs;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "nu9ve-dng-plan/1";
const MAX_MAP: usize = 1 << 34;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn required<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params[key]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
}

fn load_units(root: &Root, path: &str) -> Result<Vec<Value>> {
    let raw = crate::tree::read_bytes(&root.path, path, MAX_MAP)?.ok_or_else(|| {
        Error::new(
            "no_units_map",
            "No units map yet: ingest drone stills first",
        )
    })?;
    let raw =
        std::str::from_utf8(&raw).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|e| Error::new("operation_failed", e.to_string()))
        })
        .collect()
}

fn map_path(work: &Value, flow: &Value) -> Result<(String, String)> {
    let path = format!(
        "{}/{}",
        text(work, "prefix").trim_matches('/'),
        text(flow, "units_map").trim_matches('/')
    );
    relative(&path)?;
    let base = path
        .rsplitn(3, '/')
        .nth(2)
        .ok_or_else(|| Error::new("invalid_plan", "Invalid units map path"))?
        .to_owned();
    Ok((path, base))
}

fn work_paths(unit: &Value, base: &str) -> Result<Option<(String, String)>> {
    let Some(jpg) = unit["work"]["jpg"].as_str().filter(|p| !p.is_empty()) else {
        return Ok(None);
    };
    let jpg = relative(jpg)?;
    let dng = Path::new(jpg).with_extension("DNG");
    Ok(Some((
        format!("{base}/{jpg}"),
        format!("{base}/{}", dng.to_string_lossy()),
    )))
}

fn list_matches(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let mut paths = vec![root.to_owned()];
    for part in relative(pattern)?.split('/') {
        let mut next = Vec::new();
        for path in paths {
            if part.contains(['*', '?', '[']) {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        if (!name.starts_with('.') || part.starts_with('.')) && fnmatch(&name, part)
                        {
                            next.push(entry.path());
                        }
                    }
                }
            } else {
                next.push(path.join(part));
            }
        }
        paths = next;
    }
    paths.sort();
    Ok(paths)
}

fn labels(root: &Root, work: &Value, rules: &Value) -> Result<BTreeMap<String, String>> {
    let prefix = relative(text(work, "prefix"))?;
    let mut labels = BTreeMap::new();
    for path in list_matches(&root.path.join(prefix), text(rules, "labels"))? {
        let Some(rel) = path.strip_prefix(&root.path).ok().and_then(|p| p.to_str()) else {
            continue;
        };
        let Some(manifest) = crate::tree::read_bytes(&root.path, rel, 16 << 20)
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            continue;
        };
        let set = manifest["set"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                path.parent()
                    .and_then(Path::file_name)
                    .and_then(|n| n.to_str())
            })
            .unwrap_or_default();
        let key = rules["labels_key"].as_str().unwrap_or("photos");
        for photo in manifest[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            labels.insert(photo.to_owned(), set.to_owned());
        }
    }
    Ok(labels)
}

struct Session<'a> {
    id: &'a str,
    plan: &'a Value,
    units: Vec<(&'a Value, &'a Value)>,
}

fn ratio(session: &Session<'_>, field: &str, class: &Value) -> f64 {
    let values: Vec<_> = session
        .units
        .iter()
        .map(|(_, record)| &record[field])
        .filter(|v| truthy(v))
        .collect();
    if values.is_empty() {
        return -1.0;
    }
    values.iter().filter(|v| **v == class).count() as f64 / values.len() as f64
}

fn evenly<'a>(items: &[&'a Value], count: usize) -> Vec<&'a Value> {
    if items.len() <= count {
        return items.to_vec();
    }
    let step = items.len() as f64 / count as f64;
    (0..count)
        .map(|i| items[(i as f64 * step) as usize])
        .collect()
}

fn select(
    units: &[Value],
    history: &Value,
    labels: &BTreeMap<String, String>,
    rules: &Value,
) -> Vec<Value> {
    let log: BTreeMap<_, _> = ["captureLog", "captures"]
        .into_iter()
        .flat_map(|key| history[key].as_array().into_iter().flatten())
        .filter_map(|record| {
            record["captureID"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|id| (id, record))
        })
        .collect();
    let plans: BTreeMap<_, _> = history["plans"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|plan| plan["id"].as_str().map(|id| (id, &plan["name"])))
        .collect();
    let mut sessions = Vec::<Session<'_>>::new();
    let mut by_id = BTreeMap::new();
    let mut by_capture = BTreeMap::new();
    for unit in units.iter().filter(|unit| truthy(&unit["archive"]["dng"])) {
        let stem = Path::new(text(unit, "original_name"))
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let when = text(unit, "captured_at").replace(['-', ':', 'T'], "");
        by_capture.insert(format!("{stem}_{when}"), unit);
        let mission = &unit["mission"];
        if !truthy(mission) {
            continue;
        }
        let id = text(mission, "sessionID");
        let plan = plans
            .get(text(mission, "planID"))
            .copied()
            .filter(|p| truthy(p))
            .unwrap_or(&unit["plan_name"]);
        let index = *by_id.entry(id).or_insert_with(|| {
            let index = sessions.len();
            sessions.push(Session {
                id,
                plan,
                units: Vec::new(),
            });
            index
        });
        sessions[index].units.push((
            unit,
            log.get(text(mission, "captureID"))
                .copied()
                .unwrap_or(&Value::Null),
        ));
    }
    for session in &mut sessions {
        session
            .units
            .sort_by_key(|(unit, _)| text(unit, "captured_at"));
    }
    sessions.sort_by_key(|session| std::cmp::Reverse(text(session.units[0].0, "captured_at")));
    let mut groups = Vec::new();
    let mut taken = BTreeSet::new();
    let mut add = |reason, detail: Value, members: Vec<&Value>| {
        let chosen: Vec<_> = members
            .iter()
            .map(|unit| text(unit, "asset_id").to_owned())
            .filter(|id| !taken.contains(id))
            .collect();
        if !chosen.is_empty() {
            taken.extend(chosen.iter().cloned());
            groups.push(json!({"reason":reason,"detail":detail,"asset_ids":chosen}));
        }
    };
    let names: BTreeSet<_> = labels.values().map(String::as_str).collect();
    add(
        "labelled",
        json!(names.into_iter().collect::<Vec<_>>().join(", ")),
        labels
            .keys()
            .filter_map(|id| by_capture.get(id).copied())
            .collect(),
    );
    let dominance = rules["dominance"].as_f64().unwrap_or(0.6);
    let sample = |key: &str| rules[key].as_u64().unwrap_or_default() as usize;
    let mut used = BTreeSet::new();
    for light in rules["light_classes"].as_array().into_iter().flatten() {
        if let Some(session) = sessions
            .iter()
            .find(|s| ratio(s, "lightClass", light) >= dominance)
        {
            let mut best: Vec<_> = session
                .units
                .iter()
                .filter(|(_, r)| matches!(r["qualityGrade"].as_str(), Some("good" | "fair")))
                .map(|(u, _)| *u)
                .collect();
            if best.is_empty() {
                best.extend(session.units.iter().map(|(u, _)| *u));
            }
            let plan = session.plan.as_str().unwrap_or("None");
            add(
                "light",
                json!(format!("{} · {plan}", light.as_str().unwrap_or_default())),
                evenly(&best, sample("per_light")),
            );
            used.insert(session.id);
        }
    }
    let poor = json!("poor");
    if let Some(session) = sessions
        .iter()
        .find(|s| !used.contains(s.id) && ratio(s, "qualityGrade", &poor) >= dominance)
    {
        let members: Vec<_> = session
            .units
            .iter()
            .filter(|(_, r)| r["qualityGrade"] == poor)
            .map(|(u, _)| *u)
            .collect();
        add(
            "poor",
            session.plan.clone(),
            evenly(&members, sample("poor_sample")),
        );
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let local = nu9ve_time::host_local(now.floor() as i64) as f64 + now.fract();
    let mut cutoff = nu9ve_time::DateTime::fromtimestamp_utc(
        local - rules["older_than_days"].as_f64().unwrap_or_default() * 86400.0,
    );
    cutoff.offset = None;
    let cutoff = cutoff.isoformat();
    let mut largest: Option<&Session<'_>> = None;
    for session in sessions
        .iter()
        .filter(|s| text(s.units[0].0, "captured_at") < cutoff.as_str())
    {
        if largest.is_none_or(|prior| prior.units.len() < session.units.len()) {
            largest = Some(session);
        }
    }
    if let Some(session) = largest {
        let members: Vec<_> = session.units.iter().map(|(u, _)| *u).collect();
        add(
            "older",
            session.plan.clone(),
            evenly(&members, sample("older_sample")),
        );
    }
    groups
}

fn volumes_for(
    catalog: &Catalog,
    backend: &dyn Backend,
    archive: &str,
    work: &str,
) -> Result<(Value, Value)> {
    let archive = catalog.volume(archive)?;
    let work = catalog.volume(work)?;
    require(
        archive["role"] == "archive" && work["role"] == "work",
        "invalid_role",
        "Choose the archive and the work volume",
    )?;
    volumes::check(backend, &archive)?;
    volumes::check(backend, &work)?;
    Ok((archive, work))
}

pub fn plan(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let flow = flow(&catalog, &params["flow_id"])?;
    let (archive, work) = volumes_for(
        &catalog,
        backend,
        required(params, "archive")?,
        required(params, "work")?,
    )?;
    let root = Root::open(Path::new(text(&work, "root")))?;
    let (map, base) = map_path(&work, &flow)?;
    let units = load_units(&root, &map)?;
    let history_path = format!(
        "{}/history.json",
        text(&flow, "archive_area").trim_matches('/')
    );
    let raw = crate::tree::read_bytes(Path::new(text(&archive, "root")), &history_path, MAX_MAP)?;
    let history: Value = serde_json::from_slice(raw.as_deref().unwrap_or(b"{}"))
        .map_err(|e| Error::new("operation_failed", e.to_string()))?;
    let rules = &flow["dng"];
    let groups = if params["keep"].is_null() {
        select(&units, &history, &labels(&root, &work, rules)?, rules)
    } else {
        require(
            params["keep"].is_array(),
            "invalid_request",
            "keep must be a list",
        )?;
        vec![json!({"reason":"chosen","detail":"","asset_ids":params["keep"]})]
    };
    let selected: BTreeSet<_> = groups
        .iter()
        .flat_map(|group| group["asset_ids"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let receipts: BTreeMap<_, _> = catalog
        .json_rows(
            "SELECT payload FROM copies WHERE volume_id=?",
            [text(&archive, "id")],
        )?
        .into_iter()
        .map(|row| (text(&row, "destination").to_owned(), row))
        .collect();
    let (mut copy, mut release, mut kept, mut problems, mut unbacked) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut known = BTreeSet::new();
    for unit in &units {
        let dest = text(&unit["archive"], "dng");
        let Some((jpg, dng)) = work_paths(unit, &base)? else {
            continue;
        };
        if dest.is_empty() {
            continue;
        }
        relative(dest)?;
        known.extend([jpg, dng.clone()]);
        let present = root.stat(&dng)?;
        let receipt = receipts
            .get(dest)
            .filter(|row| row["sha256"] == unit["sha256"]["dng"]);
        let id = text(unit, "asset_id");
        let sha = &unit["sha256"]["dng"];
        if selected.contains(id) {
            if present.is_some() {
                kept.push(json!({"asset_id":id,"work":dng}));
            } else if let Some(row) = receipt {
                copy.push(json!({"asset_id":id,"archive":dest,"sha256":sha,"work":dng,"bytes":row["bytes"]}));
            } else {
                problems.push(json!({"asset_id":id,"reason":"no_verified_archive_copy"}));
            }
        } else if let Some(present) = present {
            if let Some(row) = receipt {
                release.push(json!({"asset_id":id,"archive":dest,"sha256":sha,"work":dng,"archive_stamp":row["stamp"],"bytes":present.size}));
            } else {
                problems
                    .push(json!({"asset_id":id,"work":dng,"reason":"no_verified_archive_copy"}));
            }
        }
    }
    let requested = params["trash_unbacked"].as_array();
    require(
        params["trash_unbacked"].is_null() || requested.is_some(),
        "invalid_request",
        "trash_unbacked must be a list",
    )?;
    let mut requested: Vec<_> = requested.into_iter().flatten().collect();
    requested.sort_by_key(|path| path.as_str().unwrap_or_default());
    for path in requested {
        let rel = path.as_str().unwrap_or_default();
        require(
            relative(rel).is_ok() && rel.starts_with(&format!("{base}/")) && !known.contains(rel),
            "invalid_request",
            format!("Only stills without a backup, inside the consumer root, can be listed: {rel}"),
        )?;
        let stamp = root
            .stat(rel)?
            .ok_or_else(|| Error::new("invalid_request", format!("Not a regular file: {rel}")))?;
        unbacked.push(json!({"work":rel,"stamp":stamp_json(&stamp),"bytes":stamp.size}));
    }
    let bytes = |rows: &[Value]| {
        rows.iter()
            .filter_map(|row| row["bytes"].as_u64())
            .sum::<u64>()
    };
    let mut value = json!({"schema":SCHEMA,"flow":flow["id"],"archive_id":archive["id"],"work_id":work["id"],
        "groups":groups,"kept":kept,"copy":copy,"release":release,"trash_unbacked":unbacked,"problems":problems,
        "totals":{"release_files":release.len(),"release_bytes":bytes(&release),"copy_files":copy.len(),"copy_bytes":bytes(&copy),
            "kept_files":kept.len(),"unbacked_files":unbacked.len(),"unbacked_bytes":bytes(&unbacked)}});
    value["digest"] = json!(digest(&value));
    Ok(value)
}

fn trash_path(tree: &Root, path: &str) -> Result<String> {
    let path = Path::new(path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let ext = path
        .extension()
        .and_then(|name| name.to_str())
        .map(|name| format!(".{name}"))
        .unwrap_or_default();
    // SAFETY: getuid has no arguments or memory effects.
    let uid = unsafe { libc::getuid() };
    let folder = format!(".Trashes/{uid}");
    for index in 1.. {
        let rel = if index == 1 {
            format!("{folder}/{name}")
        } else {
            format!("{folder}/{stem} {index}{ext}")
        };
        let (dir, name) = tree.parent(&rel, true)?;
        if fs::stamp_opt(&dir.join(name))?.is_none() {
            return Ok(rel);
        }
    }
    unreachable!()
}

pub fn apply(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    apply_with(home, backend, params, &|_, _| Ok(()))
}

fn apply_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    hook: crate::transfer::Hook<'_>,
) -> Result<Value> {
    let mut value: Value = if let Some(raw) = params["plan"].as_str() {
        serde_json::from_str(raw).map_err(|e| Error::new("operation_failed", e.to_string()))?
    } else {
        params["plan"].clone()
    };
    let expected = required(params, "digest")?;
    if let Some(doc) = value.as_object_mut() {
        doc.remove("digest");
    }
    require(
        value["schema"] == SCHEMA && digest(&value) == expected,
        "review_changed",
        "The DNG plan changed; review it again",
    )?;
    let operation = format!("dng-{}", &expected[..16]);
    let _lock = writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let writer = WritableCatalog::open(home)?;
    let (archive, work) = volumes_for(
        &catalog,
        backend,
        required(&value, "archive_id")?,
        required(&value, "work_id")?,
    )?;
    let archive_tree = Root::open(Path::new(text(&archive, "root")))?;
    let work_tree = Root::open(Path::new(text(&work, "root")))?;
    let flow = flow(&catalog, &value["flow"])?;
    let (map, base) = map_path(&work, &flow)?;
    let mut units = load_units(&work_tree, &map)?;
    let mut known = BTreeSet::new();
    let mut mapped = BTreeMap::new();
    for unit in &units {
        if truthy(&unit["archive"]["dng"])
            && let Some((jpg, dng)) = work_paths(unit, &base)?
        {
            known.extend([jpg, dng.clone()]);
            mapped.insert(dng, unit);
        }
    }
    let receipts: BTreeMap<_, _> = catalog
        .json_rows(
            "SELECT payload FROM copies WHERE volume_id=?",
            [text(&archive, "id")],
        )?
        .into_iter()
        .map(|receipt| (text(&receipt, "destination").to_owned(), receipt))
        .collect();
    let mut unique = BTreeSet::new();
    for key in ["copy", "release", "trash_unbacked"] {
        for item in value[key]
            .as_array()
            .ok_or_else(|| Error::new("invalid_plan", "Invalid DNG plan entries"))?
        {
            let path = relative(required(item, "work")?)?;
            require(
                path.starts_with(&format!("{base}/")) && unique.insert(path),
                "invalid_plan",
                "Invalid or duplicate DNG path",
            )?;
            if key == "trash_unbacked" {
                require(
                    !known.contains(path),
                    "invalid_plan",
                    "A backed-up still cannot be marked as unbacked",
                )?;
            } else {
                let source = required(item, "archive")?;
                let unit = mapped.get(path);
                let receipt = receipts.get(source);
                require(
                    unit.is_some_and(|unit| {
                        unit["asset_id"] == item["asset_id"]
                            && unit["archive"]["dng"] == source
                            && unit["sha256"]["dng"] == item["sha256"]
                    }) && receipt.is_some_and(|receipt| {
                        receipt["sha256"] == item["sha256"]
                            && (key != "release" || receipt["stamp"] == item["archive_stamp"])
                    }),
                    "review_changed",
                    "The DNG receipts or units map changed; review again",
                )?;
            }
        }
    }
    let mut transfer = Transfer::new(home, &writer, backend, &|_, _| Ok(()));
    let (mut copied, mut released, mut trashed) = (0, 0, 0);
    for item in value["copy"].as_array().into_iter().flatten() {
        let rel = text(item, "work");
        let sha = required(item, "sha256")?;
        transfer.copy_many(
            &archive_tree,
            required(item, "archive")?,
            &[Target {
                root: &work_tree,
                rel: rel.to_owned(),
                record: &work,
            }],
            Some(sha),
            None,
        )?;
        let journal = format!("{operation}:copy:{rel}");
        if writer.get(&journal)?.is_none() {
            let tx = writer.db().unchecked_transaction().map_err(sql)?;
            writer.event(
                &operation,
                "dng_copied",
                &json!({"asset_id":item["asset_id"],"work":rel,"sha256":sha}),
            )?;
            writer.set(&journal, &json!(true))?;
            tx.commit().map_err(sql)?;
        }
        copied += 1;
    }
    for (key, event, count) in [
        ("release", "dng_released", &mut released),
        ("trash_unbacked", "unbacked_trashed", &mut trashed),
    ] {
        for item in value[key].as_array().into_iter().flatten() {
            volumes::check(backend, &archive)?;
            volumes::check(backend, &work)?;
            let rel = text(item, "work");
            let journal = format!("{operation}:trash:{rel}");
            let prior = writer.get(&journal)?;
            let Some(stamp) = work_tree.stat(rel)? else {
                if let Some(prior) = prior.filter(|prior| prior["state"] == "intent") {
                    let archived = checked_hash(&work_tree, text(&prior, "target"))?;
                    require(
                        archived.is_some_and(|(sha, stamp)| {
                            prior["sha256"] == sha
                                && stamp["size"] == prior["stamp"]["size"]
                                && stamp["inode"] == prior["stamp"]["inode"]
                        }),
                        "source_changed",
                        format!("Missing reviewed file in Trash: {rel}"),
                    )?;
                    finish_trash(&writer, &operation, event, &journal, item, prior)?;
                    *count += 1;
                }
                continue;
            };
            let stamp = stamp_json(&stamp);
            let (source_sha, verified_stamp) = checked_hash(&work_tree, rel)?.ok_or_else(|| {
                Error::new("source_changed", format!("Changed since review: {rel}"))
            })?;
            require(
                verified_stamp == stamp,
                "source_changed",
                format!("Changed since review: {rel}"),
            )?;
            if key == "release" {
                require(
                    item["sha256"] == source_sha,
                    "checksum_mismatch",
                    format!("Work DNG differs from the archive: {rel}"),
                )?;
                let dest = required(item, "archive")?;
                require(
                    checked_hash(&archive_tree, dest)?.is_some_and(|(sha, found)| {
                        item["sha256"] == sha && item["archive_stamp"] == found
                    }),
                    "archive_changed",
                    format!("The archive copy changed since it was verified: {dest}"),
                )?;
            } else {
                require(
                    stamp == item["stamp"],
                    "source_changed",
                    format!("Changed since review: {rel}"),
                )?;
            }
            require(
                prior.as_ref().is_none_or(|prior| prior["state"] != "done"),
                "source_changed",
                format!("Source reappeared after moving to Trash: {rel}"),
            )?;
            let target = match prior {
                Some(prior) => {
                    require(
                        prior["stamp"] == stamp && prior["sha256"] == source_sha,
                        "source_changed",
                        format!("Changed since review: {rel}"),
                    )?;
                    text(&prior, "target").to_owned()
                }
                None => trash_path(&work_tree, rel)?,
            };
            let intent =
                json!({"state":"intent","target":target,"stamp":stamp,"sha256":source_sha});
            writer.set(&journal, &intent)?;
            let (src, name) = work_tree.parent(rel, false)?;
            let (dst, trash_name) = work_tree.parent(&target, true)?;
            require(
                work_tree
                    .stat(rel)?
                    .is_some_and(|found| stamp_json(&found) == stamp),
                "source_changed",
                format!("Changed since review: {rel}"),
            )?;
            fs::rename_new(&src.join(name), &dst.join(trash_name))?;
            fs::sync_dir(&src)?;
            fs::sync_dir(&dst)?;
            hook("after_trash_rename", item)?;
            finish_trash(&writer, &operation, event, &journal, item, intent)?;
            *count += 1;
        }
    }
    let mut raw = Vec::new();
    for unit in &mut units {
        let dng = if let Some((_, dng)) = work_paths(unit, &base)? {
            if work_tree.stat(&dng)?.is_some() {
                json!(&dng[base.len() + 1..])
            } else {
                Value::Null
            }
        } else {
            Value::Null
        };
        unit["work"]["dng"] = dng;
        raw.extend(canonical(unit));
        raw.push(b'\n');
    }
    volumes::check(backend, &work)?;
    work_tree.write_bytes(&map, &raw, true)?;
    require(
        crate::tree::read_bytes(&work_tree.path, &map, MAX_MAP)?.as_deref() == Some(&raw),
        "checksum_mismatch",
        "units map did not read back identical",
    )?;
    Ok(
        json!({"copied":copied,"released":released,"trashed_unbacked":trashed,"operation":operation}),
    )
}

fn finish_trash(
    catalog: &WritableCatalog,
    operation: &str,
    event: &str,
    key: &str,
    item: &Value,
    mut intent: Value,
) -> Result<()> {
    let payload = if event == "dng_released" {
        json!({"asset_id":item["asset_id"],"work":item["work"],"archive":item["archive"]})
    } else {
        json!({"work":item["work"]})
    };
    let tx = catalog.db().unchecked_transaction().map_err(sql)?;
    catalog.event(operation, event, &payload)?;
    intent["state"] = json!("done");
    catalog.set(key, &intent)?;
    tx.commit().map_err(sql)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::test_backend::{Setup, setup};

    const MAP: &str = "001_proyectos/atlas-subaereo/_nuun/units.ndjson";
    const BASE: &str = "001_proyectos/atlas-subaereo";

    fn fixture() -> Setup {
        let s = setup();
        let archive = Root::open(&s.archive).unwrap();
        let work = Root::open(&s.work).unwrap();
        let writer = WritableCatalog::open(&s.home).unwrap();
        let mut units = Vec::new();
        for (id, name, body) in [
            ("a", "a", b"first".as_slice()),
            ("b", "b", b"second".as_slice()),
        ] {
            let path = format!("001_raw/dron/{name}.DNG");
            archive.write_bytes(&path, body, false).unwrap();
            let (sha, stamp) = checked_hash(&archive, &path).unwrap().unwrap();
            let copy = json!({"destination":path,"sha256":sha,"stamp":stamp,"bytes":body.len()});
            writer
                .db()
                .execute(
                    "INSERT INTO copies VALUES('fixture','id-MEDIA',?,?,?)",
                    (name, &sha, copy.to_string()),
                )
                .unwrap();
            units.push(json!({"asset_id":id,"work":{"jpg":format!("photos/{name}.jpg")},"archive":{"dng":path},"sha256":{"dng":sha}}));
        }
        let mut raw = Vec::new();
        for unit in units {
            raw.extend(canonical(&unit));
            raw.push(b'\n');
        }
        work.write_bytes(MAP, &raw, false).unwrap();
        work.write_bytes(&format!("{BASE}/photos/a.DNG"), b"first", false)
            .unwrap();
        work.write_bytes(&format!("{BASE}/photos/orphan.DNG"), b"orphan", false)
            .unwrap();
        s
    }

    #[test]
    fn dng_rechecks_archive_and_resumes_verified_trash_moves() {
        let s = fixture();
        let options = json!({"archive":"MEDIA","work":"ESSD","keep":["b"],"trash_unbacked":[format!("{BASE}/photos/orphan.DNG")]});
        let reviewed = plan(&s.home, &s.backend, &options).unwrap();
        assert_eq!(
            reviewed["totals"],
            json!({"release_files":1,"release_bytes":5,"copy_files":1,"copy_bytes":6,
            "kept_files":0,"unbacked_files":1,"unbacked_bytes":6})
        );
        let args = json!({"plan":reviewed,"digest":reviewed["digest"]});
        let archived = s.archive.join("001_raw/dron/a.DNG");
        std::fs::write(&archived, b"wrong").unwrap();
        assert_eq!(
            apply(&s.home, &s.backend, &args).unwrap_err().code,
            "archive_changed"
        );
        let work = s.work.join(format!("{BASE}/photos/a.DNG"));
        assert!(work.exists());

        let s = fixture();
        let reviewed = plan(&s.home, &s.backend, &options).unwrap();
        let args = json!({"plan":reviewed.to_string(),"digest":reviewed["digest"]});
        let error = apply_with(&s.home, &s.backend, &args, &|stage, _| {
            require(stage != "after_trash_rename", "interrupted", "fixture")
        })
        .unwrap_err();
        assert_eq!(error.code, "interrupted");
        assert!(!s.work.join(format!("{BASE}/photos/a.DNG")).exists());
        let done = apply(&s.home, &s.backend, &args).unwrap();
        assert_eq!(done["released"], 1);
        assert_eq!(done["trashed_unbacked"], 1);
        assert_eq!(
            std::fs::read(s.work.join(format!("{BASE}/photos/b.DNG"))).unwrap(),
            b"second"
        );
        assert_eq!(
            std::fs::read(s.archive.join("001_raw/dron/a.DNG")).unwrap(),
            b"first"
        );
        let map = load_units(&Root::open(&s.work).unwrap(), MAP).unwrap();
        assert!(map[0]["work"]["dng"].is_null());
        assert_eq!(map[1]["work"]["dng"], "photos/b.DNG");
        let repeated = apply(&s.home, &s.backend, &args).unwrap();
        assert_eq!(repeated["released"], 0);
        assert_eq!(repeated["trashed_unbacked"], 0);
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn reviewed_plan_and_units_map_match_python() {
        let s = fixture();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let reviewed = plan(
            &s.home,
            &s.backend,
            &json!({"archive":"MEDIA","work":"ESSD","keep":["b"],
            "trash_unbacked":[format!("{BASE}/photos/orphan.DNG")]}),
        )
        .unwrap();
        let mut result = apply(
            &s.home,
            &s.backend,
            &json!({"plan":reviewed,"digest":reviewed["digest"]}),
        )
        .unwrap();
        result.as_object_mut().unwrap().remove("operation");
        let mut plan = reviewed;
        plan.as_object_mut().unwrap().remove("digest");
        plan["archive_id"] = json!("MEDIA");
        plan["work_id"] = json!("ESSD");
        for (group, key) in [("release", "archive_stamp"), ("trash_unbacked", "stamp")] {
            for item in plan[group].as_array_mut().unwrap() {
                item[key] = json!({"size":item[key]["size"]});
            }
        }
        assert_eq!(plan, golden["dng"]["plan"]);
        assert_eq!(result, golden["dng"]["result"]);
        assert_eq!(
            json!(load_units(&Root::open(&s.work).unwrap(), MAP).unwrap()),
            golden["dng"]["units"]
        );
    }

    #[test]
    fn automatic_sampling_keeps_labels_lighting_poor_and_older_sessions() {
        let mut units = Vec::new();
        let mut log = Vec::new();
        for (index, (session, date, light, grade)) in [
            ("day", "2030-01-01", "lowSun", "good"),
            ("night", "2030-01-02", "night", "poor"),
            ("poor", "2030-01-01", "night", "poor"),
            ("older", "2000-01-01", "", ""),
        ]
        .into_iter()
        .enumerate()
        {
            for sequence in 0..3 {
                let n = index * 10 + sequence;
                let id = format!("{session}-{sequence}");
                units.push(
                    json!({"asset_id":id,"original_name":format!("DJI_{n:04}.JPG"),
                    "captured_at":format!("{date}T10:0{sequence}:00"),"archive":{"dng":"test.DNG"},
                    "mission":{"sessionID":session,"captureID":id},"plan_name":session}),
                );
                log.push(json!({"captureID":id,"lightClass":light,"qualityGrade":grade}));
            }
        }
        let rules = json!({"light_classes":["lowSun","night"],"dominance":0.6,"per_light":2,
            "poor_sample":1,"older_sample":1,"older_than_days":30});
        let labels = BTreeMap::from([("DJI_0000_20300101100000".to_owned(), "test".to_owned())]);
        let groups = select(&units, &json!({"captureLog":log}), &labels, &rules);
        assert_eq!(
            json!(groups),
            json!([
                {"reason":"labelled","detail":"test","asset_ids":["day-0"]},
                {"reason":"light","detail":"lowSun · day","asset_ids":["day-1"]},
                {"reason":"light","detail":"night · night","asset_ids":["night-0","night-1"]},
                {"reason":"poor","detail":"poor","asset_ids":["poor-0"]},
                {"reason":"older","detail":"older","asset_ids":["older-0"]}
            ])
        );
    }
}
