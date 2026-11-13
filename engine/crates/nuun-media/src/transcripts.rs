//! Verified copies of Clipper transcripts linked to archived members.

use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use crate::clipper::realpath;
use crate::delivery::ActionProgress;
use crate::integrations::{Integrations, invoke_clipper};
use crate::reads::{project_record, related};
use crate::release::checked_hash;
use crate::transfer::{Target, Transfer};
use crate::tree::Root;
use crate::volumes::{self, Backend, truthy};
use nu9ve_json::{Error, Result, digest, require, uid};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

const AREA: &str = "004_datos/clipper/transcripts";
const STAMP_KEYS: [&str; 5] = ["device", "inode", "size", "mtime_ns", "ctime_ns"];
const RECEIPT_FIELDS: [&str; 8] = [
    "asset_id",
    "member",
    "member_destination",
    "format",
    "sha256",
    "bytes",
    "target",
    "action",
];

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

pub(crate) fn editor_roots(
    catalog: &Catalog,
    project_id: &str,
    volume: &Value,
) -> Result<Vec<String>> {
    let (_, profile, title) = project_record(catalog, project_id)?;
    let aliases = catalog.json_rows(
        "SELECT json_quote(alias) FROM project_aliases WHERE project_id=?",
        [project_id],
    )?;
    let mut titles = BTreeSet::from([title]);
    titles.extend(aliases.iter().filter_map(Value::as_str).map(str::to_owned));
    Ok(titles
        .into_iter()
        .map(|title| format!("{}/{profile}/{title}", text(volume, "prefix")))
        .collect())
}

pub(crate) fn clipper_roots(volume: &Value, editors: &[String]) -> Result<Vec<PathBuf>> {
    let base = Path::new(text(volume, "root"));
    let tree = Root::open(base)?;
    let mut roots = Vec::new();
    for editor in editors {
        for rel in [format!("{editor}/clipper"), editor.clone()] {
            if tree.stat(&format!("{rel}/clipper-project.json"))?.is_some()
                || tree
                    .stat(&format!("{rel}/clipper-retired-project.json"))?
                    .is_some()
            {
                roots.push(base.join(rel));
                break;
            }
        }
    }
    Ok(roots)
}

fn validate_export(data: &Value, root: &Path) -> Result<()> {
    require(
        data["schema"] == "clipper-transcript-export/1"
            && data["project_root"].as_str().is_some_and(|reported| {
                let resolved = realpath(&root.to_string_lossy());
                Path::new(reported) == resolved
                    || cfg!(windows)
                        && Path::new(reported).is_absolute()
                        && realpath(reported) == resolved
            })
            && data["entries"].is_array()
            && text(data, "digest").chars().count() == 64,
        "clipper_invalid",
        "Exportación de transcripciones de Clipper desconocida",
    )?;
    for entry in data["entries"].as_array().into_iter().flatten() {
        let sha = text(entry, "sha256");
        require(
            Path::new(text(entry, "path")).is_absolute()
                && [
                    "transcript.json",
                    "provider-response.json",
                    "srt",
                    "vtt",
                    "ass",
                ]
                .contains(&text(entry, "format"))
                && entry["bytes"].is_u64()
                && entry["bytes"] == entry["stamp"]["size"]
                && sha.len() == 64
                && sha
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                && STAMP_KEYS.iter().all(|key| {
                    entry["stamp"][*key].as_i64().is_some()
                        || entry["stamp"][*key].as_u64().is_some()
                })
                && entry["source"]["path"].is_string(),
            "clipper_invalid",
            "Entrada de transcripción incompleta",
        )?;
    }
    Ok(())
}

fn archived_copy(
    catalog: &Catalog,
    archive_id: &str,
    job: &str,
    path: &str,
) -> Result<Option<Value>> {
    Ok(catalog
        .json_rows(
            "SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?",
            (job, archive_id, path),
        )?
        .into_iter()
        .next())
}

fn archived_member(
    catalog: &Catalog,
    archive_id: &str,
    source: &Value,
) -> Result<Option<(Value, String, String)>> {
    let asset_id = text(source, "asset_id");
    let sha = text(source, "member_sha256");
    if !asset_id.is_empty()
        && !sha.is_empty()
        && let Some(asset) = catalog
            .json_rows("SELECT payload FROM assets WHERE id=?", [asset_id])?
            .into_iter()
            .next()
    {
        let job = text(&asset, "job_id");
        for path in catalog.json_rows(
            "SELECT json_quote(relative_path) FROM members WHERE job_id=? AND sha256=?",
            (job, sha),
        )? {
            if asset["paths"]
                .as_array()
                .is_some_and(|paths| paths.contains(&path))
                && let Some(copy) =
                    archived_copy(catalog, archive_id, job, path.as_str().unwrap_or_default())?
            {
                return Ok(Some((
                    json!(asset_id),
                    path.as_str().unwrap_or_default().to_owned(),
                    text(&copy, "destination").to_owned(),
                )));
            }
        }
    }

    let wanted = realpath(text(source, "path"));
    for volume in catalog
        .volumes()?
        .into_iter()
        .filter(|volume| volume["role"] == "work")
    {
        let root = realpath(text(&volume, "root"));
        let Ok(rel) = wanted.strip_prefix(&root) else {
            continue;
        };
        let rel = rel
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let rows = catalog.json_rows(
            "SELECT json_object('job',job_id,'copy',json(payload)) FROM copies WHERE volume_id=?",
            [text(&volume, "id")],
        )?;
        for row in rows {
            let copy = &row["copy"];
            let destination = text(copy, "destination");
            let path = text(copy, "path");
            let member = if rel == destination {
                path.to_owned()
            } else if let Some(inner) = rel
                .strip_prefix(destination.trim_end_matches('/'))
                .and_then(|rest| rest.strip_prefix('/'))
            {
                format!("{}/{inner}", path.trim_end_matches('/'))
            } else {
                continue;
            };
            let job = text(&row, "job");
            if let Some(archived) = archived_copy(catalog, archive_id, job, &member)? {
                let assets = catalog.json_rows(
                    "SELECT assets.payload FROM assets JOIN work ON work.asset_id=assets.id WHERE job_id=?", [job],
                )?;
                let owner = assets
                    .into_iter()
                    .find(|asset| {
                        asset["paths"]
                            .as_array()
                            .is_some_and(|paths| paths.iter().any(|p| p == &member))
                    })
                    .map_or(Value::Null, |asset| asset["id"].clone());
                return Ok(Some((
                    owner,
                    member,
                    text(&archived, "destination").to_owned(),
                )));
            }
        }
    }
    Ok(None)
}

fn target_folder(destination: &str) -> Result<String> {
    let parts: Vec<_> = crate::rules::relative(destination)?.split('/').collect();
    let file = Path::new(parts.last().unwrap());
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let digits = |part: &str, len| part.len() == len && part.bytes().all(|c| c.is_ascii_digit());
    for (index, pair) in parts
        .windows(2)
        .enumerate()
        .take(parts.len().saturating_sub(2))
    {
        if digits(pair[0], 4) && digits(pair[1], 2) {
            let mut components = parts[index..parts.len() - 1].to_vec();
            components.push(stem);
            return Ok(components.join("/"));
        }
    }
    Ok(format!("sin-fecha/{stem}"))
}

fn hash_matches(tree: &Root, path: &str, sha: &str) -> Result<bool> {
    match checked_hash(tree, path) {
        Ok(found) => Ok(found.is_some_and(|(found, _)| found == sha)),
        Err(error) if error.message.starts_with("[NotADirectoryError]") => Ok(false),
        Err(error) => Err(error),
    }
}

fn destination(tree: &Root, folder: &str, name: &str, sha: &str) -> Result<(String, &'static str)> {
    let target = format!("{folder}/{name}");
    let Some((existing, _)) = checked_hash(tree, &target)? else {
        return Ok((target, "copy"));
    };
    if existing == sha {
        return Ok((target, "archived"));
    }
    let (base, ext) = name.split_once('.').unwrap_or((name, ""));
    let prefix = format!("{base}.v");
    let (directory, _) = tree.parent(&target, false)?;
    let mut versions = Vec::new();
    for entry in
        std::fs::read_dir(&directory).map_err(|e| nu9ve_volumes::fs::io_error(e, &directory))?
    {
        let entry = entry.map_err(|e| nu9ve_volumes::fs::io_error(e, &directory))?;
        if let Ok(name) = entry.file_name().into_string()
            && name.starts_with(&prefix)
        {
            versions.push(name);
        }
    }
    versions.sort();
    for name in versions {
        let rel = format!("{folder}/{name}");
        if hash_matches(tree, &rel, sha)? {
            return Ok((rel, "archived"));
        }
    }
    let now = nu9ve_time::now_utc();
    Ok((
        format!(
            "{folder}/{prefix}{}.{ext}",
            now[..19].replace(['-', ':'], "")
        ),
        "version",
    ))
}

pub(crate) fn plan(
    catalog: &Catalog,
    backend: &dyn Backend,
    archive_id: &str,
    roots: &[PathBuf],
    export: impl Fn(&Path) -> Result<Value>,
) -> Result<Value> {
    let archive = catalog.volume(archive_id)?;
    require(
        archive["role"] == "archive",
        "invalid_role",
        "Elige un volumen de respaldo",
    )?;
    volumes::check(backend, &archive)?;
    let tree = Root::open(Path::new(text(&archive, "root")))?;
    let mut entries = Vec::new();
    let mut notes = Vec::new();
    let mut exports = Vec::new();
    for root in roots {
        let data = match export(root).and_then(|data| {
            validate_export(&data, root)?;
            Ok(data)
        }) {
            Ok(data) => data,
            Err(error) => {
                notes.push(format!(
                    "{}: Clipper no exportó sus transcripciones; siguen en clipper/ ({})",
                    root.display(),
                    error.message
                ));
                continue;
            }
        };
        exports.push(json!({"project_root": data["project_root"], "digest": data["digest"]}));
        for entry in data["entries"].as_array().into_iter().flatten() {
            let name = file_name(Path::new(text(entry, "path")));
            let Some((asset, member, archived)) =
                archived_member(catalog, text(&archive, "id"), &entry["source"])?
            else {
                notes.push(format!("{name}: su fuente no está respaldada; no se copia"));
                continue;
            };
            let folder = format!("{AREA}/{}", target_folder(&archived)?);
            let (target, action) = destination(&tree, &folder, name, text(entry, "sha256"))?;
            let stamp: Map<_, _> = STAMP_KEYS
                .into_iter()
                .map(|k| (k.into(), entry["stamp"][k].clone()))
                .collect();
            entries.push(json!({"path": entry["path"], "format": entry["format"], "bytes": entry["bytes"],
                "sha256": entry["sha256"], "stamp": stamp, "asset_id": asset, "member": member,
                "member_destination": archived, "target": target, "action": action, "project_root": data["project_root"]}));
        }
    }
    let pending: Vec<_> = entries
        .iter()
        .filter(|e| e["action"] != "archived")
        .collect();
    let bytes: u64 = pending.iter().filter_map(|e| e["bytes"].as_u64()).sum();
    Ok(
        json!({"schema": "nu9ve-transcripts-plan/1", "archive_id": archive_id, "entries": entries,
        "notes": notes, "exports": exports, "files": pending.len(), "bytes": bytes}),
    )
}

pub(crate) fn copy(
    home: &Path,
    backend: &dyn Backend,
    plan: &Value,
    receipt_id: &str,
    progress_id: Option<&str>,
) -> Result<Value> {
    let _lock = writer_lock(home)?;
    let catalog = WritableCatalog::open(home)?;
    let archive = catalog.volume(text(plan, "archive_id"))?;
    require(
        archive["role"] == "archive",
        "invalid_role",
        "Elige un volumen de respaldo",
    )?;
    volumes::check(backend, &archive)?;
    let target = Root::open(Path::new(text(&archive, "root")))?;
    let mut progress = ActionProgress::new(&catalog, progress_id, "copying_transcripts");
    progress.meter.total = plan["bytes"].as_u64().unwrap_or_default().saturating_mul(2);
    progress.meter.files_total = plan["files"].as_u64().unwrap_or_default() as usize;
    let mut transfer = Transfer::new(home, &catalog, backend, &|_, _| Ok(()));
    let mut done = Vec::new();
    let mut archived = Vec::new();
    let mut copied = 0;
    let mut bytes = 0u64;
    let mut processed = 0u64;
    let (prior, pending): (Vec<_>, Vec<_>) = plan["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .partition(|entry| entry["action"] == "archived");
    let receipt_fields = |entry: &Value| -> Map<String, Value> {
        RECEIPT_FIELDS
            .into_iter()
            .map(|key| (key.into(), entry[key].clone()))
            .collect()
    };
    for entry in prior {
        require(
            hash_matches(&target, text(entry, "target"), text(entry, "sha256"))?,
            "checksum_mismatch",
            "Cambió una transcripción archivada; revisa de nuevo",
        )?;
        archived.push(Value::Object(receipt_fields(entry)));
    }
    for entry in pending {
        let mut fields = receipt_fields(entry);
        let rel = text(entry, "target");
        let sha = text(entry, "sha256");
        progress
            .meter
            .holding(rel, "clipper", text(&archive, "name"), "copy");
        progress.report()?;
        let size = entry["bytes"].as_u64().unwrap_or_default();
        if hash_matches(&target, rel, sha)? {
            fields.insert("action".into(), json!("archived"));
        } else {
            let path = Path::new(text(entry, "path"));
            let source = Root::open(
                path.parent()
                    .ok_or_else(|| Error::new("unsafe_path", "Transcript has no parent"))?,
            )?;
            let stamp = Value::Object(
                STAMP_KEYS
                    .into_iter()
                    .filter(|key| *key != "device")
                    .map(|key| (key.into(), entry["stamp"][key].clone()))
                    .collect(),
            );
            require(
                source
                    .stat(file_name(path))?
                    .is_some_and(|found| crate::tree::stamp_json(&found) == stamp),
                "source_changed",
                "Una transcripción cambió desde que Clipper la exportó; revisa de nuevo",
            )?;
            transfer.copy_many(
                &source,
                file_name(path),
                &[Target {
                    root: &target,
                    rel: rel.to_owned(),
                    record: &archive,
                }],
                Some(sha),
                Some(&stamp),
            )?;
            fields.insert("action".into(), json!("copied"));
            copied += 1;
            bytes += size;
        }
        done.push(Value::Object(fields));
        processed += 2 * size;
        progress.meter.reach(processed);
        progress
            .meter
            .finished("copied", rel, text(&archive, "name"));
        progress.report()?;
    }
    let write_receipt = !done.is_empty();
    done.extend(archived);
    let receipt = json!({"schema": "nu9ve-data-receipt/1", "receipt_id": receipt_id, "kind": "clipper_transcripts",
        "volume_id": archive["id"], "created_at": nu9ve_time::now_utc(), "exports": plan["exports"], "files": done});
    volumes::check(backend, &archive)?;
    if write_receipt {
        target.write_json(
            &format!(".ncreator/receipts/data-{receipt_id}.json"),
            &receipt,
        )?;
    }
    catalog.set(&format!("data-receipt:{receipt_id}"), &receipt)?;
    Ok(
        json!({"receipt_id": receipt_id, "copied": copied, "adopted": done.len() - copied,
        "bytes": bytes, "notes": plan["notes"]}),
    )
}

pub fn backup(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let integrations = Integrations::read(&catalog)?;
    backup_with(home, backend, &catalog, params, |root| {
        let executable = integrations.clipper_executable()?;
        WritableCatalog::open(home)?.set("clipper-known", &json!(true))?;
        invoke_clipper(
            &executable,
            &["transcripts-export", &root.to_string_lossy()],
            Duration::from_secs(600),
            None,
        )
    })
}

fn backup_with(
    home: &Path,
    backend: &dyn Backend,
    catalog: &Catalog,
    params: &Value,
    export: impl Fn(&Path) -> Result<Value>,
) -> Result<Value> {
    let param = |key| {
        params[key]
            .as_str()
            .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
    };
    let project = param("project_id")?;
    let archive = param("archive")?;
    let mut projects = vec![project.to_owned()];
    projects.extend(related(catalog, project)?);
    let volumes = catalog.volumes()?;
    let cached = volumes::Cached::new(backend);
    let mut roots = Vec::new();
    for id in projects {
        project_record(catalog, &id)?;
        for volume in volumes.iter().filter(|volume| volume["role"] == "work") {
            if volumes::online(&cached, volume).is_none() {
                continue;
            }
            for root in clipper_roots(volume, &editor_roots(catalog, &id, volume)?)? {
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
    }
    let plan = plan(catalog, &cached, archive, &roots, export)?;
    let progress = params
        .get("progress_id")
        .filter(|id| truthy(id))
        .and_then(Value::as_str);
    let mut result = copy(home, backend, &plan, &uid()?, progress)?;
    result["planned"] = plan["files"].clone();
    result["projects"] = json!(roots.len());
    result["digest"] = json!(digest(&plan));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::test_backend::{Setup, setup};

    const MEMBER: &str = "DCIM/clip.mov";
    const ARCHIVED: &str = "001_raw/video/2026/11/clip.mov";
    const EDITABLE: &str = "001_proyectos/general_raw/intro/clip.mov";
    const DESTINATION: &str = "004_datos/clipper/transcripts/2026/11/clip/clip.srt";

    fn fixture() -> (Setup, PathBuf) {
        let s = setup();
        let editor = s.work.join("001_proyectos/general/intro/clipper");
        std::fs::create_dir_all(editor.join("transcripts")).unwrap();
        std::fs::write(editor.join("clipper-retired-project.json"), "{}").unwrap();
        let transcript = editor.join("transcripts/clip.srt");
        std::fs::write(&transcript, b"original transcript").unwrap();
        let writer = WritableCatalog::open(&s.home).unwrap();
        let db = writer.db();
        db.execute("INSERT INTO projects VALUES('p1','general','intro')", [])
            .unwrap();
        db.execute(
            "INSERT INTO jobs VALUES('job','digest','{}','completed',NULL,'t','t')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO assets VALUES('asset','job',?)",
            [json!({"id": "asset", "job_id": "job", "paths": [MEMBER]}).to_string()],
        )
        .unwrap();
        db.execute("INSERT INTO work VALUES('asset','ready',NULL,'t')", [])
            .unwrap();
        db.execute(
            "INSERT INTO members(job_id,relative_path,sha256) VALUES('job',?,'member-sha')",
            [MEMBER],
        )
        .unwrap();
        for (id, dest) in [("id-MEDIA", ARCHIVED), ("id-ESSD", EDITABLE)] {
            db.execute(
                "INSERT INTO copies VALUES('job',?,?,'member-sha',?)",
                (
                    id,
                    MEMBER,
                    json!({"path": MEMBER, "destination": dest}).to_string(),
                ),
            )
            .unwrap();
        }
        (s, transcript)
    }

    fn export(path: &Path, root: &Path, source: Value) -> Value {
        let tree = Root::open(path.parent().unwrap()).unwrap();
        let (sha, mut stamp) = checked_hash(&tree, file_name(path)).unwrap().unwrap();
        stamp["device"] = json!(1);
        json!({"schema": "clipper-transcript-export/1", "project_root": root,
            "digest": "e".repeat(64), "entries": [{"path": path, "format": "srt", "bytes": stamp["size"],
                "stamp": stamp, "sha256": sha, "source": source}]})
    }

    #[test]
    fn backup_resolves_project_and_archive_names_and_reuses_verified_versions() {
        let (s, path) = fixture();
        let catalog = Catalog::open(&s.home).unwrap();
        let params = json!({"archive": "MEDIA", "project_id": "p1", "progress_id": "fixture"});
        let run = || {
            backup_with(&s.home, &s.backend, &catalog, &params, |root| {
                assert_eq!(root, path.parent().unwrap().parent().unwrap());
                Ok(export(&path, root, json!({"path": s.work.join(EDITABLE)})))
            })
            .unwrap()
        };
        let result = run();
        assert_eq!(result["projects"], 1);
        assert_eq!(result["planned"], 1);
        assert_eq!(result["copied"], 1);
        let id = text(&result, "receipt_id");
        let receipt = catalog.get(&format!("data-receipt:{id}")).unwrap().unwrap();
        assert_eq!(receipt["files"][0]["asset_id"], "asset");
        assert_eq!(receipt["files"][0]["member_destination"], ARCHIVED);
        assert_eq!(receipt["files"][0]["target"], DESTINATION);
        let portable: Value = serde_json::from_slice(
            &std::fs::read(s.archive.join(format!(".ncreator/receipts/data-{id}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(portable, receipt);
        assert!(catalog.get("action-progress:fixture").unwrap().is_none());

        std::fs::write(&path, b"another version").unwrap();
        assert_eq!(run()["copied"], 1);
        let repeat = run();
        assert_eq!(repeat["planned"], 0);
        assert_eq!(repeat["adopted"], 1);
        assert_eq!(
            std::fs::read(s.archive.join(DESTINATION)).unwrap(),
            b"original transcript"
        );
        assert_eq!(std::fs::read(path).unwrap(), b"another version");
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python)"]
    fn backup_and_receipts_match_python() {
        let (s, path) = fixture();
        let catalog = Catalog::open(&s.home).unwrap();
        let golden_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_slice(&std::fs::read(golden_path).unwrap()).unwrap();
        let cases = golden["transcripts"].as_array().unwrap();
        assert_eq!(cases.len(), 3);
        let timestamp = regex::Regex::new(r"\.v[0-9]{8}T[0-9]{6}\.").unwrap();
        for (body, expected) in ["original transcript", "another version", "another version"]
            .into_iter()
            .zip(cases)
        {
            std::fs::write(&path, body).unwrap();
            let mut result = backup_with(
                &s.home,
                &s.backend,
                &catalog,
                &json!({"archive": "MEDIA", "project_id": "p1"}),
                |root| Ok(export(&path, root, json!({"path": s.work.join(EDITABLE)}))),
            )
            .unwrap();
            let id = text(&result, "receipt_id").to_owned();
            let mut receipt = catalog.get(&format!("data-receipt:{id}")).unwrap().unwrap();
            receipt["created_at"] = json!("2026-11-12T02:03:04Z");
            result.as_object_mut().unwrap().remove("digest");
            let root = json!(s.archive.parent().unwrap()).to_string();
            let normalize = |v: Value| -> Value {
                let v = v
                    .to_string()
                    .replace(&root[1..root.len() - 1], "<ROOT>")
                    .replace("id-MEDIA", "MEDIA")
                    .replace(&id, "<ID>");
                serde_json::from_str(&timestamp.replace_all(&v, ".v20261112T020304.")).unwrap()
            };
            assert_eq!(normalize(result), expected["result"]);
            assert_eq!(normalize(receipt), expected["receipt"]);
        }
    }

    #[test]
    fn source_change_blocks_copy_and_a_retry_adopts_verified_content() {
        let (s, path) = fixture();
        let catalog = Catalog::open(&s.home).unwrap();
        let root = path.parent().unwrap().parent().unwrap();
        let export = |root: &Path| {
            Ok(export(
                &path,
                root,
                json!({
                    "asset_id": "asset", "member_sha256": "member-sha", "path": "/offline/clip.mov",
                }),
            ))
        };
        let review = || plan(&catalog, &s.backend, "id-MEDIA", &[root.to_owned()], export).unwrap();
        let planned = review();
        std::fs::write(&path, b"changed transcript!").unwrap();
        let apply = |plan: &Value| copy(&s.home, &s.backend, plan, &uid().unwrap(), None);
        assert_eq!(apply(&planned).unwrap_err().code, "source_changed");
        assert!(!s.archive.join(DESTINATION).exists());
        let revised = review();
        assert_eq!(apply(&revised).unwrap()["copied"], 1);
        assert_eq!(apply(&revised).unwrap()["adopted"], 1);
        let archived = review();
        std::fs::write(s.archive.join(DESTINATION), b"corrupted").unwrap();
        assert_eq!(apply(&archived).unwrap_err().code, "checksum_mismatch");
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn project_backup_invokes_the_clipper_export_contract() {
        use std::os::unix::fs::PermissionsExt;
        let (s, path) = fixture();
        let root = path.parent().unwrap().parent().unwrap();
        let data = export(&path, root, json!({"path": s.work.join(EDITABLE)}));
        let binary = s.home.join("clipper");
        std::fs::write(&binary, format!(r#"#!/bin/sh
[ "$1" = library ] && [ "$2" = transcripts-export ] && [ "$3" = '{}' ] && [ "$4" = --json ] || exit 8
cat <<'JSON'
{}
JSON
"#, root.display(), data)).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        WritableCatalog::open(&s.home)
            .unwrap()
            .set("clipper-executable", &json!(binary))
            .unwrap();
        let result = backup(
            &s.home,
            &s.backend,
            &json!({"archive": "MEDIA", "project_id": "p1"}),
        )
        .unwrap();
        assert_eq!(result["copied"], 1, "{result}");
        assert_eq!(
            Catalog::open(&s.home)
                .unwrap()
                .get("clipper-known")
                .unwrap(),
            Some(json!(true))
        );
    }

    #[test]
    fn unavailable_clipper_and_sources_without_receipts_produce_notes() {
        let (s, path) = fixture();
        let catalog = Catalog::open(&s.home).unwrap();
        let root = path.parent().unwrap().parent().unwrap().to_owned();
        let wrong_role = plan(
            &catalog,
            &s.backend,
            "ESSD",
            std::slice::from_ref(&root),
            |_| unreachable!("role must be validated before invoking Clipper"),
        )
        .unwrap_err();
        assert_eq!(wrong_role.code, "invalid_role");
        let orphan = plan(
            &catalog,
            &s.backend,
            "MEDIA",
            std::slice::from_ref(&root),
            |root| Ok(export(&path, root, json!({"path": "/offline/missing.mov"}))),
        )
        .unwrap();
        assert_eq!(orphan["files"], 0);
        assert_eq!(
            orphan["notes"],
            json!(["clip.srt: su fuente no está respaldada; no se copia"])
        );
        let offline = plan(&catalog, &s.backend, "MEDIA", &[root], |_| {
            Err(Error::new("clipper_unavailable", "fixture offline"))
        })
        .unwrap();
        assert_eq!(offline["entries"], json!([]));
        assert!(
            offline["notes"][0]
                .as_str()
                .unwrap()
                .contains("fixture offline")
        );
    }
}
