//! Editorial closeout inventory. Clipper owns its project files and regenerable cleanup.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::integrations::{Integrations, invoke_clipper};
use crate::tree::Root;
use nu9ve_json::{Error, Result, require};
use nu9ve_volumes::{EntryKind, fs};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

pub(crate) fn folder_files(tree: &Root, scope: &str) -> Result<Vec<String>> {
    let mut files = Vec::new();
    let mut stack = vec![scope.to_owned()];
    while let Some(rel) = stack.pop() {
        let (dir, _) = match tree.parent(&format!("{rel}/x"), false) {
            Ok(found) => found,
            Err(error)
                if fs::python_exception(&error) == Some("FileNotFoundError") && rel == scope =>
            {
                break;
            }
            Err(error) => return Err(error),
        };
        let mut children = std::fs::read_dir(&dir)
            .map_err(|e| fs::io_error(e, &dir))?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| fs::io_error(e, &dir))?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| Error::new("unsafe_path", "Path is not valid UTF-8"))?;
            let lower = name.to_ascii_lowercase();
            if name.starts_with('.')
                || [
                    ".fcpbundle",
                    ".fcpcache",
                    ".stalker",
                    ".photoslibrary",
                    ".app",
                ]
                .iter()
                .any(|suffix| lower.ends_with(suffix))
            {
                continue;
            }
            let path = format!("{rel}/{name}");
            match nu9ve_volumes::stamp(&child.path())?.kind {
                EntryKind::Dir => stack.push(path),
                EntryKind::File => files.push(path),
                EntryKind::Link => {
                    return Err(Error::new(
                        "unsafe_file",
                        format!("Enlace en intermedios; se conserva: {path}"),
                    ));
                }
                _ => {
                    return Err(Error::new(
                        "unsafe_file",
                        format!("Archivo especial: {path}"),
                    ));
                }
            }
        }
    }
    Ok(files)
}

pub(crate) fn invoke(home: &Path, catalog: &Catalog, args: &[&str]) -> Result<Value> {
    let command = Integrations::read(catalog)?.clipper_executable()?;
    WritableCatalog::open(home)?.set("clipper-known", &json!(true))?;
    invoke_clipper(&command, args, Duration::from_secs(600), None)
}

fn validate_document(document: &Value, root: &Path) -> Result<()> {
    require(
        document["schema"] == "clipper-regenerable-files/1"
            && document["project_root"] == json!(crate::clipper::realpath(&root.to_string_lossy()))
            && matches!(
                document["project_state"].as_str(),
                Some("active" | "retired")
            )
            && document["inventory_complete"].is_boolean()
            && document["categories"].is_array()
            && document["entries"].is_array()
            && text(document, "digest").chars().count() == 64,
        "clipper_invalid",
        "Lista de regenerables de Clipper desconocida",
    )?;
    let mut ids = BTreeSet::new();
    for category in document["categories"].as_array().into_iter().flatten() {
        let id = text(category, "id");
        require(
            !id.is_empty()
                && category["files"].is_u64()
                && category["bytes"].is_u64()
                && category["default_selected"].is_boolean()
                && ids.insert(id),
            "clipper_invalid",
            "Categoría de regenerables incompleta",
        )?;
    }
    for entry in document["entries"].as_array().into_iter().flatten() {
        require(
            Path::new(text(entry, "path")).is_absolute()
                && ids.contains(text(entry, "category"))
                && entry["bytes"].is_u64()
                && entry["hardlink_duplicate"].is_boolean()
                && ["device", "inode", "size", "mtime_ns", "ctime_ns"]
                    .iter()
                    .all(|k| entry["stamp"][*k].is_u64()),
            "clipper_invalid",
            "Entrada de regenerables incompleta",
        )?;
    }
    Ok(())
}

pub(crate) fn regenerables(
    home: &Path,
    catalog: &Catalog,
    source: &Path,
    roots: &[std::path::PathBuf],
) -> (Vec<Value>, Vec<String>) {
    let mut groups = Vec::new();
    let mut notes = Vec::new();
    for root in roots {
        let rel = root.strip_prefix(source).unwrap_or(root).to_string_lossy();
        let document = invoke(home, catalog, &["cleanup-plan", &root.to_string_lossy()]).and_then(
            |document| {
                validate_document(&document, root)?;
                Ok(document)
            },
        );
        let document = match document {
            Ok(document) => document,
            Err(error) => {
                notes.push(format!(
                    "{rel}: Clipper no dio su lista de regenerables; se conserva todo ({})",
                    error.message
                ));
                continue;
            }
        };
        if document["inventory_complete"] != true {
            notes.push(format!(
                "{rel}: el inventario de Clipper está incompleto; se conserva todo"
            ));
            continue;
        }
        let mut categories = Vec::new();
        for category in document["categories"].as_array().into_iter().flatten() {
            if category["default_selected"] == true {
                categories.push(category);
            } else {
                let label = category["label"]
                    .as_str()
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| text(category, "id"));
                notes.push(format!(
                    "{rel}: {label} ({} archivos) se conserva; Clipper no lo preselecciona",
                    category["files"]
                ));
            }
        }
        if categories.is_empty() {
            continue;
        }
        let entries: Vec<_> = document["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|entry| {
                categories
                    .iter()
                    .any(|category| category["id"] == entry["category"])
            })
            .collect();
        let count = |field: &str| {
            categories
                .iter()
                .filter_map(|category| category[field].as_u64())
                .sum::<u64>()
        };
        groups.push(json!({"editor": rel, "project_root": document["project_root"], "project_state": document["project_state"],
            "digest": document["digest"], "categories": categories, "entries": entries,
            "files": count("files"), "bytes": count("bytes"), "document": document}));
    }
    (groups, notes)
}

pub(crate) fn cleanup(home: &Path, plan: &mut Value) -> Result<()> {
    let Some(groups) = plan["clipper_regenerable"]
        .as_array()
        .filter(|groups| !groups.is_empty())
    else {
        return Ok(());
    };
    let _lock = writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let key = format!("release:{}", text(plan, "id"));
    let mut stored = catalog
        .get(&key)?
        .ok_or_else(|| Error::new("invalid_plan", "Missing completed closeout"))?;
    require(
        stored["state"] == "completed" && stored["digest"] == plan["digest"],
        "plan_changed",
        "Use the recorded reviewed release plan",
    )?;
    let writer = WritableCatalog::open(home)?;
    for (index, group) in groups.iter().enumerate() {
        let root = text(group, "project_root");
        if stored["clipper_cleaned"]
            .get(root)
            .is_some_and(|ack| ack["error"].is_null())
        {
            continue;
        }
        let result = (|| {
            let name = format!("clipper-cleanup/{}-{index}.json", text(plan, "id"));
            let tree = Root::open(home)?;
            let path = tree.path.join(&name);
            tree.write_json(&name, &group["document"])?;
            let digest = text(group, "digest");
            let path = path.to_string_lossy();
            let mut args = vec!["cleanup", "--plan", &path, "--digest", digest];
            let mut categories: Vec<_> = group["categories"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|category| text(category, "id"))
                .collect();
            for category in &categories {
                args.extend(["--category", category]);
            }
            categories.sort_unstable();
            let ack = invoke(home, &catalog, &args)?;
            let mut returned: Vec<_> = ack["categories"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|id| id.as_str().unwrap_or_default())
                .collect();
            returned.sort_unstable();
            let skipped = ack["skipped"].as_array();
            require(
                ack["schema"] == "clipper-regenerable-cleanup/1"
                    && ack["plan_digest"] == digest
                    && ack["deleted_files"].is_u64()
                    && ack["deleted_bytes"].is_u64()
                    && skipped.is_some()
                    && returned == categories,
                "clipper_ack_invalid",
                "Clipper no confirmó el borrado de sus regenerables",
            )?;
            let skipped = skipped.unwrap();
            Ok(
                json!({"deleted_files": ack["deleted_files"], "deleted_bytes": ack["deleted_bytes"],
                "skipped": skipped.iter().take(200).collect::<Vec<_>>(), "skipped_count": skipped.len(), "at": nu9ve_time::now_utc()}),
            )
        })();
        stored["clipper_cleaned"][root] = result.unwrap_or_else(|error: Error| json!({
            "error": error.message, "deleted_files": 0, "deleted_bytes": 0, "skipped": [], "at": nu9ve_time::now_utc()
        }));
        writer.set(&key, &stored)?;
    }
    *plan = stored;
    Ok(())
}
