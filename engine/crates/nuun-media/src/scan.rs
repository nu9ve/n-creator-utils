//! Source discovery: what is on a card and how its files form units. Port of `media/planner.py` (`scan`,
//! `finder_only_residue`, `group_key`, `grouped_files`), `manager/inventory.py` (`camera_evidence`, ignore lists)
//! and the `discover` method of `media/service.py`.
//!
//! Python walks with directory descriptors (`O_NOFOLLOW`/`O_DIRECTORY`), which Windows does not have, so Python's
//! planner runs only on macOS. Here every entry is stamped without following it and any link, junction or reparse
//! point is refused, as Python refuses a symlink.
//!
//! Divergences, on purpose and safe:
//! - `device` and `created_at` are independent per unit, as in service.py since its fix of 2026-11-09 (before it, a
//!   still without DateTimeOriginal lost its device and a dated still without Make inherited the previous one's).
//! - `mission` (missions.py, against the archive's flight-app export) is not ported: with `archive` and a device
//!   that has a mission flow, `discover` says so in `not_ported` instead of answering without it.
//! - Name collisions compare a lowercase form without Unicode NFC (no normalization crate). A collision this misses
//!   still cannot overwrite anything: copies land as `.ncpartial` and are renamed without replacing.
//! - Stalker sources answer `not_ported` in `discover` (session metadata lives in `captures.py`).

use crate::rules::relative;
use nu9ve_json::{Error, Result, digest, require};
use nu9ve_volumes::{EntryKind, stamp};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// manager/inventory.py `IGNORED_DIRS` and `IGNORED_FILES`.
const IGNORED_DIRS: [&str; 6] = [
    ".fseventsd",
    ".Spotlight-V100",
    ".Trashes",
    ".TemporaryItems",
    ".DocumentRevisions-V100",
    "System Volume Information",
];
const IGNORED_FILES: [&str; 2] = [".DS_Store", ".localized"];
use crate::photo_meta::PHOTO_SUFFIXES;
const MEDIA_KINDS: [&str; 4] = ["video", "audio", "image", "raw_image"];

/// Python `Path(name).suffix`, lowercased: the last dot that is not the first character.
pub(crate) fn suffix(name: &str) -> String {
    name.rfind('.')
        .filter(|i| *i > 0)
        .map(|i| name[i..].to_lowercase())
        .unwrap_or_default()
}

/// Python `Path(name).stem`.
fn stem(name: &str) -> &str {
    name.rfind('.')
        .filter(|i| *i > 0)
        .map_or(name, |i| &name[..i])
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

fn entries(folder: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut list: Vec<(String, PathBuf)> = std::fs::read_dir(folder)
        .map_err(|e| Error::new("operation_failed", format!("{}: {e}", folder.display())))?
        .map(|entry| {
            let entry = entry.map_err(|e| Error::new("operation_failed", e.to_string()))?;
            Ok((
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            ))
        })
        .collect::<Result<_>>()?;
    // Python sorts by name (code points); UTF-8 byte order is the same order.
    list.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(list)
}

/// True only for a manifest-less folder containing Finder metadata and empty folders.
pub fn finder_only_residue(folder: &Path) -> bool {
    let mut pending = vec![(folder.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    while let Some((current, depth)) = pending.pop() {
        if depth > 12 {
            return false;
        }
        let Ok(list) = entries(&current) else {
            return false;
        };
        for (name, path) in list {
            visited += 1;
            let Ok(s) = stamp(&path) else { return false };
            if visited > 256 || s.kind == EntryKind::Link {
                return false;
            }
            match s.kind {
                EntryKind::Dir => pending.push((path, depth + 1)),
                EntryKind::File
                    if name == ".DS_Store" || name == ".localized" || name.starts_with("._") => {}
                _ => return false,
            }
        }
    }
    true
}

/// Python `read_json(path, limit)`.
fn read_json(path: &Path, limit: u64) -> Result<Value> {
    let s = stamp(path)?;
    require(
        s.kind == EntryKind::File,
        "unsafe_file",
        format!("Not a regular file: {}", path.display()),
    )?;
    require(
        s.size <= limit,
        "document_too_large",
        "JSON input exceeds size limit",
    )?;
    let raw = std::fs::read(path).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    serde_json::from_slice(&raw).map_err(|_| Error::new("invalid_json", "Invalid JSON document"))
}

/// Python `uuid.UUID(text)`: 32 hex digits, optionally with dashes, braces or a `urn:uuid:` prefix.
fn valid_uuid(text: &str) -> bool {
    let trimmed = text.trim_start_matches("urn:").trim_start_matches("uuid:");
    let trimmed = trimmed.trim_start_matches('{').trim_end_matches('}');
    let hex: String = trimmed.chars().filter(|c| *c != '-').collect();
    hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Manufacturer and modelName of a Sony `Device` element (either may be absent).
type SonyDevice = (Option<String>, Option<String>);

/// The first `Device` element of a Sony `…M01.XML` sidecar; `Err` when the XML does not parse.
fn sony_device(xml: &[u8]) -> std::result::Result<Option<SonyDevice>, ()> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut seen_root = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if e.local_name().into_inner() == "Device" =>
            {
                let attr = |key: &str| -> Option<String> {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().into_inner() == key)
                        .map(|a| {
                            let raw: &str = &a.value;
                            quick_xml::escape::unescape(raw)
                                .map(|v| v.into_owned())
                                .unwrap_or_else(|_| raw.to_owned())
                        })
                };
                return Ok(Some((attr("manufacturer"), attr("modelName"))));
            }
            Ok(Event::Start(_)) => {
                depth += 1;
                seen_root = true;
            }
            Ok(Event::Empty(_)) => seen_root = true,
            Ok(Event::End(_)) => depth = depth.saturating_sub(1),
            // Python's ElementTree refuses a document without a root or with unclosed elements.
            Ok(Event::Eof) => {
                return if seen_root && depth == 0 {
                    Ok(None)
                } else {
                    Err(())
                };
            }
            Err(_) => return Err(()),
            _ => {}
        }
        buffer.clear();
    }
}

/// manager/inventory.py `camera_evidence`: file-local hints only. DJI filenames and residues prove no model.
pub fn camera_evidence(path: &Path) -> Vec<Value> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = suffix(&name);
    let mut evidence = Vec::new();
    if ext == ".insv" || ext == ".insp" {
        evidence.push(json!({"source": "extension", "family": "insta360", "confidence": "high"}));
    }
    let parts: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    if parts.windows(3).any(|w| w == ["private", "m4root", "clip"]) {
        evidence.push(json!({"source": "file_path", "family": "sony", "confidence": "medium"}));
    }
    if ext == ".mp4" {
        let sidecar = path.with_file_name(format!("{}M01.XML", stem(&name)));
        if let Ok(s) = stamp(&sidecar)
            && s.kind == EntryKind::File
            && s.size <= 1024 * 1024
            && let Ok(bytes) = std::fs::read(&sidecar)
        {
            match sony_device(&bytes) {
                Ok(Some((manufacturer, model))) => evidence
                    .push(json!({"source": "sony_xml", "confidence": "high",
                    "manufacturer": manufacturer, "model": model})),
                Ok(None) => {}
                Err(()) => evidence.push(json!({"source": "sony_xml", "error": "invalid_xml"})),
            }
        }
    }
    if name.to_uppercase().starts_with("DJI") {
        evidence.push(json!({"source": "filename", "family": "dji", "confidence": "low", "requires_review": true}));
    }
    evidence
}

/// Files, Stalker bundles and skipped active captures (Python `scan(…, photos=False)`).
pub struct Scan {
    pub files: Vec<Value>,
    pub bundles: Map<String, Value>,
    pub skipped: Vec<String>,
}

struct Walker<'a> {
    skip_active: bool,
    stalker_only: bool,
    scan: &'a mut Scan,
}

impl Walker<'_> {
    fn stat_file(&self, path: &Path, rel: &str) -> Result<nu9ve_volumes::Stamp> {
        let s = stamp(path)?;
        require(
            s.kind == EntryKind::File,
            "unsafe_file",
            format!("Not a regular file: {rel}"),
        )?;
        Ok(s)
    }

    fn visit(&mut self, folder: &Path, prefix: &str, mut bundle: Option<String>) -> Result<()> {
        let folder_name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder_suffix = suffix(&folder_name);
        require(
            !folder.join("clipper-project.json").exists()
                && folder_suffix != ".fcpbundle"
                && folder_suffix != ".fcpcache",
            "editor_project",
            "Do not ingest an editor project with camera media",
        )?;
        if folder_suffix == ".stalker" {
            let manifest_path = folder.join("manifest.json");
            if self.skip_active && !manifest_path.exists() && finder_only_residue(folder) {
                return Ok(()); // Finder metadata left by a release is not a capture.
            }
            let manifest = read_json(&manifest_path, 1024 * 1024)?;
            require(
                manifest.is_object(),
                "invalid_bundle",
                "Invalid Stalker manifest",
            )?;
            if manifest["status"] != "completed" {
                require(
                    self.skip_active,
                    "active_capture",
                    "Stalker session is not completed",
                )?;
                self.scan.skipped.push(if prefix.is_empty() {
                    ".".into()
                } else {
                    prefix.into()
                });
                return Ok(());
            }
            let version_ok = manifest["schemaVersion"]
                .as_i64()
                .is_some_and(|v| (1..=3).contains(&v))
                && !manifest["schemaVersion"].is_f64();
            require(
                manifest["producer"] == "stalker" && version_ok,
                "unsupported_bundle",
                "Unsupported Stalker producer/version",
            )?;
            require(
                manifest["sessionID"].as_str().is_some_and(valid_uuid),
                "invalid_bundle",
                "Invalid Stalker session ID",
            )?;
            let tracks = manifest["tracks"].as_array().filter(|t| !t.is_empty());
            require(tracks.is_some(), "invalid_bundle", "Stalker tracks missing")?;
            for track in tracks.into_iter().flatten() {
                let rel = track["relativePath"].as_str();
                require(
                    track.is_object() && rel.is_some(),
                    "invalid_bundle",
                    "Invalid track",
                )?;
                let rel = relative(rel.unwrap_or_default())?;
                self.stat_file(&folder.join(rel), &join(prefix, rel))?;
            }
            for key in ["voice", "events", "highlights"] {
                if let Some(name) = manifest["outputs"][key].as_str().filter(|n| !n.is_empty()) {
                    let name = relative(name)?;
                    self.stat_file(&folder.join(name), &join(prefix, name))?;
                }
            }
            let id = if prefix.is_empty() {
                ".".to_owned()
            } else {
                prefix.to_owned()
            };
            self.scan.bundles.insert(
                id.clone(),
                json!({"session_id": manifest["sessionID"], "manifest_digest": digest(&manifest)}),
            );
            bundle = Some(id);
        }
        for (name, path) in entries(folder)? {
            // A Stalker library may share Downloads with editor projects and exports: visit only capture bundles.
            if self.stalker_only && bundle.is_none() && !name.to_lowercase().ends_with(".stalker") {
                continue;
            }
            let rel = join(prefix, &name);
            relative(&rel)?;
            let s = stamp(&path)?;
            require(
                s.kind != EntryKind::Link,
                "unsafe_file",
                format!("Symlink in source: {rel}"),
            )?;
            if s.kind == EntryKind::Dir {
                if !IGNORED_DIRS.contains(&name.as_str()) {
                    self.visit(&path, &rel, bundle.clone())?;
                }
            } else if IGNORED_FILES.contains(&name.as_str()) || name.starts_with("._") {
                continue;
            } else {
                require(
                    s.kind == EntryKind::File,
                    "unsafe_file",
                    format!("Special entry in source: {rel}"),
                )?;
                let mut record = json!({
                    "path": rel,
                    "stamp": {"size": s.size, "mtime_ns": s.mtime_ns, "ctime_ns": s.ctime_ns, "inode": s.inode},
                    "bundle": bundle,
                    "evidence": camera_evidence(&path),
                });
                if bundle.is_none() && PHOTO_SUFFIXES.contains(&suffix(&name).as_str()) {
                    // Device identity of a still (Make+Model+Serial, flight XMP): bounded read, never a guess by name.
                    let photo = crate::photo_meta::read(&path);
                    if !photo.is_empty() {
                        record["photo"] = Value::Object(photo);
                    }
                }
                self.scan.files.push(record);
            }
        }
        Ok(())
    }
}

pub fn scan(root: &Path, skip_active: bool, stalker_only: bool) -> Result<Scan> {
    let root =
        std::path::absolute(root).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    for parent in root.ancestors() {
        if let Ok(s) = stamp(parent) {
            require(
                s.kind != EntryKind::Link,
                "unsafe_path",
                format!("Symlink in root path: {}", parent.display()),
            )?;
        }
    }
    require(
        stamp(&root)
            .map(|s| s.kind == EntryKind::Dir)
            .unwrap_or(false),
        "operation_failed",
        format!("Not a folder: {}", root.display()),
    )?;
    let mut result = Scan {
        files: Vec::new(),
        bundles: Map::new(),
        skipped: Vec::new(),
    };
    Walker {
        skip_active,
        stalker_only,
        scan: &mut result,
    }
    .visit(&root, "", None)?;
    let mut seen = BTreeSet::new();
    let unique = result
        .files
        .iter()
        .all(|f| seen.insert(f["path"].as_str().unwrap_or_default().to_lowercase()));
    require(
        unique,
        "name_collision",
        "Source has case/Unicode name collisions",
    )?;
    Ok(result)
}

/// Python `group_key`: sidecars share their media's unit; Sony `…M01.XML` joins its clip.
pub fn group_key(file: &Value) -> String {
    if let Some(bundle) = file["bundle"].as_str() {
        return format!("stalker:{bundle}");
    }
    let path = file["path"].as_str().unwrap_or_default();
    let (parent, name) = path.rsplit_once('/').unwrap_or((".", path));
    let mut upper = stem(name).to_uppercase();
    if suffix(name) == ".xml" && upper.ends_with("M01") {
        upper.truncate(upper.len() - 3);
    }
    format!("{parent}/{upper}")
}

/// Python `grouped_files`, keeping insertion order; standalone camera support goes last as `camera-support`.
pub fn grouped_files(files: &[Value]) -> Vec<(String, Vec<Value>)> {
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for file in files {
        let key = group_key(file);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(file.clone()),
            None => groups.push((key, vec![file.clone()])),
        }
    }
    let mut result = Vec::new();
    let mut support = Vec::new();
    for (key, members) in groups {
        let media = members.iter().any(|f| {
            crate::snapshot::kind_of(f["path"].as_str().unwrap_or_default())
                .is_some_and(|k| MEDIA_KINDS.contains(&k))
        });
        if members[0]["bundle"].is_null() && !media {
            support.extend(members);
        } else {
            result.push((key, members));
        }
    }
    if !support.is_empty() {
        result.push(("camera-support".to_owned(), support));
    }
    result
}

/// The `discover` method for cards: units with paths, bytes, evidence, category, camera and, for stills, the
/// device (recipe + `device-registry` of `catalog`) and the camera time. No catalog means the template and no
/// confirmed devices.
pub fn discover(catalog: Option<&crate::catalog::Catalog>, params: &Value) -> Result<Value> {
    let (recipe, registry) = match catalog {
        Some(c) => (
            nuun_recipe::effective(c.get("recipe")?.as_ref()),
            c.get("device-registry")?,
        ),
        None => (nuun_recipe::template(), None),
    };
    let archive = params["archive"].as_str().is_some_and(|a| !a.is_empty());
    let source = params["source"]
        .as_str()
        .ok_or_else(|| Error::new("operation_failed", "'source'"))?;
    if params["source_kind"] == "stalker" {
        return Err(Error::new(
            "not_ported",
            "Discovering a Stalker library is still in media/service.py (session metadata)",
        ));
    }
    let found = scan(Path::new(source), true, false)?;
    // Python adds session metadata to bundle units (captures.py); an answer without it would be incomplete.
    require(
        found.bundles.is_empty(),
        "not_ported",
        "This source holds Stalker sessions; their metadata is still read by media/service.py",
    )?;
    let mut units = Vec::new();
    let mut missions = false;
    for (key, members) in grouped_files(&found.files) {
        let evidence: Vec<Value> = members
            .iter()
            .flat_map(|f| f["evidence"].as_array().cloned().unwrap_or_default())
            .collect();
        let kinds: Vec<Option<&str>> = members
            .iter()
            .map(|f| crate::snapshot::kind_of(f["path"].as_str().unwrap_or_default()))
            .collect();
        let bundle = &members[0]["bundle"];
        let device = if key != "camera-support" {
            crate::devices::unit_device(&members, registry.as_ref(), &recipe)?
        } else {
            None
        };
        let captured = members
            .iter()
            .find_map(|f| f["photo"]["captured_at"].as_str().filter(|c| !c.is_empty()));
        let mut extra = Map::new();
        if let Some(device) = device.filter(|d| d["conflict"] != true) {
            let keep = [
                "make",
                "model",
                "source",
                "label",
                "category",
                "rule",
                "requires_confirmation",
            ];
            let shown: Map<String, Value> = keep
                .iter()
                .map(|k| ((*k).to_owned(), device[*k].clone()))
                .collect();
            // Python matches the still against the flight-app export of the archive (missions.py): not here yet.
            let flow = recipe["missions"].as_array().is_some_and(|flows| {
                flows.iter().any(|f| {
                    f["devices"]
                        .as_array()
                        .is_some_and(|d| d.contains(&device["category"]))
                })
            });
            if archive && device["requires_confirmation"] != true && flow && captured.is_some() {
                missions = true;
            }
            extra.insert("device".into(), Value::Object(shown));
        }
        if let Some(captured) = captured {
            extra.insert("created_at".into(), json!(captured)); // camera time (EXIF), for date filters
        }
        let category = if !bundle.is_null() {
            "stalker"
        } else {
            kinds
                .iter()
                .flatten()
                .find(|k| MEDIA_KINDS.contains(k))
                .copied()
                .unwrap_or("support")
        };
        let camera = evidence
            .iter()
            .filter_map(|e| e["model"].as_str())
            .find(|m| !m.is_empty())
            .or_else(|| {
                evidence
                    .iter()
                    .filter_map(|e| e["family"].as_str())
                    .find(|f| !f.is_empty())
            })
            .unwrap_or("");
        let mut unit = json!({
            "id": key,
            "paths": members.iter().map(|f| f["path"].clone()).collect::<Vec<_>>(),
            "bytes": members.iter().map(|f| f["stamp"]["size"].as_u64().unwrap_or(0)).sum::<u64>(),
            "evidence": evidence,
            "category": category,
            "camera": camera,
        });
        unit.as_object_mut().expect("object").extend(extra);
        units.push(unit);
    }
    let bytes: u64 = found
        .files
        .iter()
        .map(|f| f["stamp"]["size"].as_u64().unwrap_or(0))
        .sum();
    let mut result = json!({"units": units, "skipped": found.skipped, "bytes": bytes});
    if missions {
        // Python would match these stills against the archive's flight-app export (`mission`). Not yet: say so.
        result["not_ported"] = json!(["mission"]);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for (path, bytes) in files {
            let full = root.path().join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, bytes).unwrap();
        }
        root
    }

    fn paths(scan: &Scan) -> Vec<&str> {
        scan.files
            .iter()
            .map(|f| f["path"].as_str().unwrap())
            .collect()
    }

    const SONY_XML: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">
  <Device manufacturer="Sony" modelName="ILCE-6400" serialNo="123"/>
</NonRealTimeMeta>"#;

    #[test]
    fn scan_skips_metadata_and_keeps_sorted_relative_paths() {
        let root = tree(&[
            ("DCIM/100MSDCF/DSC0002.JPG", b"2"),
            ("DCIM/100MSDCF/DSC0001.JPG", b"1"),
            ("DCIM/100MSDCF/._DSC0001.JPG", b"x"),
            (".DS_Store", b"x"),
            ("System Volume Information/IndexerVolumeGuid", b"x"),
            (".Trashes/501/a", b"x"),
        ]);
        let found = scan(root.path(), false, false).unwrap();
        assert_eq!(
            paths(&found),
            ["DCIM/100MSDCF/DSC0001.JPG", "DCIM/100MSDCF/DSC0002.JPG"]
        );
        assert_eq!(found.files[0]["stamp"]["size"], 1);
        assert!(found.files[0]["bundle"].is_null());
    }

    #[test]
    fn sony_clip_its_xml_and_support_form_units_like_python() {
        let root = tree(&[
            ("PRIVATE/M4ROOT/CLIP/C0001.MP4", b"video"),
            ("PRIVATE/M4ROOT/CLIP/C0001M01.XML", SONY_XML),
            ("PRIVATE/M4ROOT/THMBNL/C0001T01.JPG", b"thumb"),
            ("PRIVATE/M4ROOT/MEDIAPRO.XML", b"<a/>"),
        ]);
        let data = discover(None, &json!({"source": root.path().display().to_string()})).unwrap();
        let units = data["units"].as_array().unwrap();
        assert_eq!(units[0]["id"], "PRIVATE/M4ROOT/CLIP/C0001");
        assert_eq!(
            units[0]["paths"],
            json!([
                "PRIVATE/M4ROOT/CLIP/C0001.MP4",
                "PRIVATE/M4ROOT/CLIP/C0001M01.XML"
            ])
        );
        assert_eq!(units[0]["category"], "video");
        assert_eq!(
            units[0]["camera"], "ILCE-6400",
            "the model from the sidecar wins over the family"
        );
        let last = units.last().unwrap();
        assert_eq!(last["id"], "camera-support");
        assert_eq!(last["category"], "support");
        assert_eq!(data["bytes"], json!(5 + SONY_XML.len() + 5 + 4));
        assert!(
            data.get("not_ported").is_none(),
            "a thumbnail is support, not a photo to read"
        );
    }

    #[test]
    fn evidence_follows_inventory_py() {
        let root = tree(&[
            ("DCIM/100MEDIA/DJI_0001.JPG", b"1"),
            ("DCIM/Camera01/VID_0001.insv", b"1"),
            ("CLIP/C0002.MP4", b"1"),
            ("CLIP/C0002M01.XML", b"<broken"),
        ]);
        let ev = |p: &str| camera_evidence(&root.path().join(p));
        assert_eq!(
            ev("DCIM/100MEDIA/DJI_0001.JPG"),
            [
                json!({"source": "filename", "family": "dji", "confidence": "low", "requires_review": true})
            ]
        );
        assert_eq!(ev("DCIM/Camera01/VID_0001.insv")[0]["family"], "insta360");
        assert_eq!(
            ev("CLIP/C0002.MP4"),
            [json!({"source": "sony_xml", "error": "invalid_xml"})]
        );
        let data = discover(None, &json!({"source": root.path().display().to_string()})).unwrap();
        assert!(
            data.get("not_ported").is_none(),
            "files without EXIF give no device and nothing missing"
        );
        assert!(data["units"][0].get("device").is_none());
    }

    #[test]
    fn stills_carry_device_and_camera_time_and_a_mission_is_named_missing() {
        let jpg = crate::photo_meta::sample::jpeg("DJI", "FC3170", "2026:05:14 10:20:30", None);
        let other = crate::photo_meta::sample::tiff("DJI", "FC9", "2026:05:14 10:21:00");
        let root = tree(&[
            ("DCIM/100MEDIA/DJI_0001.JPG", &jpg),
            (
                "DCIM/100MEDIA/DJI_0001.DNG",
                &crate::photo_meta::sample::tiff("DJI", "FC3170", "2026:05:14 10:20:30"),
            ),
            ("DCIM/100MEDIA/DJI_0002.JPG", &jpg),
            ("DCIM/100MEDIA/DJI_0002.DNG", &other),
        ]);
        let source = root.path().display().to_string();
        let data = discover(None, &json!({"source": source})).unwrap();
        let unit = &data["units"][0];
        assert_eq!(unit["id"], "DCIM/100MEDIA/DJI_0001");
        assert_eq!(
            unit["device"],
            json!({"make": "DJI", "model": "FC3170", "source": "MAVICAIR2", "label": "DJI Mavic Air 2",
                   "category": "dji-drone", "rule": "template", "requires_confirmation": false})
        );
        assert_eq!(unit["created_at"], "2026-05-14T10:20:30");
        // JPG and DNG disagree: no device (the user decides), but the camera time still filters by date.
        assert!(data["units"][1].get("device").is_none());
        assert_eq!(
            data["units"][1]["created_at"], "2026-05-14T10:21:00",
            "first member in scan order (DNG)"
        );
        assert!(
            data.get("not_ported").is_none(),
            "without an archive there is no mission to look up"
        );
        let with_archive = discover(None, &json!({"source": source, "archive": "vol-1"})).unwrap();
        assert_eq!(with_archive["not_ported"], json!(["mission"]));
    }

    #[test]
    fn stalker_bundles_are_validated_and_active_ones_skipped() {
        let manifest = |status: &str| {
            json!({"producer": "stalker", "schemaVersion": 3, "status": status,
                   "sessionID": "123e4567-e89b-12d3-a456-426614174000",
                   "tracks": [{"relativePath": "screen.mov"}], "outputs": {"events": "events.json"}})
            .to_string()
        };
        let done = manifest("completed");
        let active = manifest("recording");
        let root = tree(&[
            ("A.stalker/manifest.json", done.as_bytes()),
            ("A.stalker/screen.mov", b"v"),
            ("A.stalker/events.json", b"[]"),
            ("B.stalker/manifest.json", active.as_bytes()),
            ("B.stalker/screen.mov", b"v"),
            ("C.stalker/.DS_Store", b"x"),
            ("export.mp4", b"not a capture"),
        ]);
        let found = scan(root.path(), true, true).unwrap();
        assert_eq!(
            paths(&found),
            [
                "A.stalker/events.json",
                "A.stalker/manifest.json",
                "A.stalker/screen.mov"
            ]
        );
        assert_eq!(found.skipped, ["B.stalker"]);
        assert_eq!(
            found.bundles["A.stalker"]["session_id"],
            "123e4567-e89b-12d3-a456-426614174000"
        );
        assert_eq!(found.files[0]["bundle"], "A.stalker");
        assert_eq!(grouped_files(&found.files)[0].0, "stalker:A.stalker");
        // Without skip_active an active capture stops the scan.
        assert_eq!(
            scan(root.path(), false, true).err().unwrap().code,
            "active_capture"
        );
        assert_eq!(
            discover(
                None,
                &json!({"source": root.path().display().to_string(), "source_kind": "stalker"})
            )
            .unwrap_err()
            .code,
            "not_ported"
        );
    }

    #[test]
    fn broken_bundles_editor_projects_and_collisions_are_refused() {
        let missing_track = json!({"producer": "stalker", "schemaVersion": 2, "status": "completed",
            "sessionID": "123e4567e89b12d3a456426614174000", "tracks": [{"relativePath": "gone.mov"}]}).to_string();
        let root = tree(&[("S.stalker/manifest.json", missing_track.as_bytes())]);
        assert_eq!(
            scan(root.path(), true, false).err().unwrap().code,
            "operation_failed"
        );
        let bad_version =
            json!({"producer": "stalker", "schemaVersion": 9, "status": "completed"}).to_string();
        let root = tree(&[("S.stalker/manifest.json", bad_version.as_bytes())]);
        assert_eq!(
            scan(root.path(), true, false).err().unwrap().code,
            "unsupported_bundle"
        );
        let root = tree(&[("Proyecto/clipper-project.json", b"{}")]);
        assert_eq!(
            scan(root.path(), false, false).err().unwrap().code,
            "editor_project"
        );
        let root = tree(&[("Edit.fcpbundle/x", b"1")]);
        assert_eq!(
            scan(root.path(), false, false).err().unwrap().code,
            "editor_project"
        );
    }

    #[test]
    fn group_keys_follow_python() {
        let f = |path: &str| json!({"path": path, "bundle": null});
        assert_eq!(group_key(&f("C0001.MP4")), "./C0001");
        assert_eq!(group_key(&f("CLIP/c0001m01.xml")), "CLIP/C0001");
        assert_eq!(
            group_key(&f("DCIM/IMG_1.JPG")),
            group_key(&f("DCIM/IMG_1.CR3"))
        );
        assert_eq!(group_key(&json!({"path": "x", "bundle": "."})), "stalker:.");
        assert!(valid_uuid("{123E4567-E89B-12D3-A456-426614174000}"));
        assert!(!valid_uuid("not-a-uuid"));
    }
}
