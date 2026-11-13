//! The coverage report compares an editing disk against an archive without modifying either volume.

use crate::catalog::Catalog;
use crate::rules::relative;
use crate::scan::{IGNORED_DIRS, IGNORED_FILES};
use crate::tree::Root;
use crate::volumes::{self, Backend, truthy};
use nu9ve_json::{Error, Result, require};
use nu9ve_volumes::{EntryKind, fs};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Visit regular files and special entries in the order used by Python's coverage walk.
pub(crate) fn walk(
    tree: &Root,
    start: &str,
    mut visit: impl FnMut(String, Option<u64>) -> Result<()>,
) -> Result<()> {
    let mut stack = vec![start.to_owned()];
    while let Some(folder) = stack.pop() {
        let marker = if folder.is_empty() {
            "x".to_owned()
        } else {
            format!("{folder}/x")
        };
        let (dir, _) = tree.parent(&marker, false)?;
        let mut children = std::fs::read_dir(&dir)
            .map_err(|e| fs::io_error(e, &dir))?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| fs::io_error(e, &dir))?;
        children.sort_by_cached_key(|entry| entry.file_name().to_string_lossy().into_owned());
        for child in children {
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| Error::new("operation_failed", "Path is not valid UTF-8"))?;
            if IGNORED_FILES.contains(&name.as_str())
                || name.starts_with("._")
                || name.ends_with(".ncpartial")
            {
                continue;
            }
            let rel = if folder.is_empty() {
                name.clone()
            } else {
                format!("{folder}/{name}")
            };
            let path = child.path();
            let stamp = nu9ve_volumes::stamp(&path)?;
            match stamp.kind {
                EntryKind::Dir if !IGNORED_DIRS.contains(&name.as_str()) && name != ".ncreator" => {
                    stack.push(rel)
                }
                EntryKind::Dir => (),
                EntryKind::File => visit(rel, Some(stamp.size))?,
                _ => visit(rel, None)?,
            }
        }
    }
    Ok(())
}

fn digest_of(tree: &Root, path: &str) -> Result<String> {
    let file = tree
        .open_read(path)?
        .ok_or_else(|| Error::new("operation_failed", format!("[FileNotFoundError] {path}")))?;
    crate::transfer::hash_file(&mut fs::verify_read(file), |_| {})
}

#[derive(Default)]
struct Amount {
    files: u64,
    bytes: u64,
}

#[derive(Default)]
struct Bucket {
    in_archive: Amount,
    size_match: Amount,
    only_work: Amount,
}

impl Bucket {
    fn view(&self) -> Value {
        json!({"in_archive": self.in_archive.view(), "size_match": self.size_match.view(),
               "only_work": self.only_work.view()})
    }
}

impl Amount {
    fn view(&self) -> Value {
        json!({"files": self.files, "bytes": self.bytes})
    }

    fn add(&mut self, bytes: u64) {
        self.files += 1;
        self.bytes += bytes;
    }
}

pub fn report(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let required = |key| {
        params
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
    };
    let work = required("work")?;
    let archive = required("archive")?;
    let catalog = Catalog::open(home)?;
    let work = catalog.volume(work)?;
    let archive = catalog.volume(archive)?;
    require(
        work["role"] == "work" && archive["role"] == "archive",
        "invalid_role",
        "Compare a work volume against an archive volume",
    )?;
    volumes::check(backend, &work)?;
    volumes::check(backend, &archive)?;
    let start = if truthy(&params["path"]) {
        relative(params["path"].as_str().unwrap_or_default())?
    } else {
        ""
    };
    let hashed = !truthy(&params["no_hash"]);
    let work_tree = Root::open(Path::new(work["root"].as_str().unwrap_or_default()))?;
    let archive_tree = Root::open(Path::new(archive["root"].as_str().unwrap_or_default()))?;
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();
    walk(&archive_tree, "", |rel, size| {
        if let Some(size) = size {
            by_size.entry(size).or_default().push(rel);
        }
        Ok(())
    })?;

    let mut archive_hashes = HashMap::new();
    let mut folders: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut totals = Bucket::default();
    let mut only_work = Vec::new();
    let mut special = Vec::new();
    walk(&work_tree, start, |rel, size| {
        let folder = if rel.contains('/') {
            rel.split('/').take(2).collect::<Vec<_>>().join("/")
        } else {
            ".".to_owned()
        };
        let bucket = folders.entry(folder).or_default();
        let Some(size) = size else {
            special.push(rel);
            return Ok(());
        };
        if let Some(candidates) = by_size.get(&size) {
            if !hashed {
                bucket.size_match.add(size);
                totals.size_match.add(size);
                return Ok(());
            }
            let wanted = digest_of(&work_tree, &rel)?;
            for candidate in candidates {
                let sha = match archive_hashes.entry(candidate) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(digest_of(&archive_tree, candidate)?)
                    }
                };
                if *sha == wanted {
                    bucket.in_archive.add(size);
                    totals.in_archive.add(size);
                    return Ok(());
                }
            }
        }
        bucket.only_work.add(size);
        totals.only_work.add(size);
        only_work.push(json!({"path": rel, "bytes": size}));
        Ok(())
    })?;
    let folders: BTreeMap<_, _> = folders
        .into_iter()
        .map(|(key, bucket)| (key, bucket.view()))
        .collect();
    let totals = totals.view();
    let created = nu9ve_time::now_utc();
    let rel = format!("reports/coverage-{}.json", created.replace(':', "-"));
    let report = json!({"schema": "nu9ve-coverage/1", "created_at": created, "work": work["name"],
                        "archive": archive["name"], "path": start, "hashed": hashed, "folders": folders,
                        "only_work": only_work, "links_or_special": special, "totals": totals});
    let state = Root::open(home)?;
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|e| Error::new("operation_failed", e.to_string()))?;
    state.write_bytes(&rel, &bytes, false)?;
    Ok(
        json!({"report": state.path.join(rel), "totals": totals, "folders": folders,
              "only_work_sample": only_work.iter().take(50).collect::<Vec<_>>()}),
    )
}
