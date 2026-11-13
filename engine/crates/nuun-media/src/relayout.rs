//! Move verified Stalker bundles to their dated names and update portable receipts.

use crate::catalog::{Catalog, WritableCatalog, sql, writer_lock};
use crate::clipper_release::Guard;
use crate::release::checked_hash;
use crate::retitle::{manifest_at, text, uuid};
use crate::rules::relative;
use crate::tree::Root;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, Result, canonical, require};
use nu9ve_volumes::{EntryKind, fs};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn relative_bundle(member: &Value, prefix: &str) -> Result<String> {
    let inner = text(member, "path")
        .strip_prefix(prefix)
        .ok_or_else(|| Error::new("revision_mismatch", "Member outside its bundle"))?;
    let suffix = format!("/{inner}");
    let dir = text(member, "destination")
        .strip_suffix(&suffix)
        .ok_or_else(|| Error::new("revision_mismatch", "Bundle paths were not preserved"))?;
    Ok(relative(dir)?.to_owned())
}

fn existing_folder(tree: &Root, rel: &str) -> Result<bool> {
    let (parent, name) = match tree.parent(rel, false) {
        Ok(parts) => parts,
        Err(error) if fs::python_exception(&error) == Some("FileNotFoundError") => {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    match fs::stamp_opt(&parent.join(name))? {
        None => Ok(false),
        Some(stamp) => {
            require(
                stamp.kind == EntryKind::Dir,
                "destination_conflict",
                format!("Target bundle name already exists: {rel}"),
            )?;
            Ok(true)
        }
    }
}

fn prepare(catalog: &Catalog, backend: &dyn Backend, job: &Value) -> Result<Vec<Value>> {
    let plan = &job["plan"];
    let units: Vec<_> = plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|unit| unit["kind"] == "stalker_session")
        .collect();
    require(
        !units.is_empty(),
        "nothing_to_relayout",
        "This job has no Stalker sessions",
    )?;
    let mut groups = BTreeMap::<String, Vec<Value>>::new();
    for row in catalog.json_rows(
        "SELECT json_object('volume',volume_id,'copy',json(payload)) FROM copies WHERE job_id=?",
        [text(job, "id")],
    )? {
        groups
            .entry(text(&row, "volume").to_owned())
            .or_default()
            .push(row["copy"].clone());
    }
    let archive = catalog.volume(text(plan, "archive_id"))?;
    volumes::check(backend, &archive)?;
    let archive_tree = Root::open(Path::new(text(&archive, "root")))?;
    let mut moves = Vec::new();
    let mut targets = BTreeSet::new();
    for unit in units {
        let prefix = if unit["bundle"] == "." {
            String::new()
        } else {
            format!("{}/", relative(text(unit, "bundle"))?)
        };
        let manifest_member = format!("{prefix}manifest.json");
        let copy = groups
            .get(text(&archive, "id"))
            .into_iter()
            .flatten()
            .find(|copy| copy["path"] == manifest_member)
            .ok_or_else(|| Error::new("missing_receipt", "Archived manifest receipt missing"))?;
        let bundle = relative_bundle(copy, &prefix)?;
        let (manifest, sha) = manifest_at(&archive_tree.path.join(bundle))?;
        require(
            sha == text(copy, "sha256")
                && uuid(&manifest["sessionID"])? == uuid(&unit["session_id"])?,
            "invalid_bundle",
            "Archived manifest is not this verified session",
        )?;
        let fallback = plan["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|file| file["unit_id"] == unit["id"])
            .filter_map(|file| file["stamp"]["mtime_ns"].as_i64())
            .min()
            .unwrap_or_default();
        let (name, created) = crate::layout::stalker_bundle_name(
            &manifest,
            fallback,
            text(plan, "country"),
            text(unit, "project"),
        )?;
        for (id, copies) in &groups {
            let members: Vec<_> = unit["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|path| copies.iter().find(|copy| copy["path"] == *path))
                .collect();
            let Some(sample) = members.first() else {
                continue;
            };
            require(
                unit["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.len() == members.len()),
                "missing_receipt",
                "Complete revision receipts required",
            )?;
            let volume = catalog.volume(id)?;
            volumes::check(backend, &volume)?;
            let old = relative_bundle(sample, &prefix)?;
            let parent = old
                .rsplit_once('/')
                .ok_or_else(|| Error::new("invalid_plan", "Invalid bundle destination"))?
                .0;
            let parent = if volume["role"] == "archive" {
                let base = parent.rsplitn(3, '/').nth(2).unwrap_or(parent);
                format!("{base}/{}", created.strftime("%Y/%m"))
            } else {
                parent.to_owned()
            };
            let new = format!("{parent}/{name}");
            require(
                targets.insert((id.to_owned(), new.clone())),
                "destination_conflict",
                format!("Target bundle name already exists: {new}"),
            )?;
            for member in &members {
                require(
                    relative_bundle(member, &prefix)? == old,
                    "revision_mismatch",
                    "Bundle members have different locations",
                )?;
            }
            moves.push(json!({"session_id":unit["session_id"],"volume_id":id,"from":old,"to":new,"members":members,
                "root":volume["root"], "identity":volume["identity"], "prefix":volume["prefix"]}));
        }
    }
    Ok(moves)
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
    hook: crate::transfer::Hook<'_>,
) -> Result<Value> {
    let id = params["job_id"]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", "'job_id'"))?;
    let catalog = Catalog::open(home)?;
    let guard = acquire()?;
    let _lock = writer_lock(home)?;
    let writer = WritableCatalog::open(home)?;
    let job = crate::reads::job(&catalog, params)?;
    require(
        matches!(
            job["state"].as_str(),
            Some(
                "completed"
                    | "camera_action_required"
                    | "source_retained"
                    | "source_has_new_material"
            )
        ),
        "job_not_finished",
        "Only finished jobs can be renamed in place",
    )?;
    let key = format!("relayout-intent:{id}");
    let moves: Vec<Value> = if let Some(prior) = writer.get(&key)? {
        serde_json::from_value(prior).map_err(|e| Error::new("operation_failed", e.to_string()))?
    } else {
        let moves = prepare(&catalog, backend, &job)?;
        for movement in &moves {
            let volume = catalog.volume(text(movement, "volume_id"))?;
            let tree = Root::open(Path::new(text(&volume, "root")))?;
            let from = text(movement, "from");
            let to = text(movement, "to");
            require(
                from == to || !existing_folder(&tree, to)?,
                "destination_conflict",
                format!("Target bundle name already exists: {to}"),
            )?;
        }
        moves
    };
    let mut paths = Vec::new();
    for movement in &moves {
        let from = text(movement, "from");
        let to = text(movement, "to");
        if from == to {
            continue;
        }
        let volume = catalog.volume(text(movement, "volume_id"))?;
        require(
            volume["root"] == movement["root"]
                && volume["identity"] == movement["identity"]
                && volume["prefix"] == movement["prefix"],
            "destination_changed",
            "Registered volume changed during relayout",
        )?;
        let root = Path::new(text(&volume, "root"));
        if existing_folder(&Root::open(root)?, from)? {
            paths.push((root.join(from).to_string_lossy().into_owned(), None));
        }
    }
    if !paths.is_empty() {
        let inspection = guard.inspect(&paths)?;
        let blockers: Vec<_> = inspection["blockers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        require(
            blockers.is_empty(),
            "dependencies_blocked",
            blockers.join("; "),
        )?;
    }
    writer.set(&key, &json!(moves))?;
    let mut renamed = Vec::new();
    let mut copies = BTreeMap::<String, BTreeMap<String, Value>>::new();
    let mut overrides = writer
        .get(&format!("layout_override:{id}"))?
        .unwrap_or(json!({}));
    for movement in moves {
        let volume_id = text(&movement, "volume_id");
        let volume = catalog.volume(volume_id)?;
        volumes::check(backend, &volume)?;
        let tree = Root::open(Path::new(text(&volume, "root")))?;
        let from = text(&movement, "from");
        let to = text(&movement, "to");
        let found = existing_folder(&tree, from)?;
        require(
            found || existing_folder(&tree, to)?,
            "missing_copy",
            format!("Neither old nor new bundle folder exists: {from}"),
        )?;
        let current = if found { from } else { to };
        let mut checked = Vec::new();
        for member in movement["members"].as_array().into_iter().flatten() {
            let suffix = text(member, "destination")
                .strip_prefix(from)
                .ok_or_else(|| Error::new("revision_mismatch", "Member outside its bundle"))?;
            let current_path = format!("{current}{suffix}");
            let (sha, stamp) = checked_hash(&tree, &current_path)?.ok_or_else(|| {
                Error::new("missing_copy", format!("Missing member: {current_path}"))
            })?;
            require(
                member["sha256"] == sha && member["bytes"] == stamp["size"],
                "checksum_mismatch",
                format!("Renamed member differs: {current_path}"),
            )?;
            checked.push((member.clone(), suffix));
        }
        if found && from != to {
            let (src, src_name) = tree.parent(from, false)?;
            let (dst, dst_name) = tree.parent(to, true)?;
            volumes::check(backend, &volume)?;
            fs::rename_new(&src.join(src_name), &dst.join(dst_name))?;
            fs::sync_dir(&src)?;
            if src != dst {
                fs::sync_dir(&dst)?;
            }
            hook("renamed", &movement)?;
        }
        for (mut member, suffix) in checked {
            let destination = format!("{to}{suffix}");
            let stamp = tree.stat(&destination)?.ok_or_else(|| {
                Error::new("missing_copy", format!("Missing member: {destination}"))
            })?;
            require(
                member["bytes"] == stamp.size,
                "checksum_mismatch",
                format!("Renamed member differs: {destination}"),
            )?;
            let new_path = destination
                .strip_prefix(text(&volume, "prefix"))
                .and_then(|path| path.strip_prefix('/'))
                .ok_or_else(|| Error::new("unsafe_path", "Bundle outside volume prefix"))?;
            let path = text(&member, "path").to_owned();
            overrides[&path][text(&volume, "role")] = json!(new_path);
            member["destination"] = json!(destination);
            member["stamp"] = crate::tree::stamp_json(&stamp);
            copies
                .entry(volume_id.to_owned())
                .or_default()
                .insert(path, member);
        }
        renamed.push(
            json!({"session_id":movement["session_id"],"volume_id":volume_id,"from":from,"to":to}),
        );
    }
    let receipt_rel = format!(".ncreator/receipts/{id}.json");
    for (volume_id, copies) in &copies {
        let volume = catalog.volume(volume_id)?;
        volumes::check(backend, &volume)?;
        let root = Path::new(text(&volume, "root"));
        if let Some(raw) = crate::tree::read_bytes(root, &receipt_rel, 16 * 1024 * 1024)? {
            let mut receipt: Value = serde_json::from_slice(&raw)
                .map_err(|e| Error::new("operation_failed", e.to_string()))?;
            if let Some(files) = receipt["files"].as_array_mut() {
                for file in files {
                    if let Some(copy) = copies.get(text(file, "path")) {
                        file["destination"] = copy["destination"].clone();
                        file["stamp"] = copy["stamp"].clone();
                    }
                }
            }
            receipt["relayout"] = json!({"at":nu9ve_time::now_utc(),"renamed":renamed.iter().filter(|r| r["volume_id"] == *volume_id).collect::<Vec<_>>()});
            Root::open(root)?.write_json(&receipt_rel, &receipt)?;
        }
    }
    let tx = writer.db().unchecked_transaction().map_err(sql)?;
    for (volume, rows) in copies {
        for (path, copy) in rows {
            let payload = String::from_utf8(canonical(&copy)).expect("canonical JSON is UTF-8");
            tx.execute(
                "UPDATE copies SET payload=? WHERE job_id=? AND volume_id=? AND relative_path=?",
                (payload, id, &volume, path),
            )
            .map_err(sql)?;
        }
    }
    writer.set(&format!("layout_override:{id}"), &overrides)?;
    writer.event(id, "relayout", &json!({"renamed":renamed}))?;
    tx.execute("DELETE FROM metadata WHERE key=?", [&key])
        .map_err(sql)?;
    tx.commit().map_err(sql)?;
    Ok(json!({"job_id":id,"renamed":renamed}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relayout_resumes_after_rename_and_updates_portable_receipts() {
        let (s, asset, _) = crate::revisions::tests::fixture();
        let id = text(&asset, "job_id");
        let catalog = Catalog::open(&s.home).unwrap();
        let writer = WritableCatalog::open(&s.home).unwrap();
        let mut older = Vec::new();
        for volume in catalog.volumes().unwrap() {
            let volume_id = text(&volume, "id");
            let rows = catalog
                .json_rows(
                    "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
                    (id, volume_id),
                )
                .unwrap();
            let root = Root::open(Path::new(text(&volume, "root"))).unwrap();
            let prefix = format!("{}/", text(&asset, "bundle"));
            let from = relative_bundle(&rows[0], &prefix).unwrap();
            let old = format!("{}/old.stalker", from.rsplit_once('/').unwrap().0);
            fs::rename_new(&root.path.join(&from), &root.path.join(&old)).unwrap();
            older.push(root.path.join(&old));
            let receipt_rel = format!(".ncreator/receipts/{id}.json");
            let mut receipt: Value =
                serde_json::from_slice(&std::fs::read(root.path.join(&receipt_rel)).unwrap())
                    .unwrap();
            for mut copy in rows {
                let path = text(&copy, "path").to_owned();
                copy["destination"] = json!(text(&copy, "destination").replacen(&from, &old, 1));
                writer.db().execute("UPDATE copies SET payload=? WHERE job_id=? AND volume_id=? AND relative_path=?",
                    (copy.to_string(),id,volume_id,&path)).unwrap();
                if let Some(file) = receipt["files"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|file| file["path"] == path)
                {
                    file["destination"] = copy["destination"].clone();
                }
            }
            root.write_json(&receipt_rel, &receipt).unwrap();
        }
        let params = json!({"job_id":id});
        let run = |hook: crate::transfer::Hook<'_>| {
            apply_with(
                &s.home,
                &s.backend,
                &params,
                || Guard::acquire_at(&s.home, s.home.join("clipper-home")),
                hook,
            )
        };
        let error =
            run(&|stage, _| require(stage != "renamed", "fixture_interrupted", "Interrupted"))
                .unwrap_err();
        assert_eq!(error.code, "fixture_interrupted");
        assert!(
            writer
                .get(&format!("relayout-intent:{id}"))
                .unwrap()
                .is_some()
        );
        let result = run(&|_, _| Ok(())).unwrap();
        assert_eq!(result["renamed"].as_array().unwrap().len(), 2);
        assert!(older.iter().all(|path| !path.exists()));
        for entry in result["renamed"].as_array().unwrap() {
            let volume = catalog.volume(text(entry, "volume_id")).unwrap();
            let root = Path::new(text(&volume, "root"));
            assert_eq!(
                std::fs::read(root.join(text(entry, "to")).join("screen.mov")).unwrap(),
                b"fixture video"
            );
            let receipt: Value = serde_json::from_slice(
                &std::fs::read(root.join(format!(".ncreator/receipts/{id}.json"))).unwrap(),
            )
            .unwrap();
            assert!(
                receipt["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|file| text(file, "destination").starts_with(text(entry, "to")))
            );
        }
        assert!(
            writer
                .get(&format!("relayout-intent:{id}"))
                .unwrap()
                .is_none()
        );
        let repeat = run(&|_, _| Ok(())).unwrap();
        assert!(
            repeat["renamed"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["from"] == entry["to"])
        );
    }
}
