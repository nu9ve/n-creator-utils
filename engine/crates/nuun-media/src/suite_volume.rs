//! F0 volume markers, local sightings and portable, read-only adoption.
use crate::catalog::Catalog;
use crate::suite_device::{self, OS, name, os, time, uuid};
use crate::tree::Root;
use crate::volumes::{Backend, normalize};
use nu9ve_json::{Error, Result, require};
use nu9ve_volumes::fs;
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};

mod index;

const MARKER: &str = ".nu9ve/volume.json";
const INDEX: &str = ".nu9ve/apps/nuun/index.json";

fn document(root: &Root, path: &str, limit: usize, code: &'static str) -> Result<Option<Value>> {
    let Some(file) = root
        .open_read(path)
        .map_err(|e| Error::new(code, e.message))?
    else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::new(code, e.to_string()))?;
    require(
        bytes.len() <= limit,
        code,
        format!("{path} exceeds its size limit"),
    )?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| Error::new(code, format!("Invalid JSON in {path}")))?;
    Ok(Some(value))
}

fn role(value: &Value) -> bool {
    matches!(value.as_str(), Some("archive" | "work" | "other"))
}

fn marker(root: &Root) -> Result<Option<Value>> {
    let Some(value) = document(root, MARKER, 16 * 1024, "invalid_marker")? else {
        return Ok(None);
    };
    require(
        value["contract"] == "nu9ve-volume/1"
            && uuid(&value["volume_id"])
            && role(&value["role"])
            && name(&value["label"], 64)
            && time(&value["registered_at"])
            && uuid(&value["registered_by"]["device_id"])
            && value["registered_by"]["app"].as_str().is_some_and(app_name)
            && os(&value["fs"]["os"])
            && matches!(
                value["fs"]["kind"].as_str(),
                Some("apfs" | "hfsplus" | "exfat" | "fat32" | "ntfs" | "ext4" | "other")
            )
            && name(&value["fs"]["serial"], 128)
            && value.get("role_changed_at").is_none_or(time)
            && value.get("previous_volume_id").is_none_or(uuid),
        "invalid_marker",
        "Invalid .nu9ve/volume.json",
    )?;
    Ok(Some(value))
}

fn app_name(s: &str) -> bool {
    s.len() <= 32
        && s.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn factor(info: &Value) -> Value {
    let kind = match info["filesystem"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "apfs" => "apfs",
        "hfs" | "hfs+" | "hfsplus" => "hfsplus",
        "exfat" => "exfat",
        "msdos" | "fat32" => "fat32",
        "ntfs" => "ntfs",
        "ext4" => "ext4",
        _ => "other",
    };
    let serial = if OS == "windows" {
        info["device"]
            .as_u64()
            .and_then(|num| u32::try_from(num).ok())
            .map(|num| format!("{:04X}-{:04X}", num >> 16, num & 0xffff))
    } else {
        info["volume_uuid"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_uppercase)
    };
    json!({"os": OS, "kind": kind, "serial": serial})
}

fn mounted(backend: &dyn Backend, path: &Path) -> Result<(Root, Value)> {
    let info = backend.describe_suite(path)?;
    require(
        info["mount"] == normalize(path) && info["disk_image"] != true,
        "not_volume_root",
        "Choose the root of a mounted external volume",
    )?;
    Ok((Root::open(path)?, info))
}

fn duplicate(backend: &dyn Backend, root: &Root, volume: &Value) -> bool {
    backend.mounted_roots().iter().any(|path| {
        normalize(path) != normalize(&root.path)
            && Root::open(path)
                .and_then(|r| marker(&r))
                .ok()
                .flatten()
                .is_some_and(|other| other["volume_id"] == volume["volume_id"])
    })
}

fn sighting_path(volume: &Value) -> String {
    format!(
        "suite-volumes/{}-{OS}.json",
        volume["volume_id"].as_str().unwrap()
    )
}

fn sighting(home: &Path, volume: &Value) -> Result<Option<Value>> {
    if !home.exists() {
        return Ok(None);
    }
    document(
        &Root::open(home)?,
        &sighting_path(volume),
        4096,
        "invalid_sighting",
    )
}

fn observed(
    backend: &dyn Backend,
    root: &Root,
    info: &Value,
    volume: &Value,
    known: Option<&Value>,
) -> &'static str {
    if duplicate(backend, root, volume) {
        return "duplicate";
    }
    let current = factor(info);
    if current["serial"].is_null() {
        return "unverified";
    }
    let expected = known
        .and_then(|s| s.get("serial"))
        .filter(|s| !s.is_null())
        .or_else(|| (volume["fs"]["os"] == OS).then_some(&volume["fs"]["serial"]));
    if expected.is_some_and(|serial| *serial != current["serial"]) {
        "suspect"
    } else {
        "ok"
    }
}

fn record(home: &Path, volume: &Value, info: &Value, known: Option<&Value>) -> Result<()> {
    let now = nu9ve_time::now_utc();
    let fs = factor(info);
    let serial = if fs["serial"].is_null() {
        known.map_or(Value::Null, |saved| saved["serial"].clone())
    } else {
        fs["serial"].clone()
    };
    let first = known.and_then(|s| s["first_seen"].as_str()).unwrap_or(&now);
    Root::open(home)?.write_json(
        &sighting_path(volume),
        &json!({"serial":serial, "first_seen":first, "last_seen":now}),
    )
}

fn allowed(state: &str) -> Result<()> {
    match state {
        "ok" | "unverified" => Ok(()),
        "suspect" => Err(Error::new(
            "suspect",
            "The filesystem serial disagrees with the known volume",
        )),
        "duplicate" => Err(Error::new(
            "duplicate",
            "Two mounted volumes have the same marker",
        )),
        _ => Err(Error::new(
            "unmarked",
            "Register this volume with the suite first",
        )),
    }
}

fn selected(params: &Value) -> Result<PathBuf> {
    let root = PathBuf::from(params["root"].as_str().unwrap_or_default());
    require(
        root.is_absolute(),
        "invalid_params",
        "Use an absolute volume root",
    )?;
    Ok(root)
}

/// Normal operations honor the marker on the mount even when their source or destination is a subfolder.
pub(crate) fn check_mount(home: &Path, backend: &dyn Backend, info: &Value) -> Result<()> {
    let Some(path) = info["mount"].as_str().filter(|_| info["external"] == true) else {
        return Ok(());
    };
    let root = Root::open(Path::new(path))?;
    if let Some(volume) = marker(&root)? {
        let known = sighting(home, &volume)?;
        allowed(observed(backend, &root, info, &volume, known.as_ref()))?;
    }
    Ok(())
}

pub fn register(
    home: &Path,
    backend: &dyn Backend,
    isolated: bool,
    params: &Value,
) -> Result<Value> {
    let path = selected(params)?;
    let (root, info) = mounted(backend, &path)?;
    let label = if name(&info["label"], 64) {
        &info["label"]
    } else {
        &params["label"]
    };
    require(
        role(&params["role"]) && name(label, 64),
        "invalid_params",
        "Use a valid volume role and label (1–64 characters)",
    )?;
    let _writer = crate::catalog::writer_lock(home)?;
    let device = suite_device::current(isolated, &json!({}))?;
    require(
        device["state"] == "ok",
        "device_unavailable",
        device["message"]
            .as_str()
            .unwrap_or("A local device identity is required"),
    )?;
    let mut created = false;
    if marker(&root)?.is_none() {
        let fs = factor(&info);
        require(
            !fs["serial"].is_null(),
            "unverified",
            "The filesystem has no serial to register",
        )?;
        let value = json!({"contract":"nu9ve-volume/1","volume_id":nu9ve_json::uid()?,
            "role":params["role"], "label":label, "registered_at":nu9ve_time::now_utc(),
            "registered_by":{"device_id":device["device"]["device_id"],"app":"nuun"}, "fs":fs});
        backend.fast_check(&path, &info)?;
        let (dir, _) = root.parent(MARKER, true)?;
        // The directory itself is the advisory lock for cooperating publishers on macOS exFAT.
        #[cfg(target_os = "macos")]
        let _publication = {
            let fd = std::fs::File::open(&dir).map_err(|e| fs::io_error(e, &dir))?;
            fd.try_lock()
                .map_err(|e| Error::new("volume_busy", e.to_string()))?;
            fd
        };
        #[cfg(not(target_os = "macos"))]
        let _ = dir;
        if marker(&root)?.is_none() {
            let result = root.write_bytes(MARKER, &nu9ve_json::canonical(&value), false);
            if let Some(winner) = marker(&root)? {
                created = winner == value;
                if created {
                    result?;
                }
            } else {
                result?;
                return Err(Error::new(
                    "volume_replaced",
                    "Could not read back the marker",
                ));
            }
        }
    }
    let volume =
        marker(&root)?.ok_or_else(|| Error::new("volume_replaced", "Marker disappeared"))?;
    let known = sighting(home, &volume)?;
    let state = observed(backend, &root, &info, &volume, known.as_ref());
    allowed(state)?;
    require(
        volume["role"] == params["role"],
        "role_mismatch",
        "The existing marker has a different role",
    )?;
    record(home, &volume, &info, known.as_ref())?;
    let mut result = json!({"state":state, "marker":volume, "created":created});
    if let Some(suggestion) = device.get("suggested_name") {
        result["suggested_device_name"] = suggestion.clone();
    }
    Ok(result)
}

/// A reviewed serial change or copy is the only identity change allowed in F0.
pub fn resolve(
    home: &Path,
    backend: &dyn Backend,
    isolated: bool,
    params: &Value,
) -> Result<Value> {
    let path = selected(params)?;
    let (root, info) = mounted(backend, &path)?;
    let _writer = crate::catalog::writer_lock(home)?;
    let volume =
        marker(&root)?.ok_or_else(|| Error::new("unmarked", "This disk has no suite marker"))?;
    require(
        params["volume_id"] == volume["volume_id"],
        "volume_replaced",
        "Review the current marker before resolving",
    )?;
    let known = sighting(home, &volume)?;
    let state = observed(backend, &root, &info, &volume, known.as_ref());
    require(
        matches!(state, "suspect" | "duplicate"),
        "invalid_volume_state",
        "Only a suspect or duplicate volume requires review",
    )?;
    let fs = factor(&info);
    require(
        !fs["serial"].is_null() && params["serial"] == fs["serial"],
        "volume_replaced",
        "Review the current filesystem serial before resolving",
    )?;
    backend.fast_check(&path, &info)?;
    let result = match params["decision"].as_str() {
        Some("same") if state == "suspect" => {
            record(home, &volume, &info, known.as_ref())?;
            volume
        }
        Some("copy") => {
            let device = suite_device::current(isolated, &json!({}))?;
            require(
                device["state"] == "ok",
                "device_unavailable",
                "A local device identity is required",
            )?;
            require(
                marker(&root)?.as_ref() == Some(&volume),
                "volume_replaced",
                "Marker changed before publishing",
            )?;
            let replacement = json!({"contract":"nu9ve-volume/1","volume_id":nu9ve_json::uid()?,
                "previous_volume_id":volume["volume_id"], "role":volume["role"], "label":volume["label"],
                "registered_at":nu9ve_time::now_utc(),
                "registered_by":{"device_id":device["device"]["device_id"],"app":"nuun"}, "fs":fs});
            root.write_json(MARKER, &replacement)?;
            require(
                marker(&root)?.as_ref() == Some(&replacement),
                "volume_replaced",
                "Could not read back the marker",
            )?;
            record(home, &replacement, &info, None)?;
            replacement
        }
        _ => {
            return Err(Error::new(
                "invalid_params",
                "Confirm the serial change or register this volume as a copy",
            ));
        }
    };
    Ok(json!({"state":"ok", "marker":result}))
}

pub fn observe(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let path = selected(params)?;
    let Ok((root, info)) = mounted(backend, &path) else {
        return Ok(json!({"state":"unreadable"}));
    };
    let volume = match marker(&root) {
        Ok(Some(volume)) => volume,
        Ok(None) => return Ok(json!({"state":"unmarked"})),
        Err(error) => return Ok(json!({"state":"invalid","message":error.message})),
    };
    let _writer = crate::catalog::writer_lock(home)?;
    let known = sighting(home, &volume)?;
    let state = observed(backend, &root, &info, &volume, known.as_ref());
    if allowed(state).is_ok() {
        record(home, &volume, &info, known.as_ref())?;
    }
    Ok(json!({"state":state,"marker":volume, "current_fs":factor(&info)}))
}

pub fn publish(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    require(
        params["include_personal"] == true,
        "confirmation_required",
        "Confirm publishing the metadata of the copies on this disk",
    )?;
    let path = selected(params)?;
    let (root, info) = mounted(backend, &path)?;
    let _writer = crate::catalog::writer_lock(home)?;
    let catalog = Catalog::open(home)?;
    let registration = catalog
        .volumes()?
        .into_iter()
        .find(|entry| entry["root"] == normalize(&path))
        .ok_or_else(|| {
            Error::new(
                "unknown_volume",
                "This disk has no local nüün destination to index",
            )
        })?;
    crate::volumes::check(backend, &registration)?;
    let volume = marker(&root)?
        .ok_or_else(|| Error::new("unmarked", "Register this volume with the suite first"))?;
    let known = sighting(home, &volume)?;
    let state = observed(backend, &root, &info, &volume, known.as_ref());
    allowed(state)?;
    require(
        state == "ok",
        "unverified",
        "Index publication requires a verified filesystem serial",
    )?;
    require(
        volume["role"] == registration["role"],
        "role_mismatch",
        "The local and suite roles disagree",
    )?;
    let value = index::build(&root, &volume, &catalog, &registration)?;
    index::validate(&value)?;
    let bytes = nu9ve_json::canonical(&value);
    require(
        bytes.len() <= index::LIMIT,
        "index_too_large",
        "Portable index exceeds 8 MiB",
    )?;
    backend.fast_check(&path, &info)?;
    require(
        marker(&root)?.as_ref() == Some(&volume),
        "volume_replaced",
        "Marker changed before publishing",
    )?;
    root.write_bytes(INDEX, &bytes, true)?;
    record(home, &volume, &info, known.as_ref())?;
    Ok(
        json!({"state":state, "path":INDEX, "generated_at":value["generated_at"],
        "dataset_count":value["datasets"].as_array().unwrap().len(), "bytes":bytes.len()}),
    )
}

pub fn adopt(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let (root, info) = mounted(backend, &selected(params)?)?;
    let volume =
        marker(&root)?.ok_or_else(|| Error::new("unmarked", "This volume has no suite marker"))?;
    let known = sighting(home, &volume)?;
    let state = observed(backend, &root, &info, &volume, known.as_ref());
    allowed(state)?;
    let Some(value) = document(&root, INDEX, index::LIMIT, "invalid_index")? else {
        return Err(Error::new(
            "missing_index",
            "This volume does not carry a nüün index",
        ));
    };
    index::validate(&value)?;
    if let Some(previous) = volume.get("previous_volume_id") {
        require(
            !value["roots"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| &entry["volume_id"] == previous),
            "stale_index",
            "Republish the index for the newly registered copy",
        )?;
    }
    let snapshot = index::verify_snapshot(&root, &value)?;
    backend.fast_check(&root.path, &info)?;
    require(
        marker(&root)?.as_ref() == Some(&volume),
        "volume_replaced",
        "Marker changed while reading",
    )?;
    let resolved_roots: Vec<Value> = value["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let resolved = if entry["volume_id"] == volume["volume_id"] {
                (state == "ok")
                    .then(|| resolve_path(&root, entry))
                    .flatten()
            } else if let Some(id) = entry["volume_id"].as_str() {
                resolve_other(home, backend, id, entry)
            } else {
                None
            };
            match resolved {
                Some(path) => json!({"id":entry["id"],"state":"ok","path":path}),
                None => json!({"id":entry["id"],"state":"missing"}),
            }
        })
        .collect();
    let mut result = json!({"state":state,"marker":volume,"generated_at":value["generated_at"],
        "dataset_count":value["datasets"].as_array().unwrap().len(),
        "project_count":value["projects"].as_array().unwrap().len(), "snapshot":snapshot,
        "receipts":value["nuun_receipts"],
        "roots":resolved_roots});
    if params["include_index"] == true {
        result["index"] = value;
    }
    Ok(result)
}

fn resolve_path(root: &Root, entry: &Value) -> Option<PathBuf> {
    let relative = entry["volume_path"].as_str()?;
    if relative.is_empty() {
        root.check().ok()?;
        return Some(root.path.clone());
    }
    // A fictitious leaf lets Root check only the directory and all its parents.
    root.parent(&format!("{relative}/index-probe"), false)
        .ok()?;
    Some(root.path.join(relative))
}

fn resolve_other(home: &Path, backend: &dyn Backend, id: &str, entry: &Value) -> Option<PathBuf> {
    backend.mounted_roots().into_iter().find_map(|path| {
        let root = Root::open(&path).ok()?;
        let other = marker(&root).ok()??;
        if other["volume_id"] != id {
            return None;
        }
        let (_, info) = mounted(backend, &path).ok()?;
        let known = sighting(home, &other).ok()?;
        if observed(backend, &root, &info, &other, known.as_ref()) != "ok" {
            return None;
        }
        resolve_path(&root, entry)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::test_backend::Fixture;

    #[test]
    fn read_only_adoption_and_serial_states() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let path = base.join("disk");
        let home = base.join("fresh");
        std::fs::create_dir(&path).unwrap();
        let root = Root::open(&path).unwrap();
        let id = "2b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e6f";
        let mut volume = json!({"contract":"nu9ve-volume/1","volume_id":id, "role":"work", "label":"Fixture",
            "registered_at":"2026-11-22T00:00:00Z","registered_by":{"app":"nuun","device_id":id},
            "fs":{"os":OS,"kind":"exfat","serial":"1234-5678"}});
        root.write_json(MARKER, &volume).unwrap();
        let mut backend = Fixture {
            roots: vec![path.clone()],
            ..Default::default()
        };
        let overrides = if OS == "windows" {
            json!({"device":0x12345678u32})
        } else {
            json!({"volume_uuid":"1234-5678"})
        };
        backend.overrides.insert(normalize(&path), overrides);
        let params = json!({"root":path});
        assert_eq!(
            adopt(&home, &backend, &params).unwrap_err().code,
            "missing_index"
        );
        let mut index = json!({"contract":"nu9ve-data/1","app":"nuun","generated_at":"2026-11-22T00:00:00Z",
            "projects":[],"datasets":[],"next_cursor":null,
            "roots":[{"id":"disk","label":"Fixture","state":"ok","volume_id":id,"volume_path":""}]});
        root.write_json(INDEX, &index).unwrap();
        let before = std::fs::read(path.join(INDEX)).unwrap();
        let result = adopt(&home, &backend, &params).unwrap();
        assert_eq!(result["state"], "ok");
        assert_eq!(result["roots"][0]["path"], json!(path));
        assert_eq!(std::fs::read(path.join(INDEX)).unwrap(), before);
        assert!(!home.exists());

        let other = base.join("duplicate");
        std::fs::create_dir(&other).unwrap();
        Root::open(&other)
            .unwrap()
            .write_json(MARKER, &volume)
            .unwrap();
        backend.roots.push(other);
        assert_eq!(
            adopt(&home, &backend, &params).unwrap_err().code,
            "duplicate"
        );
        backend.roots.pop();
        volume["fs"]["serial"] = json!("AABB-CCDD");
        root.write_json(MARKER, &volume).unwrap();
        assert_eq!(adopt(&home, &backend, &params).unwrap_err().code, "suspect");

        let other_os = if OS == "macos" { "windows" } else { "macos" };
        volume["fs"]["os"] = json!(other_os);
        root.write_json(MARKER, &volume).unwrap();
        assert_eq!(observe(&home, &backend, &params).unwrap()["state"], "ok");
        assert_eq!(
            sighting(&home, &volume).unwrap().unwrap()["serial"],
            "1234-5678"
        );
        if OS == "windows" {
            backend
                .overrides
                .insert(normalize(&path), json!({"device":null}));
        } else {
            backend
                .overrides
                .insert(normalize(&path), json!({"volume_uuid":null}));
        }
        let result = adopt(&home, &backend, &params).unwrap();
        assert_eq!(result["state"], "unverified");
        assert_eq!(result["roots"][0]["state"], "missing");
        assert_eq!(
            observe(&home, &backend, &params).unwrap()["state"],
            "unverified"
        );
        assert_eq!(
            sighting(&home, &volume).unwrap().unwrap()["serial"],
            "1234-5678"
        );

        index["app"] = json!("atlas");
        root.write_json(INDEX, &index).unwrap();
        assert_eq!(
            adopt(&home, &backend, &params).unwrap_err().code,
            "app_mismatch"
        );
        index["app"] = json!("nuun");
        index["roots"][0]["volume_path"] = json!("../elsewhere");
        root.write_json(INDEX, &index).unwrap();
        assert_eq!(
            adopt(&home, &backend, &params).unwrap_err().code,
            "invalid_index"
        );
    }
}
