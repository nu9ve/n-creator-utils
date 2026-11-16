//! Read-only ingest plans for cards. Port of `media/planner.py` (`_build_plan`, `validate_plan`,
//! `destination_path`, `preflight`, `build_plan`) and of `missions.WorkPresence`.
//!
//! The plan is the contract between review and apply: its `digest` (canonical JSON, SHA-256) and its ids
//! (`uuid5` of canonical JSON) must come out of the same inputs exactly as in Python, so every rule below follows
//! Python step by step, in Python's order of checks (the first failing check decides the error code).
//!
//! Cards, Stalker sources and deliveries share this planner. Stalker times retain the original clock evidence.

//! Divergence: the plan size guard uses the canonical length (Python measures `json.dumps` with spaces and ASCII
//! escapes, slightly longer); both stop far below the 12 MiB the apply side accepts.

use crate::catalog::Catalog;
use crate::layout::{MEDIA_KINDS, build_layout, sony_card_path, sony_index_member};
use crate::missions::{MissionIndex, index_for};
use crate::rules::{relative, slug};
use crate::scan::{grouped_files, scan};
use crate::snapshot::kind_of;
use crate::volumes::{Backend, check, independent};
use nu9ve_json::{Error, NAMESPACE_URL, Result, digest, require, uuid5};
use nu9ve_time::DateTime;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const PARAMS: [&str; 18] = [
    "source",
    "archive",
    "work",
    "project",
    "profile",
    "country",
    "city",
    "archive_only",
    "confirm_archive_only",
    "work_decision",
    "cleanup",
    "assignments",
    "camera",
    "source_kind",
    "finals_route",
    "delivery_paths",
    "finals_role",
    "handycam_history",
];

fn disjoint_target(source: &Path, root: &Path, prefix: &str, history: bool) -> bool {
    if history && is_parent(root, source) {
        let target = root.join(prefix).join("handycam");
        return key(source) != key(&target)
            && !is_parent(source, &target)
            && !is_parent(&target, source);
    }
    key(source) != key(root) && !is_parent(source, root) && !is_parent(root, source)
}

pub(crate) fn check_handycam_cleanup(
    cleanup: bool,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    require(
        !cleanup || !crate::layout::handycam_hdd(paths),
        "handycam_cleanup_in_camera",
        "Desactiva Limpiar SD. La Handycam se limpia desde su propio menú después de comprobar el respaldo.",
    )
}

pub(crate) fn transfer_budget(file: &Value) -> u64 {
    let size = file["stamp"]["size"].as_u64().unwrap_or_default();
    // Original temporary copy, temporary MP4 and published MP4.
    if file["handycam_original_sha256"].is_string() {
        size.saturating_mul(5)
    } else {
        size
    }
}

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

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

/// A Python `str` parameter: slug errors for anything else, as `slug(None)` raises `invalid_tag`.
fn slug_value(value: &Value) -> Result<String> {
    match value.as_str() {
        Some(s) => slug(s),
        None => Err(Error::new("invalid_tag", "Invalid project/profile tag")),
    }
}

/// Python `Path(source).expanduser().absolute()`.
pub(crate) fn absolute_source(source: &str) -> Result<PathBuf> {
    let expanded = match source.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or_else(|| {
                    Error::new("internal_error", "Could not determine home directory")
                })?;
            PathBuf::from(home).join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(source),
    };
    std::path::absolute(&expanded).map_err(|e| Error::new("internal_error", e.to_string()))
}

/// A comparable form of a path: on Windows the file system ignores case, so the comparison does too.
fn key(path: &Path) -> String {
    let text = path
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_owned();
    if cfg!(windows) {
        text.replace('/', "\\").to_lowercase()
    } else {
        text
    }
}

/// Python `root in path.parents` (lexical).
fn is_parent(root: &Path, path: &Path) -> bool {
    let root = key(root);
    path.ancestors().skip(1).any(|p| key(p) == root)
}

/// Python `str(Path(root) / rel)`.
fn joined(root: &str, rel: &str) -> String {
    let mut path = PathBuf::from(root);
    for part in rel.split('/') {
        path.push(part);
    }
    path.display().to_string()
}

/// Python `missions.WorkPresence`: stills already in the consumer's work root, by original name + size and
/// confirmed by EXIF time + serial (never by name alone). Folders starting with `_` or `.` are skipped.
struct WorkPresence {
    root: PathBuf,
    by_name: BTreeMap<(String, u64), Vec<PathBuf>>,
}

impl WorkPresence {
    fn new(work: &Value, flow: &Value) -> Self {
        let root = PathBuf::from(text(work, "root"));
        let base = format!(
            "{}/{}",
            text(work, "prefix").trim_matches('/'),
            text(flow, "work_root").trim_matches('/')
        );
        let mut by_name: BTreeMap<(String, u64), Vec<PathBuf>> = BTreeMap::new();
        let mut pending = vec![base.split('/').fold(root.clone(), |p, part| p.join(part))];
        while let Some(folder) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    if !name.starts_with('_') && !name.starts_with('.') {
                        pending.push(entry.path());
                    }
                } else if kind.is_file()
                    && crate::photo_meta::PHOTO_SUFFIXES
                        .contains(&crate::scan::suffix(&name).as_str())
                    && !name.starts_with("._")
                    && let Ok(meta) = entry.metadata()
                {
                    // Never follow a link out of the work root.
                    by_name
                        .entry((name.to_uppercase(), meta.len()))
                        .or_default()
                        .push(entry.path());
                }
            }
        }
        WorkPresence { root, by_name }
    }

    fn find(&self, original: &str, size: u64, photo: &Value) -> Option<String> {
        let serial = photo["serial"].as_str().filter(|s| !s.is_empty())?;
        for path in self.by_name.get(&(original.to_uppercase(), size))? {
            let other = crate::photo_meta::read(path);
            if other.get("captured_at") == photo.get("captured_at")
                && other.get("serial").and_then(Value::as_str) == Some(serial)
            {
                let rel = path.strip_prefix(&self.root).ok()?;
                return Some(
                    rel.components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
        None
    }
}

/// Python `planner.destination_path`: the destination below the volume root, or `None` when this role does not
/// receive the file.
pub fn destination_path(
    record: &Value,
    plan: &Value,
    unit: &Value,
    source_path: &str,
    selected_only: bool,
) -> Result<Option<String>> {
    relative(text(record, "prefix"))?;
    if selected_only && record["role"] == "work" && unit["work_selected"] == false {
        return Ok(None);
    }
    let role = text(record, "role");
    if plan.get("layout").is_some() {
        let target = &plan["layout"][source_path][role];
        let root = if plan["source_kind"] == "delivery" {
            record["finals_prefix"].as_str().unwrap_or("002_finales")
        } else {
            text(record, "prefix")
        };
        return Ok(match target.as_str() {
            None => None,
            Some(t) => Some(format!("{root}/{}", relative(t)?)),
        });
    }
    // Plans before schema 2 nest original trees under per-job/per-unit containers.
    let prefix = if role == "archive" {
        let base = if plan["source_kind"] == "delivery" {
            record["finals_prefix"].as_str().unwrap_or("002_finales")
        } else {
            text(record, "prefix")
        };
        let kind = if unit["kind"] == "stalker_session" {
            "stalker"
        } else {
            "camera-bundles"
        };
        format!("{base}/{kind}/{}", text(plan, "media_date"))
    } else {
        format!(
            "{}/{}_raw",
            text(record, "prefix"),
            slug(text(unit, "profile"))?
        )
    };
    Ok(Some(
        [
            prefix,
            slug(text(unit, "project"))?,
            text(plan, "id").to_owned(),
            text(unit, "id").to_owned(),
            relative(text(plan, "source_name"))?.to_owned(),
            relative(source_path)?.to_owned(),
        ]
        .join("/"),
    ))
}

fn is_uuid(text: &str) -> bool {
    let hex = text.replace('-', "");
    let dashed = text.len() == 36 && [8, 13, 18, 23].iter().all(|i| text.as_bytes()[*i] == b'-');
    let braced_or_plain = text.len() == 32;
    (dashed || braced_or_plain) && hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Python `planner.validate_plan`: a plan is applied only if it is exactly what was reviewed and coherent.
pub fn validate_plan(plan: &Value) -> Result<()> {
    require(
        plan.is_object()
            && matches!(
                plan["schema"].as_str(),
                Some("nu9ve-ingest-plan/1" | "nu9ve-ingest-plan/2")
            ),
        "invalid_plan",
        "Unsupported plan",
    )?;
    let mut raw = plan.clone();
    let supplied = raw.as_object_mut().expect("object").remove("digest");
    require(
        supplied.as_ref().and_then(Value::as_str) == Some(digest(&raw).as_str()),
        "plan_changed",
        "Plan digest mismatch",
    )?;
    let malformed = || Error::new("invalid_plan", "Malformed ingest plan");
    (|| -> Result<()> {
        let check = |ok: bool| if ok { Ok(()) } else { Err(malformed()) };
        check(is_uuid(plan["id"].as_str().ok_or_else(malformed)?))?;
        let units = plan["units"].as_array().ok_or_else(malformed)?;
        for unit in units {
            check(is_uuid(unit["id"].as_str().ok_or_else(malformed)?))?;
            slug_value(&unit["project"])?;
            slug_value(&unit["profile"])?;
            if truthy(&unit["support"]) {
                let paths = unit["paths"].as_array().ok_or_else(malformed)?;
                require(
                    unit["bundle"].is_null()
                        && unit["project"] == "camera-support"
                        && !paths.iter().any(|p| {
                            kind_of(p.as_str().unwrap_or_default())
                                .is_some_and(|k| MEDIA_KINDS.contains(&k))
                        }),
                    "invalid_plan",
                    "Invalid camera support unit",
                )?;
            }
        }
        require(
            plan["cleanup"].is_boolean(),
            "invalid_plan",
            "Invalid cleanup policy",
        )?;
        let kind = plan["source_kind"].as_str().ok_or_else(malformed)?;
        require(
            matches!(kind, "card" | "stalker" | "delivery"),
            "invalid_plan",
            "Invalid source type",
        )?;
        require(
            plan["cleanup"] == false
                || kind == "card"
                || (kind == "delivery" && plan.get("layout").is_some()),
            "invalid_plan",
            "Only cards and the finals route may be cleaned",
        )?;
        let release = plan
            .get("release_stalker_source")
            .cloned()
            .unwrap_or(json!(false));
        require(
            release.is_boolean()
                && (release == false
                    || kind == "stalker" && units.iter().any(|u| u["kind"] == "stalker_session")),
            "invalid_plan",
            "Invalid Stalker release policy",
        )?;
        relative(plan["media_date"].as_str().ok_or_else(malformed)?)?;
        let source_name = plan["source_name"].as_str().ok_or_else(malformed)?;
        relative(source_name)?;
        require(
            !source_name.contains('/'),
            "invalid_plan",
            "Source name must be one component",
        )?;
        let ids: BTreeMap<&str, &Value> = units.iter().map(|u| (text(u, "id"), u)).collect();
        require(ids.len() == units.len(), "invalid_plan", "Duplicate units")?;
        let mut seen = BTreeSet::new();
        for file in plan["files"].as_array().ok_or_else(malformed)? {
            let path = file["path"].as_str().ok_or_else(malformed)?;
            relative(path)?;
            let unit_id = file["unit_id"].as_str().ok_or_else(malformed)?;
            require(
                !seen.contains(path) && ids.contains_key(unit_id),
                "invalid_plan",
                "Duplicate file or unknown unit",
            )?;
            seen.insert(path.to_owned());
            let members = ids[unit_id]["paths"].as_array().ok_or_else(malformed)?;
            require(
                members.iter().any(|p| p == path),
                "invalid_plan",
                "Unit/member mismatch",
            )?;
            let size = &file["stamp"]["size"];
            if file.get("handycam_original_sha256").is_some() {
                require(
                    plan["handycam"] == "usb"
                        && plan["cleanup"] == false
                        && plan["work_id"].is_null()
                        && crate::handycam::needs_conversion(path)
                        && file["handycam_original_sha256"]
                            .as_str()
                            .is_some_and(|sha| {
                                sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit())
                            }),
                    "invalid_plan",
                    "Invalid Handycam conversion",
                )?;
            }
            require(
                size.is_u64() || size.as_i64().is_some_and(|s| s >= 0),
                "invalid_plan",
                "Invalid file size",
            )?;
        }
        let all: BTreeSet<String> = units
            .iter()
            .flat_map(|u| u["paths"].as_array().cloned().unwrap_or_default())
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect();
        require(
            !seen.is_empty() && seen == all,
            "invalid_plan",
            "Empty or inconsistent plan",
        )?;
        if let Some(retained) = plan.get("retained") {
            for (path, bytes) in retained.as_object().ok_or_else(malformed)? {
                relative(path)?;
                require(
                    kind == "card"
                        && bytes.is_u64()
                        && !seen.contains(path)
                        && !plan["discarded"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|item| item["stamps"].get(path).is_some()),
                    "invalid_plan",
                    "Retained files cannot be copied or discarded",
                )?;
            }
        }
        if kind == "card" {
            check_handycam_cleanup(
                plan["cleanup"] == true,
                seen.iter().map(String::as_str).chain(
                    plan["retained"]
                        .as_object()
                        .into_iter()
                        .flat_map(|files| files.keys().map(String::as_str)),
                ),
            )?;
        }
        if plan["handycam"] == "history" {
            require(
                kind == "card"
                    && plan["cleanup"] == false
                    && plan["work_id"].is_null()
                    && plan["layout"].as_object().is_some_and(|layout| {
                        layout.iter().all(|(path, target)| {
                            crate::handycam::is_history_movie(path)
                                && target["work"].is_null()
                                && text(target, "archive").starts_with("handycam/")
                        })
                    }),
                "invalid_plan",
                "An invalid Handycam history layout overlaps the source",
            )?;
        }
        if plan["schema"] == "nu9ve-ingest-plan/2"
            && (kind != "delivery" || plan.get("layout").is_some())
        {
            let layout = plan["layout"].as_object().ok_or_else(malformed)?;
            let covered: BTreeSet<String> = layout.keys().cloned().collect();
            require(
                covered == seen,
                "invalid_plan",
                "Layout must cover every planned file",
            )?;
            for role in ["archive", "work"] {
                let mut lowered = BTreeSet::new();
                let mut count = 0;
                for entry in layout.values() {
                    let target = &entry.get(role).ok_or_else(malformed)?;
                    if let Some(t) = target.as_str() {
                        relative(t)?;
                        lowered.insert(t.to_lowercase());
                        count += 1;
                    } else if !target.is_null() {
                        return Err(malformed());
                    }
                }
                require(
                    lowered.len() == count,
                    "invalid_plan",
                    "Two files share one destination",
                )?;
            }
            require(
                layout.values().all(|e| truthy(&e["archive"])),
                "invalid_plan",
                "Every file needs an archive destination",
            )?;
        }
        if plan.get("work_id").ok_or_else(malformed)?.is_null() {
            require(
                plan["archive_only_confirmed"] == true
                    && matches!(
                        plan["work_delivery_status"].as_str(),
                        Some("pending" | "skipped")
                    ),
                "confirmation_required",
                "Archive-only confirmation missing",
            )?;
        }
        require(
            matches!(
                plan["cleanup_profile"].as_str(),
                Some("files" | "camera_action_required" | "sony_clips_then_format")
            ),
            "invalid_plan",
            "Invalid cleanup profile",
        )?;
        let identity: BTreeSet<&str> = plan["source_identity"]
            .as_object()
            .ok_or_else(malformed)?
            .keys()
            .map(String::as_str)
            .collect();
        require(
            identity == BTreeSet::from(["mode", "volume_uuid", "disk_uuid", "root_inode"]),
            "invalid_plan",
            "Source identity is incomplete",
        )?;
        let sony = plan["files"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|f| sony_card_path(text(f, "path")))
            || units.iter().any(|u| u["camera"] == "sony");
        require(
            !sony
                || matches!(
                    plan["cleanup_profile"].as_str(),
                    Some("camera_action_required" | "sony_clips_then_format")
                ),
            "invalid_plan",
            "Sony cleanup requires camera action until a validated profile exists",
        )?;
        Ok(())
    })()
}

/// Python `planner.preflight`: destinations unchanged, online, apart from the source and, with `reserve`, with
/// room for the copies (rounded to blocks) plus a margin.
pub fn preflight(
    catalog: &Catalog,
    backend: &dyn Backend,
    plan: &Value,
    reserve: bool,
) -> Result<()> {
    validate_plan(plan)?;
    let mut infos = Vec::new();
    let mut ids = vec![text(plan, "archive_id").to_owned()];
    if let Some(work) = plan["work_id"].as_str() {
        ids.push(work.to_owned());
    }
    let units: BTreeMap<&str, &Value> = plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|u| (text(u, "id"), u))
        .collect();
    for volume_id in &ids {
        let record = catalog.volume(volume_id)?;
        let expected = if *volume_id == ids[0] {
            "archive"
        } else {
            "work"
        };
        require(
            record["role"] == expected,
            "invalid_role",
            "Destination role changed",
        )?;
        let registered =
            json!({"root": record["root"], "prefix": record["prefix"], "role": record["role"]});
        require(
            plan["destinations"][volume_id.as_str()] == registered,
            "destination_changed",
            "Registered destination changed after planning",
        )?;
        let info = check(backend, &record)?;
        let (source, root) = (
            PathBuf::from(text(plan, "source")),
            PathBuf::from(text(&record, "root")),
        );
        require(
            disjoint_target(
                &source,
                &root,
                text(&record, "prefix"),
                plan["handycam"] == "history",
            ),
            "overlapping_roots",
            "Overlapping roots",
        )?;
        let block = info["block_size"].as_u64().unwrap_or(0).max(4096);
        let mut needed = 0u64;
        for file in plan["files"].as_array().into_iter().flatten() {
            let unit = units
                .get(text(file, "unit_id"))
                .copied()
                .unwrap_or(&Value::Null);
            if destination_path(&record, plan, unit, text(file, "path"), true)?.is_some() {
                needed = needed
                    .saturating_add(transfer_budget(file).div_ceil(block).saturating_mul(block));
            }
        }
        if reserve {
            let free = info["free"].as_u64().unwrap_or(0);
            require(
                free >= needed + (64 * 1024 * 1024u64).max(needed / 20),
                "insufficient_space",
                format!("Insufficient free space on {}", text(&record, "name")),
            )?;
        }
        infos.push(info);
    }
    require(
        infos.len() == 1 || independent(&infos[0], &infos[1]),
        "same_physical_disk",
        "Destinations share a physical disk",
    )
}

/// Python `_build_plan` for card sources. `params` are the keyword arguments of the `plan` RPC.
pub fn build(catalog: &Catalog, backend: &dyn Backend, params: &Value) -> Result<Value> {
    build_with_stamps(catalog, backend, params, None)
}

/// `build`, with the file stamps replaced after the scan. Only the differential test against Python uses it: the
/// same bytes written again get other inodes and ctimes, and the unit ids hash the stamps.
fn build_with_stamps(
    catalog: &Catalog,
    backend: &dyn Backend,
    params: &Value,
    stamps: Option<&Map<String, Value>>,
) -> Result<Value> {
    let p = params.as_object().cloned().unwrap_or_default();
    if let Some(unexpected) = p.keys().find(|k| !PARAMS.contains(&k.as_str())) {
        return Err(Error::new(
            "internal_error",
            format!("_build_plan() got an unexpected keyword argument '{unexpected}'"),
        ));
    }
    let get = |k: &str, default: Value| p.get(k).cloned().unwrap_or(default);
    let source = p.get("source").and_then(Value::as_str).ok_or_else(|| {
        Error::new(
            "internal_error",
            "_build_plan() missing required argument: 'source'",
        )
    })?;
    let archive = p.get("archive").ok_or_else(|| {
        Error::new(
            "internal_error",
            "_build_plan() missing required argument: 'archive'",
        )
    })?;
    let work = get("work", Value::Null);
    let (archive_only, confirm) = (
        get("archive_only", json!(false)),
        get("confirm_archive_only", json!(false)),
    );
    let mut work_decision = get("work_decision", json!("pending"));
    let cleanup = get("cleanup", json!(false));
    let source_kind = get("source_kind", json!("card"));
    let finals_route = get("finals_route", json!(false));
    let finals_role = get("finals_role", Value::Null);
    let delivery_paths = get("delivery_paths", Value::Null);
    let history = get("handycam_history", json!(false));
    require(
        history.is_boolean()
            && (history == false
                || (source_kind == "card"
                    && cleanup == false
                    && archive_only == true
                    && work.is_null())),
        "invalid_policy",
        "El histórico Handycam sólo se copia al archivo",
    )?;
    require(
        archive_only.is_boolean() && confirm.is_boolean(),
        "invalid_policy",
        "Archive-only flags must be boolean",
    )?;
    require(
        matches!(source_kind.as_str(), Some("card" | "stalker" | "delivery")),
        "invalid_source_kind",
        "Invalid source kind",
    )?;
    require(
        cleanup.is_boolean(),
        "invalid_policy",
        "cleanup must be boolean",
    )?;
    require(
        finals_route == false || source_kind == "delivery",
        "invalid_policy",
        "Only deliveries follow the finals route",
    )?;
    require(
        finals_role.is_null()
            || matches!(
                finals_role.as_str(),
                Some("horizontal" | "vertical" | "thumbnail" | "auxiliary")
            ) && truthy(&finals_route)
                && !delivery_paths.is_null()
                && !truthy(&cleanup),
        "invalid_policy",
        "An explicit finals role requires a selected copy-only delivery",
    )?;
    let cleanup = cleanup == true;
    require(
        !cleanup || source_kind == "card" || truthy(&finals_route),
        "cleanup_not_supported",
        "Only cards and the finals route support automatic cleanup",
    )?;
    let retain = catalog
        .get("retain-stalker-source")?
        .unwrap_or(json!(false));
    require(
        retain.is_boolean(),
        "invalid_policy",
        "Invalid Stalker source retention setting",
    )?;
    let stalker = source_kind == "stalker";
    let release_stalker_source = stalker && retain == false;
    let project = match p.get("project") {
        None => String::new(),
        Some(Value::String(s)) if s.trim().is_empty() => String::new(),
        Some(other) => slug_value(other)?,
    };
    let profile = slug_value(&get("profile", json!("e64")))?;
    let country = slug_value(&get("country", json!("mx")))?;
    let city = slug_value(&get("city", json!("hermosillo")))?;
    let camera = get("camera", Value::Null);
    require(
        camera.is_null()
            || camera
                .as_str()
                .is_some_and(|c| nuun_recipe::CAMERAS.contains(&c)),
        "invalid_camera",
        "Unknown camera category",
    )?;
    let source = absolute_source(source)?;
    let card_cleanup = cleanup && source_kind == "card";
    let source_info = backend.describe(&source, card_cleanup)?;
    require(
        !card_cleanup
            || truthy(&source_info["external"])
                && key(Path::new(text(&source_info, "mount"))) == key(&source),
        "card_root_required",
        "Cleanup requires the root of a mounted external card",
    )?;
    let archive_name = archive
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| archive.to_string());
    let archive_record = catalog.volume(&archive_name)?;
    require(
        archive_record["role"] == "archive",
        "invalid_role",
        "Archive destination must have archive role",
    )?;
    let archive_info = check(backend, &archive_record)?;
    let work_record = if truthy(&work) {
        Some(catalog.volume(work.as_str().unwrap_or_default())?)
    } else {
        None
    };
    require(
        work_record.as_ref().is_none_or(|w| w["role"] == "work"),
        "invalid_role",
        "Work destination must have work role",
    )?;
    let expected_work = work_record
        .as_ref()
        .map(|w| w["id"].clone())
        .unwrap_or(Value::Null);
    let active_work = if archive_only == true {
        require(
            confirm == true,
            "confirmation_required",
            "Confirm archive-only for this operation",
        )?;
        require(
            matches!(work_decision.as_str(), Some("pending" | "skipped")),
            "invalid_policy",
            "Choose pending or skipped work delivery",
        )?;
        require(
            work_record.is_some() || work_decision == "skipped",
            "work_target_required",
            "Register/select the expected work volume for pending delivery",
        )?;
        None
    } else {
        let work = work_record.clone().ok_or_else(|| {
            Error::new(
                "work_required",
                "Select work volume, or explicitly confirm archive-only",
            )
        })?;
        let work_info = check(backend, &work)?;
        require(
            independent(&archive_info, &work_info),
            "same_physical_disk",
            "Archive and work must be different physical disks",
        )?;
        work_decision = json!("ready");
        Some(work)
    };
    let mut targets = vec![(&archive_record, false)];
    if let Some(w) = &active_work {
        targets.push((w, true));
    }
    for (target, is_work) in &targets {
        let root = std::path::absolute(text(target, "root"))
            .map_err(|e| Error::new("internal_error", e.to_string()))?;
        if *is_work && is_parent(&root, &source) {
            // A folder already on the work disk is its own editing copy: back it up only.
            return Err(Error::new(
                "source_on_work_disk",
                format!(
                    "This folder is already on {}: choose backup only (no editing copy)",
                    text(target, "name")
                ),
            ));
        }
        require(
            disjoint_target(&source, &root, text(target, "prefix"), history == true),
            "overlapping_roots",
            "Source and destination roots must not overlap",
        )?;
    }
    let found = scan(&source, stalker, stalker, true)?;
    let handycam = if history == true {
        Some("history")
    } else if source_kind == "card"
        && crate::layout::handycam_hdd(found.files.iter().map(|f| text(f, "path")))
    {
        Some("usb")
    } else {
        None
    };
    if source_kind == "card" {
        check_handycam_cleanup(cleanup, found.files.iter().map(|file| text(file, "path")))?;
    }
    let mut files = found.files;
    if handycam == Some("usb")
        && files
            .iter()
            .any(|f| crate::handycam::needs_conversion(text(f, "path")))
    {
        require(
            work.is_null() && archive_only == true,
            "invalid_policy",
            "Elige sólo respaldo para convertir los MPG de la Handycam",
        )?;
        let tree = crate::tree::Root::open(&source)?;
        for file in &mut files {
            let path = text(file, "path");
            if crate::handycam::needs_conversion(path) {
                file["handycam_original_sha256"] =
                    json!(crate::handycam::source_hash(&tree, path)?);
            }
        }
    }
    let mut bundles = found.bundles;
    if stalker {
        pending_stalker_files(catalog, &archive_record, &mut files, &mut bundles)?;
    }
    if let Some(stamps) = stamps {
        for file in files.iter_mut() {
            if let Some(stamp) = stamps.get(text(file, "path")) {
                file["stamp"] = stamp.clone();
            }
        }
    }
    if !delivery_paths.is_null() {
        let paths = delivery_paths
            .as_array()
            .filter(|items| !items.is_empty() && items.iter().all(Value::is_string));
        require(
            source_kind == "delivery" && !cleanup && paths.is_some(),
            "invalid_selection",
            "Select delivery files for copy-only",
        )?;
        let selected = paths
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(relative)
            .collect::<Result<BTreeSet<_>>>()?;
        let known: BTreeSet<_> = files.iter().map(|file| text(file, "path")).collect();
        require(
            selected.is_subset(&known) && bundles.is_empty(),
            "invalid_selection",
            "Delivery selection changed or contains bundles",
        )?;
        files.retain(|file| selected.contains(text(file, "path")));
    }
    let finals = if truthy(&finals_route) {
        Some(crate::projects::route_finals(
            &archive_record,
            &profile,
            &source,
            &mut files,
            finals_role.as_str(),
        )?)
    } else {
        None
    };
    let skipped = found.skipped;
    require(
        !files.is_empty(),
        "empty_source",
        "No completed files to ingest",
    )?;
    require(
        !cleanup || skipped.is_empty(),
        "active_capture",
        "Cannot clean a source with active captures",
    )?;
    let assignments = match get("assignments", Value::Null) {
        Value::Null => Map::new(),
        Value::Object(o) => o,
        Value::Array(a) if a.is_empty() => Map::new(),
        Value::String(s) if s.is_empty() => Map::new(),
        Value::Bool(false) => Map::new(),
        _ => {
            return Err(Error::new(
                "invalid_assignment",
                "Assignments must be a JSON object keyed by file path",
            ));
        }
    };
    let known: BTreeSet<&str> = files.iter().map(|f| text(f, "path")).collect();
    require(
        assignments.keys().all(|k| known.contains(k.as_str())),
        "invalid_assignment",
        "Assignment references an unknown file",
    )?;
    let mut groups = if source_kind == "delivery" {
        vec![("delivery".to_owned(), files.clone())]
    } else {
        grouped_files(&files)
    };
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    let source_key = {
        let mut keyed = Map::new();
        for k in ["mode", "volume_uuid", "disk_uuid", "root_inode"] {
            keyed.insert(
                k.into(),
                source_info
                    .get(k)
                    .cloned()
                    .ok_or_else(|| Error::new("internal_error", format!("'{k}'")))?,
            );
        }
        Value::Object(keyed)
    };
    let recipe = nuun_recipe::effective(catalog.get("recipe")?.as_ref());
    let registry = catalog.get("device-registry")?;
    let mut mission_indexes: BTreeMap<String, MissionIndex> = BTreeMap::new();
    let mut units: Vec<Value> = Vec::new();
    let mut discarded: Vec<Value> = Vec::new();
    let mut retained = Map::new();
    let mut unit_of: BTreeMap<String, String> = BTreeMap::new();
    for (group, members) in &groups {
        if history == true
            && !members
                .iter()
                .all(|file| crate::handycam::is_history_movie(text(file, "path")))
        {
            for file in members {
                retained.insert(text(file, "path").to_owned(), file["stamp"]["size"].clone());
            }
            continue;
        }
        let other = crate::scan::other_card_group(group);
        let support = group == "camera-support" || other;
        let choices: Vec<&Value> = if support && !other {
            Vec::new()
        } else {
            members
                .iter()
                .filter_map(|f| assignments.get(text(f, "path")))
                .collect()
        };
        for choice in &choices {
            let allowed = [
                "project",
                "profile",
                "country",
                "camera",
                "work",
                "discard",
                "include_other",
            ];
            require(
                choice
                    .as_object()
                    .is_some_and(|c| c.keys().all(|k| allowed.contains(&k.as_str()))),
                "invalid_assignment",
                "Invalid assignment fields",
            )?;
            require(
                choice.get("work").is_none_or(Value::is_boolean),
                "invalid_assignment",
                "work must be boolean",
            )?;
            require(
                choice.get("discard").is_none_or(Value::is_boolean),
                "invalid_assignment",
                "discard must be boolean",
            )?;
            require(
                choice
                    .get("include_other")
                    .is_none_or(|value| other && value.is_boolean())
                    && (!other || choice["discard"] != true),
                "invalid_assignment",
                "Other files must be backed up before cleanup",
            )?;
        }
        require(
            choices.iter().all(|c| *c == choices[0]),
            "assignment_conflict",
            "Paired files/bundles must be assigned together",
        )?;
        let empty = json!({});
        let assignment = choices.first().copied().unwrap_or(&empty);
        if other && assignment["include_other"] != true {
            let path = text(&members[0], "path");
            retained.insert(path.to_owned(), members[0]["stamp"]["size"].clone());
            continue;
        }
        if assignment["discard"] == true {
            // Discarded by the person: not backed up nor copied, and never deleted here.
            require(
                members[0]["bundle"].is_null(),
                "invalid_assignment",
                "A Stalker session cannot be discarded here",
            )?;
            let stamps: Map<String, Value> = members
                .iter()
                .map(|f| (text(f, "path").to_owned(), f["stamp"].clone()))
                .collect();
            discarded.push(json!({"paths": members.iter().map(|f| f["path"].clone()).collect::<Vec<_>>(),
                                  "bytes": members.iter().map(|f| f["stamp"]["size"].as_u64().unwrap_or(0)).sum::<u64>(),
                                  "stamps": stamps}));
            continue;
        }
        let mut selected = if support {
            json!("generic")
        } else {
            assignment.get("camera").cloned().unwrap_or(camera.clone())
        };
        require(
            selected.is_null()
                || selected
                    .as_str()
                    .is_some_and(|c| nuun_recipe::CAMERAS.contains(&c)),
            "invalid_camera",
            "Unknown assigned camera category",
        )?;
        let evidence: Vec<&Value> = members
            .iter()
            .flat_map(|f| f["evidence"].as_array().into_iter().flatten())
            .collect();
        let device = if support {
            None
        } else {
            crate::devices::unit_device(members, registry.as_ref(), &recipe)?
        };
        let decided = device
            .as_ref()
            .is_some_and(|d| truthy(&d["category"]) && d["requires_confirmation"] != true);
        if selected.is_null() && decided {
            // A registered device or a recipe rule decided it from the file's own EXIF, not from its name.
            selected = device.as_ref().expect("decided")["category"].clone();
        } else if selected.is_null() {
            let families: BTreeSet<&str> = evidence
                .iter()
                .filter(|e| truthy(&e["family"]) && e["confidence"] != "low")
                .filter_map(|e| e["family"].as_str())
                .collect();
            selected = if families.len() == 1 {
                json!(families.iter().next().expect("one"))
            } else {
                json!("generic")
            };
            let ambiguous = evidence.iter().any(|e| e["family"] == "dji")
                || device
                    .as_ref()
                    .is_some_and(|d| d["requires_confirmation"] == true);
            let hint = match device
                .as_ref()
                .and_then(|d| d["category"].as_str())
                .filter(|c| !c.is_empty())
            {
                Some(category) => format!(" (suggested: {category})"),
                None => String::new(),
            };
            require(
                !ambiguous,
                "camera_review_required",
                format!(
                    "Classify DJI files as drone/360/action in assignments before ingest{hint}"
                ),
            )?;
        }
        let selected_camera = selected.as_str().unwrap_or_default().to_owned();
        let flow = recipe["missions"].as_array().and_then(|flows| {
            flows.iter().find(|f| {
                f["devices"]
                    .as_array()
                    .is_some_and(|d| d.iter().any(|v| v == selected_camera.as_str()))
            })
        });
        let mut ordered: Vec<&Value> = members.iter().collect();
        ordered.sort_by_key(|f| kind_of(text(f, "path")) != Some("image"));
        let still = ordered.into_iter().find(|f| {
            f["photo"]["captured_at"]
                .as_str()
                .is_some_and(|c| !c.is_empty())
        });
        let mut mission: Option<Value> = None;
        if let (Some(flow), Some(still), Some(device)) = (flow, still, device.as_ref())
            && device["conflict"] != true
            && !support
        {
            // Mission or not, from the archive's registry: same original name, camera time and position.
            let id = text(flow, "id").to_owned();
            if !mission_indexes.contains_key(&id) {
                mission_indexes.insert(id.clone(), index_for(&archive_record, flow)?);
            }
            let name = text(still, "path").rsplit('/').next().unwrap_or_default();
            let mut found = mission_indexes[&id].matches(&still["photo"], name)?;
            found["flow"] = flow["id"].clone();
            mission = Some(found);
        }
        let mission_project = match (&mission, flow) {
            (Some(m), Some(flow)) if m["state"] == "matched" => Some(flow["project"].clone()),
            _ => None,
        };
        let requested = if support {
            json!("camera-support")
        } else if truthy(&assignment["project"]) {
            assignment["project"].clone()
        } else if let Some(p) = mission_project.filter(truthy) {
            p
        } else {
            json!(project)
        };
        require(
            requested.as_str().is_some_and(|r| !r.trim().is_empty()),
            "missing_project",
            "Asigna un proyecto a cada sesión Stalker o elige un proyecto para el lote",
        )?;
        let unit_project = slug_value(&requested)?;
        let unit_profile = slug_value(assignment.get("profile").unwrap_or(&json!(profile)))?;
        let unit_id = uuid5(
            &NAMESPACE_URL,
            &digest(&json!({"source": source_key, "members": members})),
        );
        let session = members[0]["bundle"]
            .as_str()
            .and_then(|bundle| bundles.get(bundle));
        let kind = if source_kind == "delivery" {
            "delivery"
        } else if session.is_some() {
            "stalker_session"
        } else if members.len() > 1 {
            "camera_bundle"
        } else {
            "file"
        };
        let mut unit = json!({"id": unit_id, "project": unit_project, "profile": unit_profile, "camera": selected_camera,
            "kind": kind, "paths": members.iter().map(|f| f["path"].clone()).collect::<Vec<_>>(), "bundle": members[0]["bundle"],
            "session_id": session.map(|value| &value["session_id"]),
            "capture_manifest_digest": session.map(|value| &value["manifest_digest"])});
        if support {
            unit["support"] = json!(true);
        }
        if support || selected_camera == "insta360" {
            unit["work_selected"] = json!(false);
        }
        if let Some(work) = assignment.get("work") {
            unit["work_selected"] = work.clone();
        }
        if let Some(country) = assignment.get("country") {
            unit["country"] = json!(slug_value(country)?);
        }
        if let Some(device) = device.as_ref().filter(|d| d["conflict"] != true) {
            let mut shown = Map::new();
            for k in ["make", "model", "serial", "source", "label", "rule"] {
                shown.insert(
                    k.into(),
                    device
                        .get(k)
                        .cloned()
                        .ok_or_else(|| Error::new("internal_error", format!("'{k}'")))?,
                );
            }
            shown.insert("category".into(), device["category"].clone());
            unit["device"] = Value::Object(shown);
        }
        if let Some(mission) = mission {
            unit["mission"] = mission;
        }
        for file in members {
            unit_of.insert(text(file, "path").to_owned(), unit_id.clone());
        }
        units.push(unit);
    }
    for file in files.iter_mut() {
        if let Some(id) = unit_of.get(text(file, "path")) {
            file["unit_id"] = json!(id);
        }
    }
    files.retain(|file| !retained.contains_key(text(file, "path")));
    if !discarded.is_empty() {
        let gone: BTreeSet<String> = discarded
            .iter()
            .flat_map(|d| d["paths"].as_array().cloned().unwrap_or_default())
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect();
        files.retain(|f| !gone.contains(text(f, "path")));
        require(
            !files.is_empty(),
            "empty_source",
            "Everything was discarded; nothing to back up",
        )?;
    }
    require(
        !files.is_empty(),
        "empty_source",
        "No files selected to back up",
    )?;
    require(
        archive_only == true
            || units
                .iter()
                .any(|u| u.get("work_selected").is_none_or(truthy)),
        "confirmation_required",
        "No editing copies selected; explicitly confirm archive-only",
    )?;
    units.sort_by_key(|u| truthy(&u["support"]));
    // A plan ID/digest is stable for an unchanged source, routing and policy.
    let newest = files
        .iter()
        .map(|f| f["stamp"]["mtime_ns"].as_i64().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let media_date = DateTime::fromtimestamp_utc(newest as f64 / 1e9).strftime("%Y/%m");
    let source_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    relative(&source_name)?;
    let mut plan = json!({"schema": "nu9ve-ingest-plan/2", "planner_revision": "ingest-flow/1",
        "source": source.display().to_string(), "source_identity": source_key, "source_kind": source_kind,
        "archive_id": archive_record["id"], "work_id": active_work.as_ref().map(|w| w["id"].clone()).unwrap_or(Value::Null),
        "expected_work_id": expected_work, "archive_only_confirmed": archive_only == true && confirm == true,
        "work_delivery_status": work_decision, "cleanup": cleanup, "country": country, "city": city,
        "release_stalker_source": release_stalker_source, "media_date": media_date, "source_name": source_name,
        "files": [], "units": [], "bundles": bundles, "skipped": skipped});
    if let Some(handycam) = handycam {
        plan["handycam"] = json!(handycam);
    }
    if !discarded.is_empty() {
        discarded.sort_by(|a, b| a["paths"][0].as_str().cmp(&b["paths"][0].as_str()));
        plan["discarded"] = json!(discarded);
    }
    if !retained.is_empty() {
        plan["retained"] = Value::Object(retained);
    }
    let configured = catalog.get("creative-profiles")?.unwrap_or(json!({}));
    let mut routes = Map::new();
    for unit in &units {
        let profile = text(unit, "profile");
        if let Some(entry) = configured.get(profile) {
            let raw = entry
                .get("raw_dir")
                .cloned()
                .ok_or_else(|| Error::new("internal_error", "'raw_dir'"))?;
            routes.insert(profile.to_owned(), raw);
        }
    }
    if !routes.is_empty() {
        plan["profile_routes"] = Value::Object(routes);
    }
    if units.iter().any(|u| truthy(&u["device"])) {
        // Frozen like profile routes: the recipe can change later without changing a reviewed plan.
        plan["recipe"] = nuun_recipe::recipe_ref(&recipe);
    }
    let mut folders = Map::new();
    let mut work_kinds = Map::new();
    for unit in &units {
        let camera = text(unit, "camera");
        if let Some(category) = recipe["categories"].get(camera) {
            folders.insert(
                camera.to_owned(),
                category
                    .get("archive")
                    .cloned()
                    .ok_or_else(|| Error::new("internal_error", "'archive'"))?,
            );
            if let Some(kinds) = category.get("work_kinds") {
                work_kinds.insert(camera.to_owned(), kinds.clone());
            }
        }
    }
    if !folders.is_empty() {
        plan["category_folders"] = Value::Object(folders);
        plan["category_work"] = Value::Object(work_kinds);
    }
    let flows: BTreeMap<String, Value> = recipe["missions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| (text(f, "id").to_owned(), f.clone()))
        .collect();
    let by_path: BTreeMap<String, Value> = files
        .iter()
        .map(|f| (text(f, "path").to_owned(), f.clone()))
        .collect();
    if !mission_indexes.is_empty()
        && let Some(work) = &active_work
    {
        // A drone still already in the consumer's work root (same name, size, EXIF time and serial) is not
        // copied again: the work copy is that existing file, and the units map points to it.
        let presence: BTreeMap<&String, WorkPresence> = mission_indexes
            .keys()
            .filter(|id| truthy(&flows[*id]["work_root"]))
            .map(|id| (id, WorkPresence::new(work, &flows[id])))
            .collect();
        for unit in units.iter_mut() {
            let flow_id = text(&unit["mission"], "flow").to_owned();
            let Some(index) = presence.get(&flow_id) else {
                continue;
            };
            let kinds = flows[&flow_id]["work_kinds"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut found = Map::new();
            for path in unit["paths"].as_array().cloned().unwrap_or_default() {
                let path = path.as_str().unwrap_or_default();
                let record = &by_path[path];
                if kind_of(path).is_some_and(|k| kinds.iter().any(|v| v == k))
                    && truthy(&record["photo"])
                {
                    let name = path.rsplit('/').next().unwrap_or_default();
                    if let Some(existing) = index.find(
                        name,
                        record["stamp"]["size"].as_u64().unwrap_or(0),
                        &record["photo"],
                    ) {
                        found.insert(path.to_owned(), json!(existing));
                    }
                }
            }
            if !found.is_empty() {
                unit["work_existing"] = Value::Object(found);
            }
        }
    }
    if !mission_indexes.is_empty() && active_work.is_none() {
        // Backing up a folder that already is the consumer's work copy: those stills are the work copy.
        for record in catalog.volumes()? {
            let root = std::path::absolute(text(&record, "root"))
                .map_err(|e| Error::new("internal_error", e.to_string()))?;
            if record["role"] != "work" || !is_parent(&root, &source) {
                continue;
            }
            for unit in units.iter_mut() {
                let Some(flow) = flows.get(text(&unit["mission"], "flow")) else {
                    continue;
                };
                if !truthy(&flow["work_root"]) {
                    continue;
                }
                let base = [
                    text(&record, "prefix").trim_matches('/'),
                    text(flow, "work_root").trim_matches('/'),
                ]
                .iter()
                .flat_map(|s| s.split('/'))
                .fold(root.clone(), |p, part| p.join(part));
                if key(&source) != key(&base) && !is_parent(&base, &source) {
                    continue;
                }
                let kinds = flow["work_kinds"].as_array().cloned().unwrap_or_default();
                let below: Vec<String> = source
                    .strip_prefix(&root)
                    .map(|r| {
                        r.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                let mut found = Map::new();
                for path in unit["paths"].as_array().cloned().unwrap_or_default() {
                    let path = path.as_str().unwrap_or_default();
                    if kind_of(path).is_some_and(|k| kinds.iter().any(|v| v == k)) {
                        let mut parts = below.clone();
                        parts.push(path.to_owned());
                        found.insert(path.to_owned(), json!(parts.join("/")));
                    }
                }
                if !found.is_empty() {
                    unit["work_existing"] = Value::Object(found);
                    plan["source_work_id"] = record["id"].clone();
                }
            }
        }
    }
    let state_of = |u: &Value| u["mission"]["state"].as_str().map(str::to_owned);
    let waiting = units
        .iter()
        .filter(|u| state_of(u).as_deref() == Some("pending_export"))
        .count();
    // A14: the card is the only source of these stills and the export only lives on the phone until shared.
    require(
        !(cleanup && waiting > 0),
        "mission_export_missing",
        format!(
            "{waiting} drone photos wait for the flight-app export: send it to Downloads and add it, then review again; or back up without cleaning the card"
        ),
    )?;
    if !mission_indexes.is_empty() {
        let mut summaries = Map::new();
        for (flow_id, index) in &mission_indexes {
            let flow = &flows[flow_id];
            let mut summary = json!({"history_sha256": index.history_sha256,
                "covered_until": index.covered_until.map(|c| c.isoformat()),
                "project": flow["project"],
                "layout": {"archive_folder": flow["archive_folder"], "work_folder": flow["work_folder"], "work_kinds": flow["work_kinds"]}});
            mission_counts(&mut summary, &units, flow_id);
            summaries.insert(flow_id.clone(), summary);
        }
        plan["missions"] = Value::Object(summaries);
    }
    plan["files"] = json!(files);
    plan["units"] = json!(units);
    let mut plan_units: Vec<Value> = units;
    if source_kind != "delivery" {
        plan["layout"] = Value::Object(build_layout(&source, &plan, &mut plan_units, &files)?);
    } else if let Some((layout, archived)) = finals {
        plan["layout"] = Value::Object(layout);
        plan["already_archived"] = json!(archived);
    }
    if history == true {
        let adopted = crate::handycam::review_history(
            &source,
            &archive_record,
            &files,
            plan["layout"].as_object_mut().expect("layout"),
        )?;
        if !adopted.is_empty() {
            files.retain(|file| !adopted.contains(text(file, "path")));
            require(
                !files.is_empty(),
                "empty_source",
                "Los MP4 de esta carpeta ya están respaldados",
            )?;
            for unit in &mut plan_units {
                if let Some(paths) = unit["paths"].as_array_mut() {
                    paths.retain(|path| !adopted.contains(path.as_str().unwrap_or_default()));
                }
            }
            plan_units.retain(|u| u["paths"].as_array().is_some_and(|paths| !paths.is_empty()));
            plan["files"] = json!(files);
            plan["already_archived"] = json!(adopted);
        }
    }
    plan["units"] = json!(plan_units);
    if source_kind != "delivery" && !mission_indexes.is_empty() && !cleanup {
        // A still already backed up by an earlier verified job at the same archive name and size stays out.
        let archived: BTreeMap<String, Value> = catalog
            .json_rows(
                "SELECT payload FROM copies WHERE volume_id=?",
                [text(&archive_record, "id")],
            )?
            .into_iter()
            .map(|r| {
                (
                    text(&r, "destination").to_owned(),
                    r.get("bytes").cloned().unwrap_or(Value::Null),
                )
            })
            .collect();
        let sizes: BTreeMap<String, u64> = files
            .iter()
            .map(|f| {
                (
                    text(f, "path").to_owned(),
                    f["stamp"]["size"].as_u64().unwrap_or(0),
                )
            })
            .collect();
        let mut skip = BTreeSet::new();
        for unit in plan["units"].as_array().into_iter().flatten() {
            if !truthy(&unit["mission"]) {
                continue;
            }
            let paths: Vec<&str> = unit["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let all = paths.iter().all(|path| {
                let target = format!(
                    "{}/{}",
                    text(&archive_record, "prefix"),
                    text(&plan["layout"][*path], "archive")
                );
                archived.get(&target).and_then(Value::as_u64) == sizes.get(*path).copied()
            });
            if all {
                skip.extend(paths.iter().map(|p| (*p).to_owned()));
            }
        }
        if !skip.is_empty() {
            let before: Vec<Value> = plan["units"].as_array().cloned().unwrap_or_default();
            files.retain(|f| !skip.contains(text(f, "path")));
            let kept: Vec<Value> = before
                .iter()
                .filter(|u| {
                    !u["paths"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|p| skip.contains(p.as_str().unwrap_or_default()))
                })
                .cloned()
                .collect();
            let layout = plan["layout"].as_object_mut().expect("object");
            for path in &skip {
                layout.remove(path);
            }
            plan["already_archived"] = json!(skip.iter().collect::<Vec<_>>());
            require(
                !files.is_empty(),
                "empty_source",
                "Everything here is already backed up",
            )?;
            if let Some(summaries) = plan.get_mut("missions").and_then(Value::as_object_mut) {
                for (flow_id, summary) in summaries.iter_mut() {
                    // Counts describe what this plan backs up, not what an earlier job already did.
                    mission_counts(summary, &kept, flow_id);
                    let gone = before
                        .iter()
                        .filter(|u| text(&u["mission"], "flow") == flow_id)
                        .filter(|u| {
                            u["paths"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|p| skip.contains(p.as_str().unwrap_or_default()))
                        })
                        .count();
                    summary["already_archived"] = json!(gone);
                }
            }
            plan["files"] = json!(files);
            plan["units"] = json!(kept);
        }
    }
    let sony = files.iter().any(|f| sony_card_path(text(f, "path")))
        || plan["units"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|u| u["camera"] == "sony");
    // Sony: after verification the clips leave the card, but the camera's own indexes are never edited here.
    plan["cleanup_profile"] = json!(if sony {
        "sony_clips_then_format"
    } else {
        "files"
    });
    plan["id"] = json!(uuid5(&NAMESPACE_URL, &digest(&plan)));
    let mut records = vec![archive_record.clone()];
    if let Some(w) = &active_work {
        records.push(w.clone());
    }
    let mut destinations = Map::new();
    for record in &records {
        destinations.insert(
            text(record, "id").to_owned(),
            json!({"root": record["root"], "prefix": record["prefix"], "role": record["role"]}),
        );
    }
    plan["destinations"] = Value::Object(destinations);
    let units_by_id: BTreeMap<String, Value> = plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|u| (text(u, "id").to_owned(), u.clone()))
        .collect();
    let mut preview = Vec::new();
    for file in &files {
        let unit = &units_by_id[text(file, "unit_id")];
        let path = text(file, "path");
        let archive_rel =
            destination_path(&archive_record, &plan, unit, path, true)?.unwrap_or_default();
        let work_path = match &active_work {
            Some(w) => destination_path(w, &plan, unit, path, true)?
                .map(|rel| joined(text(w, "root"), &rel)),
            None => None,
        };
        let action = if release_stalker_source {
            "release_clipper"
        } else if !cleanup {
            "retain"
        } else if plan["cleanup_profile"] == "sony_clips_then_format" && sony_index_member(path) {
            "camera_format"
        } else {
            "release_verified"
        };
        preview.push(json!({"source": path, "bytes": file["stamp"]["size"], "project": unit["project"],
            "profile": unit["profile"], "camera": unit["camera"],
            "archive_path": joined(text(&archive_record, "root"), &archive_rel), "work_path": work_path,
            "source_action": action}));
        if file["handycam_original_sha256"].is_string() {
            preview.last_mut().expect("preview row")["conversion"] = json!(true);
        }
    }
    preview.sort_by(|a, b| text(a, "archive_path").cmp(text(b, "archive_path")));
    plan["preview"] = json!(preview);
    for record in &records {
        // Fail at review time, not mid-copy, when a different file already holds a planned name.
        for file in &files {
            let unit = &units_by_id[text(file, "unit_id")];
            let Some(rel) = destination_path(record, &plan, unit, text(file, "path"), true)? else {
                continue;
            };
            if !file["handycam_original_sha256"].is_string()
                && let Some(size) = crate::tree::file_size(Path::new(text(record, "root")), &rel)?
            {
                require(
                    Some(size) == file["stamp"]["size"].as_u64(),
                    "destination_conflict",
                    format!("A different file already exists at {rel}"),
                )?;
            }
        }
    }
    require(
        files.len() <= 100_000 && nu9ve_json::canonical(&plan).len() <= 12 * 1024 * 1024,
        "plan_too_large",
        "Split this source into smaller ingest operations",
    )?;
    plan["digest"] = json!(digest(&plan));
    preflight(catalog, backend, &plan, true)?;
    Ok(plan)
}

/// The per-flow counts of a plan's `missions` summary.
fn mission_counts(summary: &mut Value, units: &[Value], flow_id: &str) {
    let mine: Vec<&Value> = units
        .iter()
        .filter(|u| text(&u["mission"], "flow") == flow_id)
        .collect();
    let state = |u: &&Value, s: &str| u["mission"]["state"] == s;
    let mut counts = Map::new();
    for s in ["matched", "none", "pending_export"] {
        counts.insert(s.into(), json!(mine.iter().filter(|u| state(u, s)).count()));
    }
    summary["units"] = Value::Object(counts);
    summary["likely"] = json!(
        mine.iter()
            .filter(|u| state(u, "pending_export") && truthy(&u["mission"]["likely_mission"]))
            .count()
    );
    summary["unregistered_likely"] = json!(
        mine.iter()
            .filter(|u| state(u, "none") && truthy(&u["mission"]["likely_mission"]))
            .count()
    );
    summary["work_existing"] = json!(mine.iter().filter(|u| truthy(&u["work_existing"])).count());
}

pub(crate) fn pending_stalker_files(
    catalog: &Catalog,
    archive: &Value,
    files: &mut Vec<Value>,
    bundles: &mut Map<String, Value>,
) -> Result<()> {
    let mut present = BTreeSet::new();
    for (bundle, session) in bundles.iter() {
        let assets = catalog.json_rows(
            "SELECT assets.payload FROM assets JOIN sessions ON sessions.asset_id=assets.id WHERE sessions.session_id=? AND sessions.manifest_digest=?",
            [text(session, "session_id"), text(session, "manifest_digest")],
        )?;
        for asset in assets {
            let paths: BTreeSet<_> = asset["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let copies = catalog.json_rows(
                "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
                [text(&asset, "job_id"), text(archive, "id")],
            )?;
            let copies: BTreeMap<_, _> = copies
                .iter()
                .filter(|copy| paths.contains(text(copy, "path")))
                .map(|copy| (text(copy, "path"), copy))
                .collect();
            if paths.len() != copies.len() {
                continue;
            }
            let verified = (|| -> Result<bool> {
                let target = crate::tree::Root::open(Path::new(text(archive, "root")))?;
                for copy in copies.values() {
                    if target
                        .stat(text(copy, "destination"))?
                        .is_none_or(|stamp| crate::tree::stamp_json(&stamp) != copy["stamp"])
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            })();
            if matches!(verified, Ok(true)) {
                present.insert(bundle.clone());
                break;
            }
        }
    }
    files.retain(|file| {
        file["bundle"]
            .as_str()
            .is_some_and(|bundle| !present.contains(bundle))
    });
    let retained: BTreeSet<_> = files
        .iter()
        .filter_map(|file| file["bundle"].as_str())
        .collect();
    bundles.retain(|bundle, _| retained.contains(bundle.as_str()));
    Ok(())
}

pub(crate) fn check_retitle(catalog: &Catalog, plan: &Value) -> Result<Vec<String>> {
    let mut sessions = Vec::new();
    let mut operations = None;
    for unit in plan["units"].as_array().into_iter().flatten() {
        let Some(session) = unit["session_id"].as_str().filter(|id| !id.is_empty()) else {
            continue;
        };
        let id = crate::captures::python_uuid(session)
            .ok_or_else(|| Error::new("internal_error", "Invalid session UUID"))?;
        if operations.is_none() {
            operations = Some(
                catalog.json_rows("SELECT value FROM metadata WHERE key LIKE 'retitle:%'", [])?,
            );
        }
        require(
            !operations
                .as_ref()
                .into_iter()
                .flatten()
                .any(|op| op["request"]["session_id"] == id && op["state"] != "applied"),
            "retitle_pending",
            "Recover the pending Stalker retitle before planning/applying",
        )?;
        let latest = catalog.get(&format!("retitle-result:{id}"))?;
        require(
            latest
                .is_none_or(|latest| latest["manifest_digest"] == unit["capture_manifest_digest"]),
            "plan_changed",
            "Stalker metadata changed; discard the old plan and review again",
        )?;
        sessions.push(id);
    }
    Ok(sessions)
}

/// Python `build_plan`: under the writer lock, build the plan and remember it as reviewed (`reviewed-plan:<id>`).
pub fn build_plan(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let _lock = crate::catalog::writer_lock(home)?;
    let writable = crate::catalog::WritableCatalog::open(home)?;
    let catalog = Catalog::open(home)?;
    let plan = build(&catalog, backend, params)?;
    let sessions = check_retitle(&catalog, &plan)?;
    let mut reviewed = json!({"id": plan["id"], "digest": plan["digest"], "sessions": sessions, "cancelled": false});
    if plan["source_kind"] == "card"
        && plan["missions"]
            .as_object()
            .is_some_and(|flows| !flows.is_empty())
    {
        reviewed["mission_request"] = params.clone();
    }
    writable.set(&format!("reviewed-plan:{}", text(&plan, "id")), &reviewed)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::photo_meta::sample;
    use crate::volumes::test_backend::{Setup, setup};

    fn pair(s: &Setup, folder: &str, stem: &str, model: &str) {
        let dir = s.card.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{stem}.JPG")),
            sample::jpeg("DJI", model, "2026:11:04 16:32:12", None),
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("{stem}.DNG")),
            sample::tiff("DJI", model, "2026:11:04 16:32:12"),
        )
        .unwrap();
    }

    fn params(s: &Setup, extra: Value) -> Value {
        let mut p = json!({"source": s.card.display().to_string(), "archive": "MEDIA", "work": "ESSD", "project": "intro"});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        p
    }

    fn plan(s: &Setup, extra: Value) -> Result<Value> {
        build(
            &Catalog::open(&s.home).unwrap(),
            &s.backend,
            &params(s, extra),
        )
    }

    fn media_unit(plan: &Value) -> &Value {
        plan["units"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| !truthy(&u["support"]))
            .unwrap()
    }

    #[test]
    fn handycam_history_skips_exact_duplicates_and_keeps_distinct_variants() {
        let s = setup();
        for (folder, data) in [("a", b"same"), ("b", b"same"), ("c", b"diff")] {
            let path = s.card.join(folder);
            std::fs::create_dir(&path).unwrap();
            let file = std::fs::File::create(path.join("M2U00001_720.mp4")).unwrap();
            use std::io::Write;
            (&file).write_all(data).unwrap();
            file.set_times(
                std::fs::FileTimes::new().set_modified(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1767340800),
                ),
            )
            .unwrap();
        }
        let plan = plan(
            &s,
            json!({"handycam_history": true, "work": null,
            "archive_only": true, "confirm_archive_only": true, "work_decision": "skipped"}),
        )
        .unwrap();
        assert_eq!(plan["files"].as_array().unwrap().len(), 2);
        assert_eq!(plan["already_archived"], json!(["b/M2U00001_720.mp4"]));
        let paths: Vec<_> = plan["layout"]
            .as_object()
            .unwrap()
            .values()
            .map(|entry| text(entry, "archive"))
            .collect();
        assert!(
            paths
                .iter()
                .all(|path| path.starts_with("handycam/2026/01/"))
        );
        assert!(paths.iter().any(|path| path.ends_with("-M2U00001[].MP4")));
        assert_ne!(paths[0], paths[1]);
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn handycam_hdd_is_copy_only_even_if_it_looks_like_a_sony_card() {
        let s = setup();
        let movie = "MP_ROOT/101PNV01/M2U00001.MPG";
        std::fs::create_dir_all(s.card.join("MP_ROOT/101PNV01")).unwrap();
        std::fs::write(s.card.join(movie), b"fixture").unwrap();
        assert_eq!(
            plan(&s, json!({"cleanup": true})).unwrap_err().code,
            "handycam_cleanup_in_camera"
        );
        let copy = plan(
            &s,
            json!({"cleanup": false, "work": null, "archive_only": true,
            "confirm_archive_only": true, "work_decision": "skipped"}),
        )
        .unwrap();
        assert_eq!(copy["preview"][0]["source_action"], "retain");
        let mut unsafe_plan = copy;
        unsafe_plan["cleanup"] = json!(true);
        unsafe_plan["files"][0]
            .as_object_mut()
            .unwrap()
            .remove("handycam_original_sha256");
        unsafe_plan.as_object_mut().unwrap().remove("digest");
        unsafe_plan["digest"] = json!(digest(&unsafe_plan));
        assert_eq!(
            validate_plan(&unsafe_plan).unwrap_err().code,
            "handycam_cleanup_in_camera"
        );
    }

    #[test]
    fn h6_session_uses_broadcast_wav_time_and_keeps_tracks_with_the_project() {
        let s = setup();
        let dir = "FOLDER01/ZOOM0001";
        std::fs::create_dir_all(s.card.join(dir)).unwrap();
        let mut wav = b"RIFF".to_vec();
        wav.extend(350u32.to_le_bytes());
        wav.extend(b"WAVEbext");
        wav.extend(338u32.to_le_bytes());
        let mut fields = [0; 338];
        let originator = b"ZOOM Handy Recorder H6";
        fields[256..256 + originator.len()].copy_from_slice(originator);
        fields[320..].copy_from_slice(b"2026-10-0221:06:30");
        wav.extend(fields);
        for name in ["ZOOM0001_LR.WAV", "ZOOM0001_Tr1.WAV", "ZOOM0001_Tr2.WAV"] {
            std::fs::write(s.card.join(dir).join(name), &wav).unwrap();
        }
        std::fs::write(s.card.join(dir).join("ZOOM0001.hprj"), b"project").unwrap();
        let plan = plan(&s, json!({})).unwrap();
        assert_eq!(plan["units"].as_array().unwrap().len(), 1);
        let unit = media_unit(&plan);
        assert_eq!(unit["camera"], "h6");
        assert_eq!(unit["capture_time"]["original"], "2026-10-02T21:06:30");
        assert_eq!(unit["paths"].as_array().unwrap().len(), 4);
        for (original, dest) in plan["layout"].as_object().unwrap() {
            let basename = original.rsplit('/').next().unwrap();
            assert_eq!(
                dest["archive"],
                format!("recordings/2026/10/26OCT02-210630_ZOOM0001_MX-INTRO[]/{basename}")
            );
            assert_eq!(
                dest["work"],
                format!("e64_raw/intro/26OCT02-210630_ZOOM0001_MX-INTRO[]/{basename}")
            );
        }
    }

    #[test]
    fn canon_clip_uses_its_own_model_and_capture_clock() {
        let s = setup();
        let clip = "DCIM/100CANON/MVI_0001.MP4";
        std::fs::create_dir_all(s.card.join("DCIM/100CANON")).unwrap();
        std::fs::write(
            s.card.join(clip),
            sample::canon_mp4("Canon EOS Rebel SL3", "2026:10:31 23:21:13"),
        )
        .unwrap();
        let plan = plan(&s, json!({})).unwrap();
        let unit = media_unit(&plan);
        assert_eq!(unit["camera"], "canon");
        assert_eq!(unit["device"]["model"], "Canon EOS Rebel SL3");
        assert_eq!(
            unit["capture_time"]["provenance"],
            "canon_mp4.exif.DateTimeOriginal"
        );
        assert_eq!(
            unit["capture_time"]["captured_at_absolute"],
            "2026-11-01T06:21:13Z"
        );
        assert!(
            text(&plan["layout"][clip], "archive")
                .starts_with("videos/2026/10/26OCT31-232113_CANONEOSREBELSL3_")
        );
        assert!(text(&plan["layout"][clip], "work").starts_with("e64_raw/intro/"));
    }

    #[test]
    fn a_known_drone_is_classified_by_exif_and_named_by_the_recipe() {
        let s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC3170");
        std::fs::create_dir_all(s.card.join("MISC/THM/101")).unwrap();
        std::fs::write(s.card.join("MISC/THM/101/DJI_0001.THM"), b"t").unwrap();
        let plan = plan(&s, json!({})).unwrap();
        let unit = media_unit(&plan);
        assert_eq!(
            (unit["camera"].as_str(), unit["kind"].as_str()),
            (Some("dji-drone"), Some("camera_bundle"))
        );
        assert_eq!(
            unit["device"],
            json!({"make": "DJI", "model": "FC3170", "serial": null, "source": "MAVICAIR2",
                   "label": "DJI Mavic Air 2", "rule": "template", "category": "dji-drone"})
        );
        assert_eq!(plan["recipe"]["id"], "nu9ve-default");
        assert_eq!(
            plan["layout"]["DCIM/101MEDIA/DJI_0001.JPG"]["archive"],
            "dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].JPG"
        );
        assert_eq!(
            plan["layout"]["DCIM/101MEDIA/DJI_0001.DNG"],
            json!({"archive": "dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].DNG", "work": null})
        );
        // No registry on the archive yet: the still waits for the export, so nothing goes to work.
        assert_eq!(unit["mission"]["state"], "pending_export");
        assert_eq!(plan["missions"]["subaereo"]["units"]["pending_export"], 1);
        assert!(plan["layout"]["DCIM/101MEDIA/DJI_0001.JPG"]["work"].is_null());
        // The support unit carries the card's device.
        let support = plan["units"].as_array().unwrap().last().unwrap();
        assert_eq!(
            (
                support["project"].as_str(),
                support["work_selected"].as_bool()
            ),
            (Some("camera-support"), Some(false))
        );
        assert!(
            text(&plan["layout"]["MISC/THM/101/DJI_0001.THM"], "archive")
                .starts_with("metadata/dji/support/2026/11/0411-163212_MX-ingest-")
        );
        assert_eq!(plan["cleanup_profile"], "files");
        assert!(
            plan["preview"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["source_action"] == "retain")
        );
        validate_plan(&plan).unwrap();
        assert!(is_uuid(text(&plan, "id")) && is_uuid(text(unit, "id")));
        // The same source and policy give the same plan.
        let again = super::build(
            &Catalog::open(&s.home).unwrap(),
            &s.backend,
            &params(&s, json!({})),
        )
        .unwrap();
        assert_eq!(
            (again["id"].clone(), again["digest"].clone()),
            (plan["id"].clone(), plan["digest"].clone())
        );
    }

    #[test]
    fn a_matched_mission_takes_the_project_folder_and_only_its_jpg_goes_to_work() {
        let s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC3170");
        let area = s.archive.join("004_datos/subaereo/coverage");
        std::fs::create_dir_all(&area).unwrap();
        let record = |when: &str, name: &str, cid: &str| {
            json!({"captureID": cid, "planID": "P1", "sessionID": "S1", "mediaCreatedAt": when,
                   "imageJPEGReference": name, "latitude": 29.09, "longitude": -110.9583333})
        };
        let history = json!({"plans": [{"id": "P1", "name": "Calles-45m-1104-1625"}],
            "captureLog": [record("2026-11-04 16:30:00", "DJI_0000.JPG", "C0"), record("2026-11-04 16:32:13", "DJI_0001.JPG", "C1")],
            "nuun": {"sources": {"x": {"kind": "all", "exportedAt": "2026-11-05T05:00:00Z"}}}});
        std::fs::write(area.join("history.json"), history.to_string()).unwrap();
        let plan = plan(&s, json!({})).unwrap();
        let unit = media_unit(&plan);
        assert_eq!(unit["project"], "subaereo");
        assert_eq!(unit["mission"]["captureID"], "C1");
        let session = "2026/11/26NOV04-163000_CALLES-45M-1104-1625/";
        assert_eq!(
            plan["layout"]["DCIM/101MEDIA/DJI_0001.JPG"],
            json!({"archive": format!("dron/misiones/{session}26NOV04-163212_MAVICAIR2_MX-SUBAEREO[].JPG"),
                   "work": format!("atlas-subaereo/misiones/{session}26NOV04-163212_MAVICAIR2_MX-SUBAEREO[].JPG")})
        );
        assert!(plan["layout"]["DCIM/101MEDIA/DJI_0001.DNG"]["work"].is_null());
        assert_eq!(
            plan["missions"]["subaereo"]["units"],
            json!({"matched": 1, "none": 0, "pending_export": 0})
        );
        assert!(plan["missions"]["subaereo"]["history_sha256"].is_string());
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn insta360_lenses_proxy_and_raw_photo_keep_their_original_neighbors() {
        let s = setup();
        let dir = "DCIM/Camera01";
        let names = [
            "VID_20261104_163212_00_001.insv",
            "VID_20261104_163212_10_001.insv",
            "LRV_20261104_163212_11_001.lrv",
            "IMG_20261104_164100_00_002.insp",
            "IMG_20261104_164100_00_002.dng",
        ];
        std::fs::create_dir_all(s.card.join(dir)).unwrap();
        for name in names {
            std::fs::write(s.card.join(dir).join(name), b"synthetic capture").unwrap();
        }
        let archive_only = plan(
            &s,
            json!({"work": null, "archive_only": true, "confirm_archive_only": true, "work_decision": "skipped"}),
        )
        .unwrap();
        assert!(archive_only["work_id"].is_null());
        assert!(
            archive_only["units"]
                .as_array()
                .unwrap()
                .iter()
                .all(|unit| unit["work_selected"] == false)
        );
        let selected = format!("{dir}/{}", names[0]);
        let plan = plan(&s, json!({"assignments": {selected: {"work": true}}})).unwrap();
        let units = plan["units"].as_array().unwrap();
        assert_eq!(units.len(), 2);
        assert!(units.iter().all(|u| u["camera"] == "insta360"
            && u["kind"] == "camera_bundle"
            && u["mission"].is_null()));
        let work_disk = json!({"role": "work", "prefix": "001_proyectos"});
        for (folder, names) in [("videos", &names[..3]), ("images", &names[3..])] {
            for (destination, prefix) in [
                ("archive", format!("360/{folder}/")),
                ("work", "e64_raw/intro/".to_owned()),
            ] {
                let mut expected_parent = None;
                for name in names {
                    let path = format!("{dir}/{name}");
                    let target = text(&plan["layout"][path.as_str()], destination);
                    let (parent, basename) = target.rsplit_once('/').unwrap();
                    assert!(parent.starts_with(&prefix));
                    assert_eq!(basename, *name);
                    assert_eq!(*expected_parent.get_or_insert(parent), parent);
                    let unit = units
                        .iter()
                        .find(|u| u["paths"].as_array().unwrap().contains(&json!(path)))
                        .unwrap();
                    assert_eq!(
                        destination_path(&work_disk, &plan, unit, &path, true)
                            .unwrap()
                            .is_some(),
                        folder == "videos"
                    );
                }
            }
        }
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn a_dji_360_clip_stays_out_of_the_drone_mission_flow() {
        let s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC3170");
        let clip = "DCIM/102MEDIA/DJI_0002.MP4";
        std::fs::create_dir_all(s.card.join("DCIM/102MEDIA")).unwrap();
        std::fs::write(s.card.join(clip), b"video fixture").unwrap();

        assert_eq!(
            plan(&s, json!({})).unwrap_err().code,
            "camera_review_required"
        );
        let plan = plan(&s, json!({"assignments": {clip: {"camera": "dji-360"}}})).unwrap();
        let units = plan["units"].as_array().unwrap();
        let drone = units.iter().find(|u| u["camera"] == "dji-drone").unwrap();
        let panorama = units.iter().find(|u| u["camera"] == "dji-360").unwrap();
        assert_eq!(drone["mission"]["state"], "pending_export");
        assert_eq!(plan["missions"]["subaereo"]["units"]["pending_export"], 1);
        assert!(panorama["mission"].is_null());
        assert_eq!(panorama["project"], "intro");
        assert!(text(&plan["layout"][clip], "archive").starts_with("360/videos/"));
        assert!(text(&plan["layout"][clip], "work").contains("/intro/"));
        validate_plan(&plan).unwrap();
    }

    #[test]
    fn dji_without_a_known_model_or_a_pair_from_two_devices_asks_first() {
        let s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC9999");
        let error = plan(&s, json!({})).unwrap_err();
        assert_eq!(error.code, "camera_review_required");
        let s = setup();
        let dir = s.card.join("DCIM/101MEDIA");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("DJI_0001.JPG"),
            sample::jpeg("DJI", "FC3170", "2026:11:04 16:32:12", None),
        )
        .unwrap();
        std::fs::write(
            dir.join("DJI_0001.DNG"),
            sample::tiff("DJI", "FC9999", "2026:11:04 16:32:12"),
        )
        .unwrap();
        assert!(
            plan(&s, json!({}))
                .unwrap_err()
                .message
                .starts_with("Classify DJI")
        );
        // An assignment decides it.
        let assigned = json!({"assignments": {"DCIM/101MEDIA/DJI_0001.JPG": {"camera": "dji-360"},
                                              "DCIM/101MEDIA/DJI_0001.DNG": {"camera": "dji-360"}}});
        let plan = plan(&s, assigned).unwrap();
        assert_eq!(media_unit(&plan)["camera"], "dji-360");
        assert!(
            media_unit(&plan).get("device").is_none(),
            "a conflict keeps no device"
        );
    }

    #[test]
    fn policy_checks_keep_pythons_codes() {
        let s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC3170");
        let code = |extra: Value| plan(&s, extra).unwrap_err().code;
        assert_eq!(code(json!({"archive_only": true})), "confirmation_required");
        assert_eq!(code(json!({"work": null})), "work_required");
        assert_eq!(code(json!({"work": "MEDIA"})), "invalid_role");
        assert_eq!(code(json!({"cleanup": "yes"})), "invalid_policy");
        assert_eq!(code(json!({"camera": "drone"})), "invalid_camera");
        assert_eq!(code(json!({"source_kind": "stalker"})), "empty_source");
        assert_eq!(code(json!({"frobnicate": 1})), "internal_error");
        assert_eq!(
            code(json!({"assignments": {"nope.JPG": {}}})),
            "invalid_assignment"
        );
        // Pending export with cleanup: the card is the only copy of those stills (A14). Cleanup also needs the
        // card root, which the fixture's mount is.
        assert_eq!(code(json!({"cleanup": true})), "mission_export_missing");
        // Archive-only with a work volume still expected.
        let only = plan(
            &s,
            json!({"archive_only": true, "confirm_archive_only": true}),
        )
        .unwrap();
        assert!(only["work_id"].is_null() && only["expected_work_id"].is_string());
        assert_eq!(only["work_delivery_status"], "pending");
        // A different file already at a planned name.
        let target = s.archive.join("001_raw/dron/fotos/2026/11");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join("26NOV04-163212_MAVICAIR2_MX-INTRO[].DNG"),
            b"other",
        )
        .unwrap();
        assert_eq!(code(json!({})), "destination_conflict");
    }

    #[test]
    fn same_disk_overlap_and_discards() {
        let mut s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC3170");
        std::fs::write(
            s.card.join("DCIM/101MEDIA/DJI_0002.JPG"),
            sample::jpeg("DJI", "FC3170", "2026:11:04 16:40:00", None),
        )
        .unwrap();
        let discard = json!({"assignments": {"DCIM/101MEDIA/DJI_0002.JPG": {"discard": true}}});
        let plan_value = plan(&s, discard).unwrap();
        assert_eq!(
            plan_value["discarded"][0]["paths"],
            json!(["DCIM/101MEDIA/DJI_0002.JPG"])
        );
        assert!(
            plan_value["layout"]
                .get("DCIM/101MEDIA/DJI_0002.JPG")
                .is_none()
        );
        let source = s.archive.join("inside");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("a.JPG"), b"x").unwrap();
        let p = json!({"source": source.display().to_string(), "archive": "MEDIA", "work": "ESSD", "project": "intro"});
        assert_eq!(
            build(&Catalog::open(&s.home).unwrap(), &s.backend, &p)
                .unwrap_err()
                .code,
            "overlapping_roots"
        );
        s.backend.physical.insert(
            crate::volumes::normalize(&s.work),
            crate::volumes::normalize(&s.archive),
        );
        assert_eq!(plan(&s, json!({})).unwrap_err().code, "same_physical_disk");
    }

    #[test]
    fn build_plan_remembers_the_reviewed_plan_under_the_writer_lock() {
        let s = setup();
        pair(&s, "DCIM/101MEDIA", "DJI_0001", "FC3170");
        let plan = build_plan(&s.home, &s.backend, &params(&s, json!({}))).unwrap();
        let reviewed = Catalog::open(&s.home)
            .unwrap()
            .get(&format!("reviewed-plan:{}", text(&plan, "id")))
            .unwrap()
            .unwrap();
        assert_eq!(
            reviewed,
            json!({"id": plan["id"], "digest": plan["digest"], "sessions": [], "cancelled": false,
                "mission_request": params(&s, json!({}))})
        );
        let _held = crate::catalog::writer_lock(&s.home).unwrap();
        assert_eq!(
            build_plan(&s.home, &s.backend, &params(&s, json!({})))
                .unwrap_err()
                .code,
            "job_busy"
        );
        // A changed plan no longer validates.
        let mut changed = plan.clone();
        changed["cleanup"] = json!(true);
        assert_eq!(validate_plan(&changed).unwrap_err().code, "plan_changed");
    }
}

/// Differential test of `plan` against the Python oracle (rust-engine.md §7), on the card trees of
/// `scripts/engine_golden.py`: same bytes, mtimes, source identity, stamps and host zone (`TZ`). Ignored by default.
#[cfg(test)]
mod golden {
    use super::*;
    use crate::volumes::test_backend::setup;

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    fn write_tree(root: &Path, tree: &Value, mtime_ns: Option<i64>) {
        for (rel, hex) in tree.as_object().into_iter().flatten() {
            let path = rel
                .split('/')
                .fold(root.to_path_buf(), |p, part| p.join(part));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, unhex(hex.as_str().unwrap())).unwrap();
            if let Some(ns) = mtime_ns {
                let when = std::time::UNIX_EPOCH + std::time::Duration::from_nanos(ns as u64);
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(when)
                    .unwrap();
            }
        }
    }

    /// The plan as text with temporary roots and random volume ids replaced, so two runs compare.
    fn normalized(plan: &Value, root: &str, ids: &[(&str, &str)]) -> Value {
        let mut text = plan
            .to_string()
            .replace(&root.replace('\\', "\\\\"), "<ROOT>");
        for (id, name) in ids {
            text = text.replace(id, &format!("<{name}>"));
        }
        let mut value: Value = serde_json::from_str(&text).unwrap();
        if let Some(map) = value.as_object_mut() {
            // Both hash the source path and the volume ids, which differ between the two runs.
            map.remove("id");
            map.remove("digest");
        }
        value
    }

    #[test]
    #[ignore = "needs tests/fixtures/engine-golden from scripts/engine_golden.py (Python, macOS, TZ set)"]
    fn plans_match_python_on_the_same_card_trees() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/engine-golden/recipe.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("run scripts/engine_golden.py"),
        )
        .unwrap();
        let cases = golden["plans"].as_array().expect("plan cases");
        assert!(!cases.is_empty());
        let zone = golden["plan_tz"].as_str().unwrap();
        assert_eq!(
            std::env::var("TZ").as_deref(),
            Ok(zone),
            "run with TZ={zone}: names use the host zone, as Python"
        );
        for case in cases {
            let mut s = setup();
            let root = s.card.parent().unwrap().display().to_string();
            write_tree(&s.card, &case["card"], case["mtime_ns"].as_i64());
            write_tree(&s.archive, &case["archive"], None);
            write_tree(&s.work, &case["work"], None);
            // The source volume answers with Python's identity, so unit ids hash the same values.
            s.backend.overrides.insert(
                crate::volumes::normalize(&s.card),
                case["source_identity"].clone(),
            );
            let mut params = case["params"].clone();
            params["source"] = json!(s.card.display().to_string());
            if params["source_kind"] == "stalker" {
                crate::catalog::WritableCatalog::open(&s.home)
                    .unwrap()
                    .set("retain-stalker-source", &json!(true))
                    .unwrap();
            }
            let discover = |source: &Path, archived_filter: bool| {
                let catalog = Catalog::open(&s.home).unwrap();
                let mut request = json!({"source":source});
                if archived_filter {
                    request["source_kind"] = json!("stalker");
                    request["archive"] = json!("MEDIA");
                }
                crate::scan::discover(Some(&catalog), Some(&s.backend), &request).unwrap()
            };
            if let Some(discovery) = case.get("discovery") {
                crate::catalog::WritableCatalog::open(&s.home)
                    .unwrap()
                    .set(
                        "capture-tag:d9391e8f-0ddd-4f03-b745-b8bbd15832d9",
                        &json!({"project":"intro","country":"mx"}),
                    )
                    .unwrap();
                assert_eq!(discover(&s.card, true), discovery["pending"]);
                assert_eq!(
                    discover(&s.card.join("S.stalker"), false),
                    discovery["bundle"]
                );
            }
            let catalog = Catalog::open(&s.home).unwrap();
            let mine = build_with_stamps(&catalog, &s.backend, &params, case["stamps"].as_object());
            let name = text(case, "name");
            match (&case["plan"]["error"], mine) {
                (Value::String(code), Err(e)) => assert_eq!(&e.code, code, "{name}"),
                (Value::String(code), Ok(_)) => {
                    panic!("{name}: Python failed with {code}, Rust planned")
                }
                (_, Err(e)) => panic!("{name}: Rust failed with {} ({})", e.code, e.message),
                (_, Ok(plan)) => {
                    let theirs = &case["plan"];
                    let mut raw = theirs.clone();
                    raw.as_object_mut().unwrap().remove("digest");
                    assert_eq!(
                        digest(&raw),
                        text(theirs, "digest"),
                        "{name}: canonical JSON of Python's plan"
                    );
                    let python_ids = [
                        (text(&case["volume_ids"], "MEDIA"), "MEDIA"),
                        (text(&case["volume_ids"], "ESSD"), "ESSD"),
                    ];
                    let rust_ids = [("id-MEDIA", "MEDIA"), ("id-ESSD", "ESSD")];
                    // Rust's plan also carries Python's root: the source identity above is Python's.
                    let mine = normalized(&plan, &root, &rust_ids);
                    assert_eq!(
                        normalized(&mine, text(case, "root"), &[]),
                        normalized(theirs, text(case, "root"), &python_ids),
                        "{name}"
                    );
                    // The same plan applied: final state and verified copies (destination, SHA-256) as Python's.
                    // Applied from a plan with this run's own stamps: Python's (inode, ctime) are
                    // there only so the ids hash the same, and apply rightly sees them as a changed source.
                    // Unit ids hash the stamps and name support folders, so copies map them back to
                    // the ids of the plan just compared with Python's.
                    let expected = &case["apply"];
                    let stamped = plan;
                    let plan = build_with_stamps(&catalog, &s.backend, &params, None).unwrap();
                    let unit_ids = |p: &Value| -> Vec<String> {
                        p["units"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|u| text(u, "id").to_owned())
                            .collect()
                    };
                    let id_map: Vec<(String, String)> = unit_ids(&plan)
                        .into_iter()
                        .zip(unit_ids(&stamped))
                        .collect();
                    let digest_text = text(&plan, "digest").to_owned();
                    let applied = {
                        let _lock = crate::catalog::writer_lock(&s.home).unwrap();
                        let writable = crate::catalog::WritableCatalog::open(&s.home).unwrap();
                        let no_hook = |_: &str, _: &Value| Ok(());
                        crate::transfer::Transfer::new(&s.home, &writable, &s.backend, &no_hook)
                            .apply(&plan, &digest_text)
                            .unwrap()
                    };
                    assert_eq!(applied["state"], expected["state"], "{name}");
                    if let Some(discovery) = case.get("discovery") {
                        assert_eq!(discover(&s.card, true), discovery["archived"]);
                    }
                    let names = |id: &str| if id == "id-MEDIA" { "MEDIA" } else { "ESSD" };
                    let mut copies: Vec<Value> = Catalog::open(&s.home)
                        .unwrap()
                        .json_rows("SELECT payload FROM copies", [])
                        .unwrap()
                        .into_iter()
                        .map(|c| {
                            let mut destination = text(&c, "destination").to_owned();
                            for (own, python) in &id_map {
                                destination = destination.replace(own.as_str(), python);
                            }
                            json!([names(text(&c, "volume_id")), destination, c["sha256"]])
                        })
                        .collect();
                    copies.sort_by_key(|c| c.to_string());
                    let mut theirs: Vec<Value> =
                        expected["copies"].as_array().cloned().unwrap_or_default();
                    theirs.sort_by_key(|c| c.to_string());
                    assert_eq!(copies, theirs, "{name}: verified copies");
                }
            }
        }
    }
}
