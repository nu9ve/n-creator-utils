//! Verified copies of processed work between two suite work disks (nu9ve-volume-v1 §7 and §8, suite-devices
//! §6.4): «copiar al otro disco» a folder of atlas-derived, or a project's folders, from VIAJE to EDICION.
//!
//! - Both disks are recognized by their marker (`ok` or `unverified`), never by their name, and must be `work` or
//!   `other`: an archive is nüün's own flow (`deliver_work` copies from it, ingest writes to it).
//! - The plan is reviewed (digest) before anything is written. Files keep their path inside the disk (or under
//!   `into`), their modification time, and land as `.name.ncpartial`, are read back from the device, then renamed
//!   without replacing anything ([`Transfer::copy_many`]). Identical files already there are adopted; a different
//!   one blocks the plan. Nothing is ever deleted, on either disk.
//! - nüün reads every app's index on the source (`.nu9ve/apps/*/index.json`) to say what the copy carries and of
//!   which projects, and keeps byte-exact, verified copies of those apps' top-level files inside its own folder of
//!   the destination, so the app can register the copy without hashing again. It never writes another app's
//!   folder.
//! - The receipt, `nuun-work-copy/1`, is published in `.nu9ve/apps/nuun/copies/<id>.json` on both disks and kept
//!   in the local catalog. docs/contracts/suite-work-copy-v1.md is its contract.
use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::rules::relative;
use crate::suite_volume;
use crate::transfer::{Hook, Target, Transfer, hash_file};
use crate::tree::{Root, stamp_json};
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, Result, digest, require, sha256_hex, uid};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "nuun-work-copy-plan/1";
pub const RECEIPT: &str = "nuun-work-copy/1";
pub const COPIES: &str = ".nu9ve/apps/nuun/copies";
const APPS: &str = ".nu9ve/apps";
/// Another app's top-level file kept with the copy: indexes and their own records, never bulk data.
const APP_FILE_LIMIT: u64 = 16 * 1024 * 1024;
const ROLES: [&str; 2] = ["work", "other"];
const SHOWN: usize = 500;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn absolute(params: &Value, key: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text(params, key));
    require(
        path.is_absolute(),
        "invalid_params",
        format!("'{key}' must be the absolute root of a mounted disk"),
    )?;
    Ok(path)
}

fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        path.to_owned()
    } else if path.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}/{path}")
    }
}

/// `a` is `b` or inside it, by path components.
fn within(a: &str, b: &str) -> bool {
    b.is_empty() || a == b || a.starts_with(&format!("{b}/"))
}

/// A folder the suite or nüün keeps for itself: never copied as work.
fn reserved(path: &str) -> bool {
    matches!(
        path.split('/').next(),
        Some(".nu9ve" | ".ncreator" | "_nuun")
    )
}

/// The synthetic destination record [`Transfer`] checks before every write: this mount, with the identity seen
/// when the plan was reviewed.
fn record(volume: &Value, root: &Path, identity: &Value) -> Value {
    json!({"id":volume["volume_id"],"name":volume["label"],"root":volumes::normalize(root),
        "role":volume["role"],"identity":identity})
}

/// What `rules::IDENTITY` compares (`volumes::check`), from a description.
fn identity(info: &Value) -> Value {
    let mut out = Map::new();
    for key in ["mode", "volume_uuid", "disk_uuid", "root_inode", "external"] {
        out.insert(key.into(), info[key].clone());
    }
    Value::Object(out)
}

/// A folder of the disk, or `None` when it is not there (a disk with no app folders is normal).
fn folder(root: &Root, rel: &str) -> Result<Option<PathBuf>> {
    match root.parent(&format!("{rel}/x"), false) {
        Ok((dir, _)) => Ok(Some(dir)),
        Err(error) if crate::tree::absent(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The apps whose folder is on this disk and what their index says lives here, resolved to disk paths.
fn app_indexes(root: &Root, volume_id: &str) -> Result<Vec<Value>> {
    let Some(dir) = folder(root, APPS)? else {
        return Ok(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| nu9ve_volumes::marker::app_name(name))
        .collect();
    names.sort();
    let mut apps = Vec::new();
    for app in names {
        let files = app_files(root, &app)?;
        let mut entry = json!({"app":app,"files":files.iter().map(|(name, size)| json!({"name":name,"bytes":size}))
            .collect::<Vec<_>>()});
        match suite_volume::app_index(root, &app) {
            Ok(None) => entry["index"] = json!("absent"),
            Err(problem) => {
                entry["index"] = json!("invalid");
                entry["problem"] = json!(problem);
            }
            Ok(Some(index)) => {
                entry["index"] = json!("ok");
                entry["generated_at"] = index["generated_at"].clone();
                let roots: BTreeMap<&str, &str> = index["roots"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|r| r["volume_id"] == volume_id)
                    .filter_map(|r| Some((r["id"].as_str()?, r["volume_path"].as_str()?)))
                    .collect();
                let titles: BTreeMap<&str, &Value> = index["projects"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| Some((p["project_key"].as_str()?, &p["title"])))
                    .collect();
                let mut datasets = Vec::new();
                let mut root_projects: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
                for dataset in index["datasets"].as_array().into_iter().flatten() {
                    let location = &dataset["location"];
                    let Some(base) = roots.get(text(location, "root_id")) else {
                        continue;
                    };
                    let path = join(base, text(location, "path"));
                    let keys: Vec<&str> = dataset["projects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|p| p["project_key"].as_str())
                        .collect();
                    root_projects
                        .entry(base)
                        .or_default()
                        .extend(keys.iter().copied());
                    // From a personal dataset, the shape only (data §6): the index is on the disk by the person's
                    // decision, but a copy plan or a receipt is not that decision.
                    let personal =
                        dataset["privacy"] != "public" && dataset["privacy"] != "restricted";
                    let mut shown = json!({"id":dataset["id"],"path":path,"modality":dataset["modality"],
                        "privacy":dataset["privacy"],"size":dataset["size"],"state":dataset["state"],
                        "projects":keys});
                    if !personal {
                        shown["title"] = dataset["title"].clone();
                    }
                    if let Some(hash) = dataset.get("hash") {
                        shown["hash"] = hash.clone();
                    }
                    datasets.push(shown);
                }
                entry["projects"] = json!(
                    titles
                        .iter()
                        .map(|(k, t)| json!({"project_key":k,"title":t}))
                        .collect::<Vec<_>>()
                );
                entry["roots"] = json!(
                    roots
                        .iter()
                        .map(|(id, path)| json!({"id":id,"volume_path":path}))
                        .collect::<Vec<_>>()
                );
                entry["root_projects"] = json!(root_projects);
                entry["dataset_count"] = json!(datasets.len());
                datasets.truncate(SHOWN);
                entry["datasets"] = json!(datasets);
            }
        }
        apps.push(entry);
    }
    Ok(apps)
}

/// The regular files directly in an app's folder (its index and own records), with their size.
fn app_files(root: &Root, app: &str) -> Result<Vec<(String, u64)>> {
    let Some(dir) = folder(root, &format!("{APPS}/{app}"))? else {
        return Ok(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut files: Vec<(String, u64)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let meta = std::fs::symlink_metadata(entry.path()).ok()?;
            let name = entry.file_name().into_string().ok()?;
            (meta.is_file() && !name.starts_with('.')).then_some((name, meta.len()))
        })
        .collect();
    files.sort();
    Ok(files)
}

/// What each app's index says this disk holds (`volume_apps`'s `apps`), for the suggestions.
pub(crate) fn app_indexes_of(root: &Root, volume_id: &str) -> Result<Vec<Value>> {
    app_indexes(root, volume_id)
}

/// The work-copy receipts published on this disk (`volume_apps`'s `copies`).
pub(crate) fn volume_apps_copies(root: &Root) -> Result<Vec<Value>> {
    receipts(root)
}

/// `volume_apps`: a disk's marker, its state here and what each app's index says it holds. A read: it writes
/// nothing but this device's sighting of the disk, as every observation does (volume §4).
pub fn volume_apps(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let path = absolute(params, "root")?;
    let (root, _, volume, state) = suite_volume::usable(home, backend, &path)?;
    let mut apps = app_indexes(&root, text(&volume, "volume_id"))?;
    if let Some(nuun) = apps
        .iter_mut()
        .find(|app| app["app"] == "nuun" && app["index"] == "ok")
        && let Some(datasets) = nuun["datasets"].as_array_mut()
    {
        let recipe = nuun_recipe::effective(Catalog::open(home)?.get("recipe")?.as_ref());
        for dataset in datasets {
            if let Some(paths) = mission_companions(text(dataset, "path"), &recipe) {
                dataset["work_copy_companions"] = json!(paths);
            }
        }
    }
    let copies = receipts(&root)?;
    Ok(json!({"state":state,"marker":volume,"apps":apps,"copies":copies}))
}

fn mission_companions(path: &str, recipe: &Value) -> Option<[String; 3]> {
    let path = format!("/{path}/");
    recipe["missions"].as_array()?.iter().find_map(|flow| {
        let folder = relative(text(flow, "work_folder")).ok()?;
        let mirror = relative(text(flow, "work_mirror")).ok()?;
        let units = relative(text(flow, "units_map")).ok()?;
        let (prefix, _) = path.split_once(&format!("/{folder}/"))?;
        let prefix = prefix.trim_start_matches('/');
        Some([
            join(prefix, units),
            join(prefix, &format!("{mirror}/history.json")),
            join(prefix, &format!("{mirror}/changes.ndjson")),
        ])
    })
}

/// The work-copy receipts published on a disk, oldest first, whole (with their file lists).
pub(crate) fn full_receipts(root: &Root) -> Vec<Value> {
    let Ok(Some(dir)) = folder(root, COPIES) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Value> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| {
            let id = name.strip_suffix(".json")?.to_owned();
            let bytes = crate::tree::read_bytes(&root.path, &format!("{COPIES}/{name}"), 64 << 20)
                .ok()??;
            let value: Value = serde_json::from_slice(&bytes).ok()?;
            (value["contract"] == RECEIPT && value["id"] == id.as_str()).then_some(value)
        })
        .collect();
    out.sort_by(|a, b| text(a, "completed_at").cmp(text(b, "completed_at")));
    out
}

/// The work-copy receipts published on a disk, newest first (only their headline).
fn receipts(root: &Root) -> Result<Vec<Value>> {
    let Some(dir) = folder(root, COPIES)? else {
        return Ok(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.filter_map(|entry| entry.ok()) {
        let Some(name) = entry.file_name().into_string().ok() else {
            continue;
        };
        let Some(id) = name.strip_suffix(".json") else {
            continue;
        };
        let Ok(Some(bytes)) =
            crate::tree::read_bytes(&root.path, &format!("{COPIES}/{name}"), 64 << 20)
        else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            out.push(json!({"id":id,"problem":"not_json"}));
            continue;
        };
        if value["contract"] != RECEIPT || value["id"] != id {
            out.push(json!({"id":id,"problem":"not_a_receipt"}));
            continue;
        }
        out.push(json!({"id":id,"completed_at":value["completed_at"],"source":value["source"],
            "destination":value["destination"],"file_count":value["file_count"],"bytes":value["bytes"],
            "manifest":value["manifest"],"projects":value["context"]["projects"]}));
    }
    out.sort_by(|a, b| text(b, "completed_at").cmp(text(a, "completed_at")));
    Ok(out)
}

struct Side {
    root: Root,
    info: Value,
    volume: Value,
}

fn side(home: &Path, backend: &dyn Backend, path: &Path, which: &str) -> Result<Side> {
    let (root, info, volume, _) = suite_volume::usable(home, backend, path)?;
    require(
        ROLES.contains(&text(&volume, "role")),
        "invalid_role",
        format!("The {which} must be a work disk; an archive has its own flows"),
    )?;
    Ok(Side { root, info, volume })
}

fn check_paths(params: &Value) -> Result<(Vec<String>, String)> {
    let allowed = [
        "source",
        "destination",
        "paths",
        "into",
        "progress_id",
        "update",
    ];
    require(
        params
            .as_object()
            .is_some_and(|p| p.keys().all(|k| allowed.contains(&k.as_str()))),
        "invalid_params",
        "Unknown parameters",
    )?;
    let paths: Vec<String> = params["paths"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 64)
        .ok_or_else(|| Error::new("invalid_params", "Choose 1 to 64 folders or files to copy"))?
        .iter()
        .map(|p| {
            let path = relative(p.as_str().unwrap_or_default())?;
            require(
                !reserved(path),
                "invalid_params",
                "The suite's and nüün's own folders are not copied as work",
            )?;
            Ok(path.to_owned())
        })
        .collect::<Result<_>>()?;
    for (i, a) in paths.iter().enumerate() {
        for b in &paths[i + 1..] {
            require(
                !within(a, b) && !within(b, a),
                "invalid_params",
                format!("Overlapping paths: {a} and {b}"),
            )?;
        }
    }
    let into = match params.get("into") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) if s.is_empty() => String::new(),
        Some(Value::String(s)) => {
            relative(s)?;
            require(
                !reserved(s),
                "invalid_params",
                "Reserved destination folder",
            )?;
            s.clone()
        }
        _ => return Err(Error::new("invalid_params", "'into' must be a folder")),
    };
    require(
        params.get("update").is_none_or(Value::is_boolean),
        "invalid_params",
        "'update' is true or false",
    )?;
    Ok((paths, into))
}

/// The regular files at or under `top` of a disk, with their stamp; links and special files stop the walk.
pub(crate) fn files_under(root: &Root, top: &str) -> Result<Vec<(String, nu9ve_volumes::Stamp)>> {
    root.check()?;
    let kind = nu9ve_volumes::stamp(&root.path.join(top))
        .map_err(|_| Error::new("source_missing", format!("Not on the source disk: {top}")))?
        .kind;
    let mut found = Vec::new();
    match kind {
        nu9ve_volumes::EntryKind::Dir => crate::coverage::walk(root, top, |path, size| {
            require(
                size.is_some(),
                "unsafe_file",
                format!("Links and special files are not followed: {path}"),
            )?;
            found.push(path);
            Ok(())
        })?,
        nu9ve_volumes::EntryKind::File => found.push(top.to_owned()),
        _ => {
            return Err(Error::new(
                "unsafe_file",
                format!("Links and special files are not followed: {top}"),
            ));
        }
    }
    found
        .into_iter()
        .map(|path| {
            let stamp = root
                .stat(&path)?
                .ok_or_else(|| Error::new("source_changed", format!("Source changed: {path}")))?;
            Ok((path, stamp))
        })
        .collect()
}

fn hash_at(root: &Root, rel: &str) -> Result<Option<String>> {
    let Some(file) = root.open_read(rel)? else {
        return Ok(None);
    };
    let mut file = nu9ve_volumes::fs::verify_read(file);
    Ok(Some(hash_file(&mut file, |_| {})?))
}

/// What a verified receipt between two disks says about one file, by its path on today's destination (U1.3).
struct Known {
    /// Its path on today's source.
    source: String,
    size: Value,
    mtime_ns: Value,
    receipt: Value,
}

/// U1.3: the files a published receipt between `source_id` and `dest_id` verified, in either direction, keyed by
/// their path on today's destination. Both ends were read back and matched then, with the source's time kept;
/// the newest receipt wins.
fn verified(dest: &Root, source_id: &str, dest_id: &str) -> BTreeMap<String, Known> {
    let mut known = BTreeMap::new();
    for receipt in full_receipts(dest) {
        let from = text(&receipt["source"], "volume_id");
        let to = text(&receipt["destination"], "volume_id");
        let forward = from == source_id && to == dest_id;
        if !forward && !(from == dest_id && to == source_id) {
            continue;
        }
        for file in receipt["files"].as_array().into_iter().flatten() {
            let (here, there) = (text(file, "source_path"), text(file, "path"));
            let (here, there) = if forward {
                (here, there)
            } else {
                (there, here)
            };
            known.insert(
                there.to_owned(),
                Known {
                    source: here.to_owned(),
                    size: file["size"].clone(),
                    mtime_ns: file["mtime_ns"].clone(),
                    receipt: receipt["id"].clone(),
                },
            );
        }
    }
    known
}

/// `work_copy_plan`: what would be copied from one work disk to another, reviewed before anything is written.
///
/// A file already on the destination is compared by its hash on both disks, unless a receipt between these two
/// disks verified it and both ends still have that size and modification time (U1.3): then it is identical
/// without reading it again. The copy itself still reads it back before adopting it.
pub fn plan(home: &Path, backend: &dyn Backend, isolated: bool, params: &Value) -> Result<Value> {
    let (paths, into) = check_paths(params)?;
    let source = side(home, backend, &absolute(params, "source")?, "source")?;
    let dest = side(
        home,
        backend,
        &absolute(params, "destination")?,
        "destination",
    )?;
    require(
        source.volume["volume_id"] != dest.volume["volume_id"],
        "same_volume",
        "Choose two different disks",
    )?;
    require(
        volumes::independent(&source.info, &dest.info),
        "same_physical_disk",
        "Source and destination must be independent disks",
    )?;
    let writer = WritableCatalog::open(home)?;
    let mut progress = crate::action::ActionProgress::new(
        &writer,
        params["progress_id"].as_str(),
        crate::action::Action::planning("copy"),
        "preparing",
    )?;
    progress.report()?;
    let mut items = Vec::new();
    let (mut bytes, mut adopt, mut adopt_bytes) = (0u64, 0usize, 0u64);
    let mut conflicts = Vec::new();
    let update = params["update"] == true;
    let (mut replace, mut replace_bytes) = (0usize, 0u64);
    let known = verified(
        &dest.root,
        text(&source.volume, "volume_id"),
        text(&dest.volume, "volume_id"),
    );
    // First the list, with what a receipt already verified; then only what is left is read, with a measure.
    let mut listed = Vec::new();
    for top in &paths {
        for (path, stamp) in files_under(&source.root, top)? {
            let target = join(&into, &path);
            let existing = dest.root.stat(&target)?;
            let receipt = existing.as_ref().and_then(|existing| {
                known.get(&target).filter(|k| {
                    k.source == path
                        && [stamp.size, existing.size]
                            .iter()
                            .all(|s| k.size == json!(s))
                        && [stamp.mtime_ns, existing.mtime_ns]
                            .iter()
                            .all(|t| k.mtime_ns == json!(t))
                })
            });
            listed.push((path, target, stamp, existing, receipt));
            if listed.len().is_multiple_of(2000) {
                progress.stage = "listing_source";
                progress.meter.files_done = listed.len();
                progress.report()?;
            }
        }
    }
    let unread = |e: &&(
        String,
        String,
        nu9ve_volumes::Stamp,
        Option<nu9ve_volumes::Stamp>,
        Option<&Known>,
    )| { e.3.is_some() && e.4.is_none() };
    progress.stage = "reviewing";
    progress.meter.total = listed.iter().filter(unread).map(|e| e.2.size * 2).sum();
    progress.meter.files_total = listed.iter().filter(unread).count();
    progress.meter.files_done = 0;
    progress.report()?;
    let (source_label, dest_label) = (
        text(&source.volume, "label").to_owned(),
        text(&dest.volume, "label").to_owned(),
    );
    let mut read = 0u64;
    for (path, target, stamp, existing, receipt) in listed {
        let mut item = json!({"path":path,"target":target,"stamp":stamp_json(&stamp)});
        if let Some(existing) = existing {
            let (same, there) = match receipt {
                Some(_) => (true, None),
                None => {
                    progress
                        .meter
                        .holding(&target, &dest_label, &source_label, "verify");
                    progress.report()?;
                    let there = hash_at(&dest.root, &target)?;
                    read += existing.size;
                    progress.meter.reach(read);
                    progress.report()?;
                    let same =
                        existing.size == stamp.size && there == hash_at(&source.root, &path)?;
                    read += stamp.size;
                    progress.meter.reach(read);
                    progress.meter.finished("verified", &target, &dest_label);
                    progress.report()?;
                    (same, there)
                }
            };
            if same {
                item["existing"] = json!("identical");
                if let Some(known) = receipt {
                    item["verified_by"] = known.receipt.clone();
                }
                adopt += 1;
                adopt_bytes += stamp.size;
            } else if update {
                // «Actualizar»: the reviewed version on the destination is kept in nüün's folder, never deleted.
                item["existing"] = json!("different");
                item["previous"] = json!({"sha256":there,"stamp":stamp_json(&existing)});
                replace += 1;
                replace_bytes += stamp.size;
                bytes += stamp.size;
            } else {
                conflicts.push(target.clone());
            }
        } else {
            bytes += stamp.size;
        }
        items.push(item);
    }
    progress.meter.current.clear();
    require(
        conflicts.is_empty(),
        "destination_conflict",
        format!(
            "{} different files already exist on the destination (review again with «update» to keep their versions): {}",
            conflicts.len(),
            conflicts
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )?;
    require(
        dest.info["free"].as_u64().is_none_or(|free| bytes <= free),
        "insufficient_space",
        format!("Not enough free space on the destination for {bytes} bytes"),
    )?;
    // What the apps of the source say these paths are, and their own files to carry with the copy.
    let source_id = text(&source.volume, "volume_id").to_owned();
    let apps = app_indexes(&source.root, &source_id)?;
    let mut context = Vec::new();
    let mut projects = BTreeSet::new();
    let mut carried = Vec::new();
    let mut titles = BTreeMap::new();
    for app in &apps {
        let touching: Vec<&Value> = app["datasets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| {
                let path = text(d, "path");
                paths.iter().any(|p| within(p, path) || within(path, p))
            })
            .collect();
        if touching.is_empty() {
            continue;
        }
        for project in app["projects"].as_array().into_iter().flatten() {
            titles.insert(
                text(project, "project_key").to_owned(),
                project["title"].clone(),
            );
        }
        for dataset in &touching {
            projects.extend(
                dataset["projects"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str().map(str::to_owned)),
            );
        }
        let name = text(app, "app");
        for file in app["files"].as_array().into_iter().flatten() {
            let size = file["bytes"].as_u64().unwrap_or(u64::MAX);
            if size > APP_FILE_LIMIT {
                continue;
            }
            let rel = format!("{APPS}/{name}/{}", text(file, "name"));
            let stamp = source
                .root
                .stat(&rel)?
                .ok_or_else(|| Error::new("source_changed", format!("Source changed: {rel}")))?;
            let sha = hash_at(&source.root, &rel)?
                .ok_or_else(|| Error::new("source_changed", format!("Source changed: {rel}")))?;
            carried.push(
                json!({"app":name,"file":file["name"],"source":rel,"bytes":stamp.size,
                "sha256":sha,"stamp":stamp_json(&stamp)}),
            );
        }
        context.push(json!({"app":name,"generated_at":app["generated_at"],
            "datasets":touching.into_iter().take(SHOWN).collect::<Vec<_>>()}));
    }
    let device = crate::suite_device::current(isolated, &json!({"peek":true}))?;
    let id = uid()?;
    let mut value = json!({"schema":SCHEMA,"id":id,"created_at":nu9ve_time::now_utc(),
        "source":{"volume_id":source_id,"label":source.volume["label"],"root":volumes::normalize(&source.root.path),
            "identity":identity(&source.info),"paths":paths},
        "destination":{"volume_id":dest.volume["volume_id"],"label":dest.volume["label"],
            "root":volumes::normalize(&dest.root.path),"identity":identity(&dest.info),"into":into},
        "copied_by":{"device_id":device["device"]["device_id"],"app":"nuun"},
        "files":items,"file_count":items.len(),"bytes":bytes,"adopt":{"files":adopt,"bytes":adopt_bytes},
        "update":update,"replace":{"files":replace,"bytes":replace_bytes},
        "free":dest.info["free"],"context":{"projects":projects,"apps":context,
            "titles":titles.into_iter().filter(|(k, _)| projects.contains(k)).collect::<Map<_, _>>()},
        "carried":carried,
        "state":"review","done":{}});
    value["digest"] = json!(digest(&value));
    let _lock = writer_lock(home)?;
    writer.set(&format!("work-copy-plan:{id}"), &value)?;
    progress.stage = "complete";
    progress.report()?;
    Ok(value)
}

/// `work_copy_apply`: the reviewed plan, copied and verified file by file. Interrupted, the same call resumes:
/// what was already copied is adopted after hashing it again.
pub fn apply(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    apply_with(home, backend, params, &|_, _| Ok(()))
}

fn reopen(home: &Path, backend: &dyn Backend, side: &Value, which: &str) -> Result<(Root, Value)> {
    let path = PathBuf::from(text(side, "root"));
    let (root, info, volume, _) = suite_volume::usable(home, backend, &path)?;
    require(
        volume["volume_id"] == side["volume_id"] && identity(&info) == side["identity"],
        "volume_replaced",
        format!("The {which} disk changed since the review"),
    )?;
    let record = record(&volume, &path, &side["identity"]);
    Ok((root, record))
}

pub(crate) fn apply_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    hook: Hook<'_>,
) -> Result<Value> {
    let id = text(params, "plan_id");
    let key = format!("work-copy-plan:{id}");
    let _lock = writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let mut value = catalog
        .get(&key)?
        .filter(|plan| plan["digest"] == params["digest"] && plan["schema"] == SCHEMA)
        .ok_or_else(|| Error::new("plan_changed", "Use the recorded reviewed plan"))?;
    if value["state"] == "completed" {
        return Ok(value);
    }
    let (origin, source_record) = reopen(home, backend, &value["source"], "source")?;
    let (target, dest_record) = reopen(home, backend, &value["destination"], "destination")?;
    let writer = WritableCatalog::open(home)?;
    value["state"] = json!("applying");
    writer.set(&key, &value)?;
    let files = value["files"].as_array().cloned().unwrap_or_default();
    let carried = value["carried"].as_array().cloned().unwrap_or_default();
    let mut progress = crate::action::ActionProgress::new(
        &writer,
        Some(id),
        crate::action::Action::new("copy"),
        "copying",
    )?;
    progress.meter.total = files
        .iter()
        .chain(&carried)
        .filter_map(|item| item["stamp"]["size"].as_u64())
        .sum();
    progress.meter.files_total = files.len() + carried.len();
    let mut transfer = Transfer::new(home, &writer, backend, &|_, _| Ok(()));
    let mut done_bytes = 0u64;
    let label = text(&value["destination"], "label").to_owned();
    let source_label = text(&value["source"], "label").to_owned();
    let copies = format!("{COPIES}/{id}");
    let jobs = files
        .iter()
        .map(|f| {
            (
                text(f, "path").to_owned(),
                text(f, "target").to_owned(),
                f["stamp"].clone(),
                None,
                f.get("previous").cloned(),
            )
        })
        .chain(carried.iter().map(|c| {
            (
                text(c, "source").to_owned(),
                format!("{copies}/source/{}/{}", text(c, "app"), text(c, "file")),
                c["stamp"].clone(),
                Some(text(c, "sha256").to_owned()),
                None,
            )
        }));
    for (path, dest, stamp, expected, previous) in jobs {
        relative(&path)?;
        relative(&dest)?;
        let size = stamp["size"].as_u64().unwrap_or_default();
        if value["done"][&dest].is_object() {
            done_bytes += size;
            progress.meter.reach(done_bytes);
            progress.meter.files_done += 1;
            continue;
        }
        progress.meter.holding(&path, &source_label, &label, "copy");
        progress.report()?;
        hook("before_copy", &json!({"path":path}))?;
        let mut kept = None;
        if let Some(previous) = &previous {
            let at = format!("{copies}/previous/{dest}");
            if target.stat(&dest)?.is_some()
                && hash_at(&target, &dest)?.as_deref() == previous["sha256"].as_str()
            {
                // Moved, not copied: the same disk, a rename that never replaces. Interrupted after it, the new
                // version is simply copied on resume.
                volumes::check(backend, &dest_record)?;
                target.parent(&at, true)?;
                target.rename_new(&dest, &at)?;
                hook("after_keep", &json!({"path":path}))?;
            }
            if target.stat(&at)?.is_some() {
                kept = Some(json!({"sha256":previous["sha256"],"path":at}));
            }
        }
        let existed = target.stat(&dest)?.is_some();
        let result = transfer.copy_many(
            &origin,
            &path,
            &[Target {
                root: &target,
                rel: dest.clone(),
                record: &dest_record,
            }],
            expected.as_deref(),
            Some(&stamp),
        )?;
        let (sha, _) = &result[0];
        let state = match (&kept, existed) {
            (_, true) => "adopted",
            (Some(_), false) => "replaced",
            (None, false) => "copied",
        };
        value["done"][&dest] = json!({"sha256":sha,"state":state});
        if let Some(kept) = kept {
            value["done"][&dest]["previous"] = kept;
        }
        writer.set(&key, &value)?;
        hook("after_copy", &json!({"path":path}))?;
        done_bytes += size;
        progress.meter.reach(done_bytes.min(progress.meter.total));
        progress
            .meter
            .finished(if existed { "adopted" } else { "copied" }, &path, &label);
    }
    // The receipt: every destination file with the hash its verification read, and a manifest hash as the index's.
    let mut listed = Vec::new();
    let mut manifest: Vec<(String, String)> = Vec::new();
    let (mut copied, mut adopted, mut replaced) = (0usize, 0usize, 0usize);
    for file in &files {
        let dest = text(file, "target");
        let done = &value["done"][dest];
        let sha = text(done, "sha256").to_owned();
        match done["state"].as_str() {
            Some("adopted") => adopted += 1,
            Some("replaced") => replaced += 1,
            _ => copied += 1,
        }
        manifest.push((dest.to_owned(), sha.clone()));
        let mut entry = json!({"source_path":file["path"],"path":dest,"size":file["stamp"]["size"],
            "mtime_ns":file["stamp"]["mtime_ns"],"sha256":sha});
        if done["previous"].is_object() {
            entry["previous"] = done["previous"].clone();
        }
        listed.push(entry);
    }
    manifest.sort();
    let lines: String = manifest
        .iter()
        .map(|(path, sha)| format!("{path}\t{sha}\n"))
        .collect();
    let completed = nu9ve_time::now_utc();
    let carried_out: Vec<Value> = carried
        .iter()
        .map(|c| {
            json!({"app":c["app"],"file":c["file"],"bytes":c["bytes"],"sha256":c["sha256"],
            "copy":format!("{copies}/source/{}/{}", text(c, "app"), text(c, "file"))})
        })
        .collect();
    let receipt = json!({"contract":RECEIPT,"id":id,"plan_digest":value["digest"],"copied_by":value["copied_by"],
        "created_at":value["created_at"],"completed_at":completed,
        "source":{"volume_id":value["source"]["volume_id"],"label":value["source"]["label"],
            "paths":value["source"]["paths"]},
        "destination":{"volume_id":value["destination"]["volume_id"],"label":value["destination"]["label"],
            "into":value["destination"]["into"]},
        "file_count":files.len(),"bytes":files.iter().map(|f| f["stamp"]["size"].as_u64().unwrap_or_default()).sum::<u64>(),
        "copied":copied,"adopted":adopted,"replaced":replaced,
        "manifest":{"algorithm":"sha256","of":"manifest","value":sha256_hex(lines.as_bytes()),"verified_at":completed},
        "files":listed,"source_indexes":carried_out,"context":value["context"]});
    let bytes = nu9ve_json::canonical(&receipt);
    let rel = format!("{COPIES}/{id}.json");
    // On both disks, in nüün's own folder: the destination says where it came from; the source, where it went.
    for (root, record) in [(&target, &dest_record), (&origin, &source_record)] {
        volumes::check(backend, record)?;
        if root.stat(&rel)?.is_none() {
            root.write_bytes(&rel, &bytes, false)?;
        }
        require(
            crate::tree::read_bytes(&root.path, &rel, bytes.len() + 1)?.as_deref()
                == Some(&bytes[..]),
            "checksum_mismatch",
            format!(
                "Could not read back the receipt on {}",
                text(record, "name")
            ),
        )?;
    }
    value["state"] = json!("completed");
    value["completed_at"] = json!(completed);
    value["receipt"] = json!({"path":rel,"sha256":sha256_hex(&bytes),"copied":copied,"adopted":adopted,
        "replaced":replaced,
        "manifest":receipt["manifest"]});
    let tx = writer
        .db()
        .unchecked_transaction()
        .map_err(crate::catalog::sql)?;
    writer.set(&key, &value)?;
    writer.event(id, "work_copy_completed", &value["receipt"])?;
    tx.commit().map_err(crate::catalog::sql)?;
    progress.stage = "complete";
    progress.meter.reach(progress.meter.total);
    progress.report()?;
    Ok(value)
}

/// `work_copies`: the plans and receipts kept in this catalog, newest first, without their file lists.
pub fn list(home: &Path) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let rows = catalog.json_rows(
        "SELECT value FROM metadata WHERE key LIKE 'work-copy-plan:%'",
        [],
    )?;
    let mut out: Vec<Value> = rows
        .into_iter()
        .map(|plan: Value| {
            json!({"id":plan["id"],"state":plan["state"],"created_at":plan["created_at"],
                "completed_at":plan["completed_at"],"digest":plan["digest"],
                "source":{"volume_id":plan["source"]["volume_id"],"label":plan["source"]["label"],"paths":plan["source"]["paths"]},
                "destination":{"volume_id":plan["destination"]["volume_id"],"label":plan["destination"]["label"],"into":plan["destination"]["into"]},
                "file_count":plan["file_count"],"bytes":plan["bytes"],"receipt":plan["receipt"]})
        })
        .collect();
    out.sort_by(|a, b| text(b, "created_at").cmp(text(a, "created_at")));
    Ok(json!({"copies":out}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::test_backend::Fixture;

    const VIAJE: &str = "1b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e01";
    const EDICION: &str = "2b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e02";

    struct Disks {
        _temp: tempfile::TempDir,
        home: PathBuf,
        viaje: PathBuf,
        edicion: PathBuf,
        backend: Fixture,
    }

    fn disk(backend: &mut Fixture, path: &Path, id: &str, role: &str, serial: &str) {
        std::fs::create_dir_all(path).unwrap();
        let marker = json!({"contract":"nu9ve-volume/1","volume_id":id,"role":role,"label":
            path.file_name().unwrap().to_str().unwrap(),"registered_at":"2026-11-22T00:00:00Z",
            "registered_by":{"app":"nove","device_id":id},
            "fs":{"os":crate::suite_device::OS,"kind":"exfat","serial":serial}});
        Root::open(path)
            .unwrap()
            .write_json(".nu9ve/volume.json", &marker)
            .unwrap();
        backend.roots.push(path.to_path_buf());
        let overrides = if crate::suite_device::OS == "windows" {
            json!({"device": u32::from_str_radix(&serial.replace('-', ""), 16).unwrap()})
        } else {
            json!({"volume_uuid": serial})
        };
        backend
            .overrides
            .insert(volumes::normalize(path), overrides);
    }

    fn disks() -> Disks {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let mut backend = Fixture::default();
        let (viaje, edicion) = (base.join("VIAJE"), base.join("EDICION"));
        disk(&mut backend, &viaje, VIAJE, "work", "AAAA-0001");
        disk(&mut backend, &edicion, EDICION, "work", "AAAA-0002");
        let x = Root::open(&viaje).unwrap();
        x.write_bytes(
            "atlas-derived/potholes/day/a.geojson",
            b"{\"type\":\"FeatureCollection\"}",
            false,
        )
        .unwrap();
        x.write_bytes(
            "atlas-derived/potholes/day/severity.json",
            b"[1,2,3]",
            false,
        )
        .unwrap();
        x.write_bytes("atlas-derived/other/keep.txt", b"not copied", false)
            .unwrap();
        // Processed long ago: the copy keeps this time.
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        for name in ["a.geojson", "severity.json"] {
            std::fs::File::options()
                .write(true)
                .open(viaje.join("atlas-derived/potholes/day").join(name))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        let index = json!({"contract":"nu9ve-data/1","app":"nove","generated_at":"2026-11-23T00:00:00Z",
            "projects":[{"project_key":"subaereo","title":"Subaéreo","parent_key":null,"state":"active"}],
            "roots":[{"id":"root-1","label":"atlas-derived","state":"ok","volume_id":VIAJE,"volume_path":"atlas-derived"}],
            "datasets":[{"id":"ds-1","owner":"nove","title":"Baches","modality":"geo_vector","privacy":"public",
                "storage":"in_place","location":{"root_id":"root-1","path":"potholes/day/a.geojson"},
                "projects":[{"project_key":"subaereo"}],"state":"unverified","updated_at":"2026-11-23T00:00:00Z"}],
            "next_cursor":null});
        x.write_json(".nu9ve/apps/nove/index.json", &index).unwrap();
        x.write_bytes(
            ".nu9ve/apps/nove/inplace-v1.json",
            b"{\"contract\":\"nove-volume-index/1\"}",
            false,
        )
        .unwrap();
        Disks {
            _temp: temp,
            home: base.join("state"),
            viaje,
            edicion,
            backend,
        }
    }

    fn ask(d: &Disks, paths: Value) -> Value {
        json!({"source":d.viaje,"destination":d.edicion,"paths":paths})
    }

    fn flows(d: &Disks) -> Vec<Value> {
        let user = d.home.join("user");
        std::fs::create_dir_all(&user).unwrap();
        crate::suggestions::suggestions(&d.home, &d.backend, &user, &Value::Null, false)
            .unwrap()["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["flow"] == "work_copy_changed")
            .cloned()
            .collect()
    }

    #[test]
    fn processed_work_goes_back_updating_with_the_previous_version_kept() {
        let d = disks();
        let there = plan(
            &d.home,
            &d.backend,
            true,
            &ask(&d, json!(["atlas-derived/potholes"])),
        )
        .unwrap();
        apply(
            &d.home,
            &d.backend,
            &json!({"plan_id":there["id"],"digest":there["digest"]}),
        )
        .unwrap();
        assert!(flows(&d).is_empty());
        // Processed on the EDICION: one file changed, one new.
        let e = Root::open(&d.edicion).unwrap();
        let rel = "atlas-derived/potholes/day/severity.json";
        std::fs::write(d.edicion.join(rel), b"[9]").unwrap();
        e.write_bytes("atlas-derived/potholes/day/out.tif", b"tif", false)
            .unwrap();
        let found = flows(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0]["key"],
            format!("work_copy_changed:{EDICION}:{VIAJE}")
        );
        assert_eq!(found[0]["count"], 2);
        assert_eq!(found[0]["paths"][0]["path"], "atlas-derived/potholes");
        assert_eq!(found[0]["paths"][0]["new"], 1);
        let back =
            json!({"source":d.edicion,"destination":d.viaje,"paths":["atlas-derived/potholes"]});
        let error = plan(&d.home, &d.backend, true, &back).unwrap_err();
        assert_eq!(error.code, "destination_conflict");
        let mut ask_update = back.clone();
        ask_update["update"] = json!(true);
        let reviewed = plan(&d.home, &d.backend, true, &ask_update).unwrap();
        assert_eq!(reviewed["replace"]["files"], 1);
        assert_eq!(reviewed["adopt"]["files"], 1);
        let args = json!({"plan_id":reviewed["id"],"digest":reviewed["digest"]});
        // Interrupted right after the old version was moved aside: resuming copies the new one.
        let error = apply_with(&d.home, &d.backend, &args, &|stage, _| {
            require(stage != "after_keep", "interrupted", "fixture")
        })
        .unwrap_err();
        assert_eq!(error.code, "interrupted");
        assert!(!d.viaje.join(rel).exists());
        let done = apply(&d.home, &d.backend, &args).unwrap();
        assert_eq!(done["receipt"]["replaced"], 1);
        assert_eq!(std::fs::read(d.viaje.join(rel)).unwrap(), b"[9]");
        let kept = format!(
            "{COPIES}/{}/previous/{rel}",
            reviewed["id"].as_str().unwrap()
        );
        assert_eq!(std::fs::read(d.viaje.join(&kept)).unwrap(), b"[1,2,3]");
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(d.viaje.join(text(&done["receipt"], "path"))).unwrap(),
        )
        .unwrap();
        let entry = receipt["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == rel)
            .unwrap();
        assert_eq!(entry["previous"]["path"], kept.as_str());
        assert!(flows(&d).is_empty());
    }

    #[test]
    fn what_both_disks_already_hold_alike_is_not_proposed_back() {
        let d = disks();
        let there = plan(
            &d.home,
            &d.backend,
            true,
            &ask(&d, json!(["atlas-derived/potholes"])),
        )
        .unwrap();
        apply(
            &d.home,
            &d.backend,
            &json!({"plan_id":there["id"],"digest":there["digest"]}),
        )
        .unwrap();
        // Another flow writes the same file to both disks with the same date (the conversations export does).
        let rel = "atlas-derived/potholes/day/both.json";
        let at = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_790_000_000);
        for disk in [&d.viaje, &d.edicion] {
            std::fs::write(disk.join(rel), b"{}").unwrap();
            std::fs::File::options()
                .write(true)
                .open(disk.join(rel))
                .unwrap()
                .set_modified(at)
                .unwrap();
        }
        assert!(flows(&d).is_empty(), "{:?}", flows(&d));
        // Only on one side, it is still proposed, from that side.
        Root::open(&d.edicion)
            .unwrap()
            .write_bytes("atlas-derived/potholes/day/only.json", b"[]", false)
            .unwrap();
        let found = flows(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0]["key"],
            format!("work_copy_changed:{EDICION}:{VIAJE}")
        );
        assert_eq!(found[0]["count"], 1);
        assert_eq!(found[0]["paths"][0]["new"], 1);
    }

    #[test]
    fn indexed_mission_copies_include_atlas_history_and_photo_map() {
        let recipe = nuun_recipe::template();
        assert_eq!(
            mission_companions("001_proyectos/atlas-subaereo/misiones/2026/11", &recipe),
            Some([
                "001_proyectos/atlas-subaereo/_nuun/units.ndjson".to_owned(),
                "001_proyectos/atlas-subaereo/_missions/history.json".to_owned(),
                "001_proyectos/atlas-subaereo/_missions/changes.ndjson".to_owned(),
            ])
        );
        assert!(mission_companions("001_proyectos/atlas-derived", &recipe).is_none());
        assert!(mission_companions("xatlas-subaereo/misiones/2026/11", &recipe).is_none());
    }

    #[test]
    fn other_apps_indexes_say_what_a_disk_holds() {
        let d = disks();
        let seen = volume_apps(&d.home, &d.backend, &json!({"root":d.viaje})).unwrap();
        assert_eq!(seen["state"], "ok");
        let nove = &seen["apps"][0];
        assert_eq!(nove["app"], "nove");
        assert_eq!(nove["index"], "ok");
        assert_eq!(
            nove["datasets"][0]["path"],
            "atlas-derived/potholes/day/a.geojson"
        );
        assert_eq!(nove["datasets"][0]["title"], "Baches");
        assert_eq!(nove["files"].as_array().unwrap().len(), 2);
        // An index in the wrong folder is reported, never trusted, and the disk stays usable.
        Root::open(&d.viaje)
            .unwrap()
            .write_bytes(
                ".nu9ve/apps/atlas/index.json",
                b"{\"app\":\"nove\",\"next_cursor\":null}",
                false,
            )
            .unwrap();
        let seen = volume_apps(&d.home, &d.backend, &json!({"root":d.viaje})).unwrap();
        assert_eq!(seen["apps"][0]["app"], "atlas");
        assert_eq!(seen["apps"][0]["problem"], "app_mismatch");
    }

    #[test]
    fn a_folder_is_copied_verified_with_its_times_and_a_receipt_on_both_disks() {
        let d = disks();
        let reviewed = plan(
            &d.home,
            &d.backend,
            true,
            &ask(&d, json!(["atlas-derived/potholes"])),
        )
        .unwrap();
        assert_eq!(reviewed["file_count"], 2);
        assert_eq!(reviewed["context"]["projects"], json!(["subaereo"]));
        assert_eq!(reviewed["carried"].as_array().unwrap().len(), 2);
        let args = json!({"plan_id":reviewed["id"],"digest":reviewed["digest"]});
        // Interrupted after the first file: the same call resumes and adopts what was verified.
        let error = apply_with(&d.home, &d.backend, &args, &|stage, _| {
            require(stage != "after_copy", "interrupted", "fixture")
        })
        .unwrap_err();
        assert_eq!(error.code, "interrupted");
        let done = apply(&d.home, &d.backend, &args).unwrap();
        assert_eq!(done["state"], "completed");
        for name in ["a.geojson", "severity.json"] {
            let rel = format!("atlas-derived/potholes/day/{name}");
            assert_eq!(
                std::fs::read(d.edicion.join(&rel)).unwrap(),
                std::fs::read(d.viaje.join(&rel)).unwrap()
            );
            let times = |p: &Path| std::fs::metadata(p.join(&rel)).unwrap().modified().unwrap();
            assert_eq!(times(&d.edicion), times(&d.viaje));
        }
        assert!(!d.edicion.join("atlas-derived/other").exists());
        // nüün never writes another app's folder; it keeps the source's files, byte for byte, in its own.
        assert!(!d.edicion.join(".nu9ve/apps/nove").exists());
        let id = text(&reviewed, "id");
        assert_eq!(
            std::fs::read(
                d.edicion
                    .join(format!("{COPIES}/{id}/source/nove/inplace-v1.json"))
            )
            .unwrap(),
            std::fs::read(d.viaje.join(".nu9ve/apps/nove/inplace-v1.json")).unwrap()
        );
        let receipt: Value = serde_json::from_slice(
            &std::fs::read(d.edicion.join(format!("{COPIES}/{id}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["contract"], RECEIPT);
        assert_eq!(receipt["source"]["volume_id"], VIAJE);
        assert_eq!(receipt["destination"]["volume_id"], EDICION);
        assert_eq!(
            receipt["copied"].as_u64().unwrap() + receipt["adopted"].as_u64().unwrap(),
            2
        );
        assert_eq!(receipt["files"][0]["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(
            std::fs::read(d.viaje.join(format!("{COPIES}/{id}.json"))).unwrap(),
            std::fs::read(d.edicion.join(format!("{COPIES}/{id}.json"))).unwrap()
        );
        assert_eq!(apply(&d.home, &d.backend, &args).unwrap(), done);
        let listed = volume_apps(&d.home, &d.backend, &json!({"root":d.edicion})).unwrap();
        assert_eq!(listed["copies"][0]["id"], reviewed["id"]);
        assert_eq!(list(&d.home).unwrap()["copies"][0]["state"], "completed");
        // Copying again: everything is already there and identical, so it is adopted, never replaced.
        let again = plan(
            &d.home,
            &d.backend,
            true,
            &ask(&d, json!(["atlas-derived/potholes"])),
        )
        .unwrap();
        assert_eq!(again["adopt"]["files"], 2);
        assert_eq!(again["bytes"], 0);
    }

    #[test]
    fn conflicts_archives_and_reserved_folders_block_the_plan() {
        let mut d = disks();
        Root::open(&d.edicion)
            .unwrap()
            .write_bytes("atlas-derived/potholes/day/severity.json", b"[9]", false)
            .unwrap();
        let code = |d: &Disks, p: Value| plan(&d.home, &d.backend, true, &p).unwrap_err().code;
        assert_eq!(
            code(&d, ask(&d, json!(["atlas-derived/potholes"]))),
            "destination_conflict"
        );
        assert_eq!(code(&d, ask(&d, json!([".nu9ve/apps"]))), "invalid_params");
        assert_eq!(code(&d, ask(&d, json!(["a", "a/b"]))), "invalid_params");
        assert_eq!(code(&d, ask(&d, json!(["../x"]))), "unsafe_path");
        let same = json!({"source":d.viaje,"destination":d.viaje,"paths":["atlas-derived"]});
        assert_eq!(code(&d, same), "same_volume");
        let media = d.viaje.parent().unwrap().join("MEDIA");
        disk(
            &mut d.backend,
            &media,
            "3b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e03",
            "archive",
            "AAAA-0003",
        );
        let to_archive =
            json!({"source":d.viaje,"destination":media,"paths":["atlas-derived/other"]});
        assert_eq!(code(&d, to_archive), "invalid_role");
        // A reviewed plan does not survive a disk swapped under the same mount.
        let reviewed = plan(
            &d.home,
            &d.backend,
            true,
            &ask(&d, json!(["atlas-derived/other"])),
        )
        .unwrap();
        d.backend
            .overrides
            .get_mut(&volumes::normalize(&d.edicion))
            .unwrap()["root_inode"] = json!(7);
        let args = json!({"plan_id":reviewed["id"],"digest":reviewed["digest"]});
        assert_eq!(
            apply(&d.home, &d.backend, &args).unwrap_err().code,
            "volume_replaced"
        );
    }

    /// U1.3: after a verified copy, the review trusts the receipt for what still has its size and time, in
    /// both directions, and reads again what does not.
    #[test]
    fn a_receipt_spares_reading_again_what_kept_its_size_and_time() {
        let d = disks();
        let paths = json!(["atlas-derived/potholes"]);
        let first = plan(&d.home, &d.backend, true, &ask(&d, paths.clone())).unwrap();
        assert!(first["files"][0].get("verified_by").is_none());
        apply(
            &d.home,
            &d.backend,
            &json!({"plan_id":first["id"],"digest":first["digest"]}),
        )
        .unwrap();
        let again = plan(&d.home, &d.backend, true, &ask(&d, paths.clone())).unwrap();
        assert_eq!(again["adopt"]["files"], 2);
        for file in again["files"].as_array().unwrap() {
            assert_eq!(file["verified_by"], first["id"], "{file}");
        }
        let back =
            json!({"source":d.edicion,"destination":d.viaje,"paths":["atlas-derived/potholes"]});
        let reverse = plan(&d.home, &d.backend, true, &back).unwrap();
        assert!(
            reverse["files"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["verified_by"] == first["id"])
        );
        // The same size and time is trusted without reading: that is the rule (and the apply reads it back).
        let rel = "atlas-derived/potholes/day/severity.json";
        let old = std::fs::metadata(d.edicion.join(rel))
            .unwrap()
            .modified()
            .unwrap();
        std::fs::write(d.edicion.join(rel), b"[1,2,4]").unwrap();
        let file = std::fs::File::options()
            .write(true)
            .open(d.edicion.join(rel))
            .unwrap();
        file.set_modified(old).unwrap();
        drop(file);
        let trusted = plan(&d.home, &d.backend, true, &ask(&d, paths.clone())).unwrap();
        assert_eq!(trusted["adopt"]["files"], 2);
        // Another time: read again, and the different content blocks the plan.
        let file = std::fs::File::options()
            .write(true)
            .open(d.edicion.join(rel))
            .unwrap();
        file.set_modified(old + std::time::Duration::from_secs(60))
            .unwrap();
        drop(file);
        let error = plan(&d.home, &d.backend, true, &ask(&d, paths)).unwrap_err();
        assert_eq!(error.code, "destination_conflict");
    }

    /// U1.3 · how long the review of «Copiar al otro disco» takes once everything is already copied
    /// (64 files of 8 MiB). Run by hand: `cargo test -p nuun-media review_of_a_finished_copy -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measure, printed; run by hand"]
    fn review_of_a_finished_copy() {
        let d = disks();
        let x = Root::open(&d.viaje).unwrap();
        let block = vec![7u8; 8 << 20];
        for index in 0..64 {
            x.write_bytes(&format!("atlas-derived/big/{index:02}.bin"), &block, false)
                .unwrap();
        }
        let paths = json!(["atlas-derived/big"]);
        let first = plan(&d.home, &d.backend, true, &ask(&d, paths.clone())).unwrap();
        apply(
            &d.home,
            &d.backend,
            &json!({"plan_id":first["id"],"digest":first["digest"]}),
        )
        .unwrap();
        let mut times = Vec::new();
        for _ in 0..3 {
            let started = std::time::Instant::now();
            let again = plan(&d.home, &d.backend, true, &ask(&d, paths.clone())).unwrap();
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(again["adopt"]["files"], 64);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "review of 64 × 8 MiB already copied: median {:.0} ms ({:.0}–{:.0})",
            times[1], times[0], times[2]
        );
    }
}
