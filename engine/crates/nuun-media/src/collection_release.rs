//! Reviewed release of verified collection media to the source volume's Trash.

use crate::catalog::{Catalog, WritableCatalog, sql, writer_lock};
use crate::clipper_release::Guard;
use crate::delivery::ActionProgress;
use crate::living::{identity, required, source, stored, text, validate_source};
use crate::release::{checked_hash, consumers};
use crate::rules::relative;
use crate::transfer::Hook;
use crate::tree::Root;
use crate::volumes::{self, Backend, truthy};
use nu9ve_json::{Error, Result, digest, require, uid};
use nu9ve_volumes::fs;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "nu9ve-collection-release/1";
type Candidates = HashMap<u64, Vec<(String, String)>>;
type Records = BTreeMap<String, Value>;

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn paths(
    root: &Path,
    entries: &[Value],
    records: &Records,
) -> Result<Vec<(String, Option<String>)>> {
    entries
        .iter()
        .map(|entry| {
            let archive = records
                .get(text(entry, "archive_id"))
                .ok_or_else(|| Error::new("invalid_plan", "Unknown archive in reviewed entry"))?;
            Ok((
                root.join(relative(required(entry, "path")?)?)
                    .to_string_lossy()
                    .into_owned(),
                Some(
                    Path::new(text(archive, "root"))
                        .join(relative(required(entry, "archive_path")?)?)
                        .to_string_lossy()
                        .into_owned(),
                ),
            ))
        })
        .collect()
}

fn archives(
    catalog: &Catalog,
    backend: &dyn Backend,
    ids: &Value,
    info: &Value,
) -> Result<Records> {
    let ids = ids
        .as_array()
        .filter(|ids| !ids.is_empty())
        .ok_or_else(|| Error::new("invalid_request", "Choose at least one archive"))?;
    ids.iter()
        .map(|id| {
            let record = catalog.volume(id.as_str().unwrap_or_default())?;
            let name = text(&record, "name");
            require(
                record["role"] == "archive",
                "invalid_role",
                format!("Archive required: {name}"),
            )?;
            require(
                volumes::independent(info, &volumes::check(backend, &record)?),
                "same_physical_disk",
                format!("Archive and source must be independent disks: {name}"),
            )?;
            Ok((text(&record, "id").to_owned(), record))
        })
        .collect()
}

fn archive_index(
    catalog: &Catalog,
    ids: &Value,
    search: &Value,
    records: &Records,
    progress: &mut ActionProgress<'_>,
) -> Result<Candidates> {
    let mut index = Candidates::new();
    for id in ids.as_array().into_iter().flatten() {
        let archive = catalog.volume(id.as_str().unwrap_or_default())?;
        let id = text(&archive, "id");
        let name = text(&archive, "name");
        let archive = &records[id];
        let tree = Root::open(Path::new(text(archive, "root")))?;
        let mut starts = search
            .as_array()
            .into_iter()
            .flatten()
            .filter(|scope| scope["archive"] == id || scope["archive"] == name)
            .map(|scope| relative(text(scope, "path")))
            .collect::<Result<Vec<_>>>()?;
        if starts.is_empty() {
            starts.push("");
        }
        for start in starts {
            progress.stage = "indexing_archive";
            progress.meter.holding(start, name, "", "index");
            progress.report()?;
            crate::coverage::walk(&tree, start, |path, size| {
                if let Some(size) = size.filter(|size| *size > 0) {
                    index.entry(size).or_default().push((id.to_owned(), path));
                }
                Ok(())
            })
            .map_err(|error| {
                if fs::python_exception(&error) == Some("FileNotFoundError") {
                    Error::new(
                        "invalid_request",
                        format!("Search folder not found on {name}: {start}"),
                    )
                } else {
                    error
                }
            })?;
        }
    }
    Ok(index)
}

fn summary(entries: &[Value], kept: &[Value], uncovered: &[Value]) -> Vec<Value> {
    let mut groups = BTreeMap::<&str, [[u64; 2]; 3]>::new();
    for (index, rows) in [entries, kept, uncovered].into_iter().enumerate() {
        for row in rows {
            let folder = text(row, "path")
                .split_once('/')
                .map_or(".", |(folder, _)| folder);
            let size = if index == 0 {
                &row["stamp"]["size"]
            } else {
                &row["bytes"]
            };
            let counts = &mut groups.entry(folder).or_default()[index];
            counts[0] += 1;
            counts[1] += size.as_u64().unwrap_or_default();
        }
    }
    groups.into_iter().map(|(folder,[release,keep,uncovered])| json!({"folder":folder,"release":release,"keep":keep,"uncovered":uncovered})).collect()
}

fn mount(root: &Path, info: &Value) -> Result<PathBuf> {
    if let Some(path) = info["mount"].as_str().filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let mut at = root;
    let dev = std::fs::metadata(root)
        .map_err(|e| fs::io_error(e, root))?
        .dev();
    while let Some(parent) = at.parent() {
        if std::fs::metadata(parent)
            .map_err(|e| fs::io_error(e, parent))?
            .dev()
            != dev
        {
            break;
        }
        at = parent;
    }
    Ok(at.to_owned())
}

pub fn plan(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    plan_with(home, backend, params, |catalog, paths| {
        consumers(home, catalog, paths)
    })
}

fn plan_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    inspect: impl FnOnce(&Catalog, &[(String, Option<String>)]) -> Result<Value>,
) -> Result<Value> {
    require(
        params.as_object().is_some_and(|map| {
            map.keys().all(|key| {
                ["source", "archives", "search", "keep", "progress_id"].contains(&key.as_str())
            })
        }),
        "invalid_request",
        "Unknown parameters",
    )?;
    let root = source(required(params, "source")?)?;
    let catalog = Catalog::open(home)?;
    for volume in catalog.volumes()? {
        let registered = Path::new(text(&volume, "root"));
        require(
            !root.starts_with(registered) && !registered.starts_with(&root),
            "overlapping_roots",
            format!(
                "Source overlaps a registered volume: {}",
                text(&volume, "name")
            ),
        )?;
    }
    let info = backend.describe(&root, false)?;
    let volume = mount(&root, &info)?;
    require(
        root.starts_with(&volume),
        "invalid_request",
        "Source is not inside its volume",
    )?;
    let records = archives(&catalog, backend, &params["archives"], &info)?;
    let search = params["search"].as_array().cloned().unwrap_or_default();
    let writer = WritableCatalog::open(home)?;
    let mut progress = ActionProgress::new(&writer, params["progress_id"].as_str(), "preparing");
    progress.report()?;
    let index = archive_index(
        &catalog,
        &params["archives"],
        &json!(search),
        &records,
        &mut progress,
    )?;
    let keep = &params["keep"];
    let asd = keep.get("asd").is_none_or(truthy);
    let roots = keep["als_roots"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| crate::planner::absolute_source(v.as_str().unwrap_or_default()))
        .collect::<Result<Vec<_>>>()?;
    let explicit = keep["paths"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| relative(v.as_str().unwrap_or_default()))
        .collect::<Result<Vec<_>>>()?;
    let (mut names, mut sets, mut unreadable) = (BTreeSet::new(), 0, Vec::new());
    for root in &roots {
        let (count, issues) = crate::ableton::references(root, false, |_, value| {
            names.insert(
                value
                    .rsplit(['/', '\\', ':'])
                    .next()
                    .unwrap_or(value)
                    .to_lowercase(),
            );
        })?;
        sets += count;
        unreadable.extend(issues);
    }
    let origin = Root::open(&root)?;
    let mut listing = Vec::new();
    crate::coverage::walk(&origin, "", |path, size| {
        let size = size.ok_or_else(|| {
            Error::new(
                "unsafe_file",
                format!("Links/special files prevent release: {path}"),
            )
        })?;
        listing.push((path, size));
        Ok(())
    })?;
    let siblings: BTreeSet<_> = listing
        .iter()
        .map(|(path, _)| path.to_lowercase())
        .collect();
    let (mut entries, mut kept, mut uncovered, mut preserved, mut media) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (path, size) in listing {
        let lower = path.to_lowercase();
        if !crate::snapshot::kind_of(&path)
            .is_some_and(|kind| crate::layout::MEDIA_KINDS.contains(&kind))
        {
            preserved.push(path);
        } else if let Some(reason) = if asd && siblings.contains(&format!("{lower}.asd")) {
            Some("ableton_analysis")
        } else if names.contains(basename(&lower)) {
            Some("ableton_set")
        } else if explicit.iter().any(|p| Path::new(&path).starts_with(p)) {
            Some("explicit")
        } else {
            None
        } {
            kept.push(json!({"path":path,"bytes":size,"reason":reason}));
        } else if !index.contains_key(&size) {
            uncovered.push(json!({"path":path,"bytes":size,"reason":"no_same_size"}));
        } else {
            media.push((path, size));
        }
    }
    progress.stage = "hashing_source";
    progress.meter.files_total = media.len();
    progress.meter.total = media.iter().map(|(_, size)| size).sum();
    let mut done = 0;
    let mut cache = HashMap::new();
    let trees: BTreeMap<_, _> = records
        .iter()
        .map(|(id, record)| {
            Root::open(Path::new(text(record, "root"))).map(|tree| (id.as_str(), tree))
        })
        .collect::<Result<_>>()?;
    for (path, size) in media {
        progress.meter.holding(&path, "", "", "verify");
        progress.report()?;
        let (sha, stamp) = checked_hash(&origin, &path)?
            .ok_or_else(|| Error::new("source_changed", format!("Source changed: {path}")))?;
        let name = basename(&path).to_lowercase();
        let mut candidates: Vec<_> = index[&size].iter().collect();
        candidates.sort_by_key(|(_, candidate)| basename(candidate).to_lowercase() != name);
        let mut found = None;
        for (id, candidate) in candidates {
            let hash = match cache.entry((id, candidate)) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(checked_hash(&trees[id.as_str()], candidate)?)
                }
            };
            if let Some((_, archive_stamp)) = hash.as_ref().filter(|(hash, _)| *hash == sha) {
                found = Some(
                    json!({"path":path,"stamp":stamp,"sha256":sha,"archive_id":id,
                    "archive_path":candidate,"archive_stamp":archive_stamp}),
                );
                break;
            }
        }
        if let Some(entry) = found {
            entries.push(entry);
        } else {
            uncovered
                .push(json!({"path":path,"bytes":size,"sha256":sha,"reason":"no_identical_copy"}));
        }
        done += size;
        progress.meter.reach(done);
        progress.meter.finished("verified", &path, "");
    }
    progress.stage = "inspecting_clipper";
    progress.report()?;
    let mut blockers = json!([]);
    if !entries.is_empty() {
        let inspection = inspect(&catalog, &paths(&root, &entries, &records)?)
            .unwrap_or_else(|error| json!({"blockers":[error.message],"relinks":[]}));
        blockers = inspection["blockers"].clone();
        entries.retain(|entry| {
            let path = root.join(text(entry, "path"));
            if inspection["relinks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|relink| {
                    crate::clipper::overlaps(&path.to_string_lossy(), text(relink, "from"))
                })
            {
                kept.push(
                    json!({"path":entry["path"],"bytes":entry["stamp"]["size"],"reason":"clipper"}),
                );
                false
            } else {
                true
            }
        });
    }
    let id = uid()?;
    let bytes: u64 = entries
        .iter()
        .filter_map(|entry| entry["stamp"]["size"].as_u64())
        .sum();
    let mut value = json!({"schema":SCHEMA,"id":id,"created_at":nu9ve_time::now_utc(),"source_root":root,
        "source_identity":identity(&info),"volume_root":volume,"archives":records.keys().collect::<Vec<_>>(),"search":search,
        "keep_rules":{"asd":asd,"als_roots":roots,"paths":explicit,"ableton_sets":sets,"unreadable_sets":unreadable},
        "entries":entries,"kept":kept,"uncovered":uncovered,"preserved":preserved,"bytes":bytes,
        "consumers":{"blockers":blockers,"relinks":[]},"summary":summary(&entries,&kept,&uncovered),"state":"review","moved":{}});
    value["digest"] = json!(digest(&value));
    let _lock = writer_lock(home)?;
    writer.set(&format!("collection-release:{id}"), &value)?;
    Ok(value)
}

fn trash_parent(volume: &Root, rel: &str, device: u64, create: bool) -> Result<(PathBuf, String)> {
    volume.check()?;
    let (parent, name) = relative(rel)?
        .rsplit_once('/')
        .ok_or_else(|| Error::new("unsafe_path", "Invalid Trash path"))?;
    let mut dir = volume.path.clone();
    for part in parent.split('/') {
        dir.push(part);
        if create
            && let Err(error) = std::fs::DirBuilder::new().mode(0o700).create(&dir)
            && error.kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(fs::io_error(error, &dir));
        }
        let stat = std::fs::symlink_metadata(&dir).map_err(|e| fs::io_error(e, &dir))?;
        require(
            stat.is_dir() && stat.dev() == device,
            "unsafe_path",
            format!("Unsafe Trash folder: {}", dir.display()),
        )?;
    }
    Ok((dir, name.to_owned()))
}

pub fn apply(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    apply_with(home, backend, params, || Guard::acquire(home), &|_, _| {
        Ok(())
    })
}

fn apply_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    acquire: impl FnOnce() -> Result<Guard>,
    hook: Hook<'_>,
) -> Result<Value> {
    let id = required(params, "plan_id")?;
    let key = format!("collection-release:{id}");
    let catalog = Catalog::open(home)?;
    let plan = stored(&catalog, &key, params)?;
    if plan["state"] == "completed" {
        return Ok(plan);
    }
    let guard = acquire()?;
    let _lock = writer_lock(home)?;
    let mut value = stored(&catalog, &key, params)?;
    if value["state"] == "completed" {
        return Ok(value);
    }
    require(
        value["schema"] == SCHEMA
            && value["id"] == id
            && crate::captures::python_uuid(id).as_deref() == Some(id),
        "invalid_plan",
        "Invalid collection release plan",
    )?;
    let root = PathBuf::from(required(&value, "source_root")?);
    let volume = PathBuf::from(required(&value, "volume_root")?);
    let info = validate_source(backend, &root, &value["source_identity"])?;
    require(
        mount(&root, &info)? == volume,
        "source_changed",
        "Source volume mount changed",
    )?;
    let prefix = root
        .strip_prefix(&volume)
        .map_err(|_| Error::new("invalid_plan", "Source is not inside its volume"))?;
    let prefix = prefix
        .to_str()
        .ok_or_else(|| Error::new("unsafe_path", "Invalid source path"))?;
    if !prefix.is_empty() {
        relative(prefix)?;
    }
    let records = archives(&catalog, backend, &value["archives"], &info)?;
    let origin = Root::open(&root)?;
    let volume_tree = Root::open(&volume)?;
    let device = std::fs::metadata(&volume)
        .map_err(|e| fs::io_error(e, &volume))?
        .dev();
    let trees: BTreeMap<_, _> = records
        .iter()
        .map(|(id, record)| {
            Root::open(Path::new(text(record, "root"))).map(|tree| (id.as_str(), tree))
        })
        .collect::<Result<_>>()?;
    let entries = value["entries"]
        .as_array()
        .cloned()
        .ok_or_else(|| Error::new("invalid_plan", "Missing reviewed entries"))?;
    if entries
        .iter()
        .any(|entry| value["moved"][text(entry, "path")] != "done")
    {
        let mut referenced = BTreeSet::new();
        for folder in value["keep_rules"]["als_roots"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let (_, errors) = crate::ableton::references(
                Path::new(folder.as_str().unwrap_or_default()),
                false,
                |_, reference| {
                    referenced.insert(
                        reference
                            .rsplit(['/', '\\', ':'])
                            .next()
                            .unwrap_or(reference)
                            .to_lowercase(),
                    );
                },
            )?;
            require(
                errors.is_empty(),
                "dependencies_blocked",
                "An Ableton set could not be checked; review again",
            )?;
        }
        let mut analysis = BTreeSet::new();
        if truthy(&value["keep_rules"]["asd"]) {
            crate::coverage::walk(&origin, "", |path, size| {
                let path = path.to_lowercase();
                if size.is_some() && path.ends_with(".asd") {
                    analysis.insert(path);
                }
                Ok(())
            })?;
        }
        for entry in &entries {
            let path = text(entry, "path");
            if value["moved"][path] == "done" || origin.stat(path)?.is_none() {
                continue;
            }
            let has_analysis = analysis.contains(&format!("{}.asd", path.to_lowercase()));
            require(
                !has_analysis && !referenced.contains(&basename(path).to_lowercase()),
                "dependencies_blocked",
                format!("Ableton started using a file in this plan: {path}"),
            )?;
        }
    }
    let paths = paths(&root, &entries, &records)?;
    // SAFETY: getuid has no arguments or memory effects.
    let trash = format!(".Trashes/{}/nuun-{}", unsafe { libc::getuid() }, &id[..8]);
    let writer = WritableCatalog::open(home)?;
    let mut progress = ActionProgress::new(&writer, Some(id), "releasing");
    progress.meter.files_total = entries.len();
    progress.meter.total = value["bytes"].as_u64().unwrap_or_default();
    value["state"] = json!("applying");
    writer.set(&key, &value)?;
    let mut done = 0u64;
    let mut last_check = None;
    for entry in &entries {
        let path = text(entry, "path");
        let moved = &value["moved"][path];
        let from = if prefix.is_empty() {
            path.to_owned()
        } else {
            format!("{prefix}/{path}")
        };
        let to = format!("{trash}/{from}");
        let size = entry["stamp"]["size"].as_u64().unwrap_or_default();
        if moved != "done" {
            progress.meter.holding(path, "", "", "release");
            progress.report()?;
            if moved == "intent" && origin.stat(path)?.is_none() {
                trash_parent(&volume_tree, &to, device, false)?;
                require(
                    checked_hash(&volume_tree, &to)?.is_some_and(|(sha, _)| entry["sha256"] == sha),
                    "source_changed",
                    format!("Interrupted release left an unexpected file: {path}"),
                )?;
            } else {
                if last_check.is_none_or(|at: std::time::Instant| at.elapsed().as_secs() >= 60) {
                    let deps = guard.inspect(&paths)?;
                    require(
                        !truthy(&deps["blockers"]) && !truthy(&deps["relinks"]),
                        "dependencies_blocked",
                        "Clipper started using a file in this plan; review again",
                    )?;
                    last_check = Some(std::time::Instant::now());
                }
                validate_source(backend, &root, &value["source_identity"])?;
                let archive = text(entry, "archive_id");
                volumes::check(backend, &records[archive])?;
                hook("before_move", entry)?;
                let destination = text(entry, "archive_path");
                require(
                    checked_hash(&trees[archive], destination)?
                        .is_some_and(|(sha, _)| entry["sha256"] == sha),
                    "checksum_mismatch",
                    format!("Archive copy changed: {destination}"),
                )?;
                require(
                    checked_hash(&origin, path)?.is_some_and(|(sha, stamp)| {
                        entry["sha256"] == sha && entry["stamp"] == stamp
                    }),
                    "source_changed",
                    format!("Source changed since review: {path}"),
                )?;
                value["moved"][path] = json!("intent");
                writer.set(&key, &value)?;
                let (src, name) = volume_tree.parent(&from, false)?;
                let original = src.join(&name);
                require(
                    std::fs::symlink_metadata(&original)
                        .map_err(|e| fs::io_error(e, &original))?
                        .dev()
                        == device,
                    "unsafe_path",
                    "Source file left its volume",
                )?;
                let (dst, trash_name) = trash_parent(&volume_tree, &to, device, true)?;
                require(
                    volume_tree
                        .stat(&from)?
                        .is_some_and(|stamp| crate::tree::stamp_json(&stamp) == entry["stamp"]),
                    "source_changed",
                    format!("File changed before release: {path}"),
                )?;
                fs::rename_new(&original, &dst.join(trash_name))?;
                fs::sync_dir(&src)?;
                fs::sync_dir(&dst)?;
                hook("after_rename", entry)?;
            }
            value["moved"][path] = json!("done");
            let tx = writer.db().unchecked_transaction().map_err(sql)?;
            writer.set(&key, &value)?;
            writer.event(
                id,
                "collection_released",
                &json!({"path":path,"archive_id":entry["archive_id"],
                "archive_path":entry["archive_path"],"sha256":entry["sha256"],"trash":to}),
            )?;
            tx.commit().map_err(sql)?;
            hook("after_move", entry)?;
        }
        done += size;
        progress.meter.reach(done);
        progress.meter.finished("released", path, "");
    }
    value["state"] = json!("completed");
    value["completed_at"] = json!(nu9ve_time::now_utc());
    value["trash"] = json!(volume.join(trash));
    let tx = writer.db().unchecked_transaction().map_err(sql)?;
    writer.set(&key, &value)?;
    writer.event(
        id,
        "collection_release_completed",
        &json!({"bytes":value["bytes"],"files":entries.len(),"trash":value["trash"]}),
    )?;
    tx.commit().map_err(sql)?;
    progress.stage = "release_complete";
    progress.meter.current.clear();
    progress.report()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn music_plans_and_incremental_backup_match_python() {
        let s = crate::volumes::test_backend::setup();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let expected = &fixture["music"];
        for (name, body) in expected["files"].as_object().unwrap() {
            std::fs::write(s.card.join(name), body.as_str().unwrap()).unwrap();
        }
        std::fs::write(
            s.archive.join("renamed.wav"),
            expected["files"]["a.wav"].as_str().unwrap(),
        )
        .unwrap();
        let release = plan_with(
            &s.home,
            &s.backend,
            &json!({"source":s.card,"archives":["MEDIA"]}),
            |_, _| Ok(json!({"blockers":[],"relinks":[]})),
        )
        .unwrap();
        let options = json!({"source":s.card,"archive":"MEDIA","folder":"Music/backup"});
        let backup = crate::living::plan(&s.home, &s.backend, &options).unwrap();
        let applied = crate::living::apply(
            &s.home,
            &s.backend,
            &json!({"plan_id":backup["id"],"digest":backup["digest"]}),
        )
        .unwrap();
        let next = crate::living::plan(&s.home, &s.backend, &options).unwrap();
        fn normalize(value: &mut Value) {
            match value {
                Value::Object(map) => {
                    for key in [
                        "id",
                        "digest",
                        "created_at",
                        "completed_at",
                        "collection_id",
                        "ctime_ns",
                        "mtime_ns",
                        "inode",
                        "root_inode",
                    ] {
                        map.remove(key);
                    }
                    map.values_mut().for_each(normalize);
                }
                Value::Array(items) => items.iter_mut().for_each(normalize),
                _ => (),
            }
        }
        let root = s.card.parent().unwrap().to_string_lossy();
        for (key, mut value) in [
            ("release", release),
            ("backup", backup),
            ("applied", applied),
            ("next", next),
        ] {
            normalize(&mut value);
            let actual: Value = serde_json::from_str(
                &value
                    .to_string()
                    .replace(root.as_ref(), "<ROOT>")
                    .replace("id-MEDIA", "MEDIA"),
            )
            .unwrap();
            assert_eq!(actual, expected[key], "{key}");
        }
    }

    #[test]
    fn reviewed_release_keeps_ableton_media_and_resumes_a_verified_trash_move() {
        let s = crate::volumes::test_backend::setup();
        let source = Root::open(&s.card).unwrap();
        let target = Root::open(&s.archive).unwrap();
        source.write_bytes("2019/free.wav", b"free", false).unwrap();
        source.write_bytes("2019/set.wav", b"song", false).unwrap();
        source
            .write_bytes("2019/analysis.wav", b"keep", false)
            .unwrap();
        source
            .write_bytes("2019/analysis.wav.asd", b"analysis", false)
            .unwrap();
        target
            .write_bytes("audio/renamed.wav", b"free", false)
            .unwrap();
        let editor = s.home.join("ableton");
        std::fs::create_dir_all(&editor).unwrap();
        let set = editor.join("fixture.als");
        let xml = editor.join("fixture.xml");
        std::fs::write(
            &xml,
            r#"<Ableton><Path Value="samples/set.wav"/></Ableton>"#,
        )
        .unwrap();
        let bytes = std::process::Command::new("/usr/bin/gzip")
            .arg("-nc")
            .arg(&xml)
            .output()
            .unwrap();
        assert!(bytes.status.success());
        std::fs::write(&set, bytes.stdout).unwrap();
        let clipper = s.home.join("clipper");
        std::fs::write(&clipper,r#"#!/bin/sh
echo '{"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],"offline_projects":[],"projects":[]}'
"#).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&clipper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let writer = WritableCatalog::open(&s.home).unwrap();
        writer.set("clipper-executable", &json!(clipper)).unwrap();
        let review = plan_with(
            &s.home,
            &s.backend,
            &json!({"source":s.card,"archives":["MEDIA"],"keep":{"als_roots":[editor]}}),
            |_, _| Ok(json!({"blockers":[],"relinks":[]})),
        )
        .unwrap();
        assert_eq!(review["keep_rules"]["ableton_sets"], 1);
        assert_eq!(review["entries"].as_array().unwrap().len(), 1);
        assert_eq!(review["kept"].as_array().unwrap().len(), 2);
        let args = json!({"plan_id":review["id"],"digest":review["digest"]});
        let run = |hook: Hook<'_>| {
            apply_with(
                &s.home,
                &s.backend,
                &args,
                || Guard::acquire_at(&s.home, s.home.join("home")),
                hook,
            )
        };
        source
            .write_bytes("2019/free.wav.asd", b"new dependency", false)
            .unwrap();
        assert_eq!(
            run(&|_, _| Ok(())).unwrap_err().code,
            "dependencies_blocked"
        );
        source.unlink("2019/free.wav.asd", None).unwrap();
        target
            .write_bytes("audio/renamed.wav", b"bad!", true)
            .unwrap();
        assert_eq!(run(&|_, _| Ok(())).unwrap_err().code, "checksum_mismatch");
        target
            .write_bytes("audio/renamed.wav", b"free", true)
            .unwrap();
        let error = run(&|stage, _| require(stage != "after_rename", "interrupted", "fixture"))
            .unwrap_err();
        assert_eq!(error.code, "interrupted");
        let completed = run(&|_, _| Ok(())).unwrap();
        assert_eq!(completed["state"], "completed");
        assert_eq!(completed["moved"]["2019/free.wav"], "done");
        assert_eq!(
            std::fs::read(Path::new(text(&completed, "trash")).join("2019/free.wav")).unwrap(),
            b"free"
        );
        assert!(s.card.join("2019/set.wav").exists());
        assert!(s.card.join("2019/analysis.wav").exists());
    }
}
