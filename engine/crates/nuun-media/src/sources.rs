//! Python `library.sources` (with `receipt_metadata`, `present` and `layout.parse_name`): a project's editable
//! sources in capture order, one call, for an editor's quick import. Read only: one `lstat` per recorded copy on
//! an online disk, never a hash.

use crate::catalog::Catalog;
use crate::reads;
use crate::snapshot::kind_of;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, NAMESPACE_URL, Result, require, uuid5};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;

const MEDIA_KINDS: [&str; 4] = ["video", "audio", "image", "raw_image"];
const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// layout.py `parse_name`: capture start (local wall clock) and duration from a dated name, or `None`.
pub fn parse_name(name: &str) -> Option<Value> {
    let pattern = regex::Regex::new(
        r"^(\d{2})([A-Z]{3})(\d{2})-(\d{2})(\d{2})(\d{2})_[^_]+(?:_(\d+)-(\d{2}))?_",
    )
    .ok()?;
    let g = pattern.captures(name)?;
    let month = MONTHS.iter().position(|m| *m == &g[2])? as u32 + 1;
    let n = |i: usize| g[i].parse::<u32>().ok();
    let day = chrono_like(2000 + n(1)?, month, n(3)?, n(4)?, n(5)?, n(6)?)?;
    let duration = match (g.get(7), g.get(8)) {
        (Some(m), Some(s)) => {
            json!(m.as_str().parse::<u64>().ok()? * 60 + s.as_str().parse::<u64>().ok()?)
        }
        _ => Value::Null,
    };
    Some(json!({"captured_at": day, "duration_seconds": duration}))
}

/// `datetime(y, m, d, H, M, S).isoformat()`, or `None` for a date that does not exist.
fn chrono_like(
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<String> {
    let text = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}");
    nu9ve_time::DateTime::fromisoformat(&text).map(|d| d.isoformat())
}

/// library.py `present`: the recorded copy is still there at its size (hashes stay with verify and recovery).
fn present(root: &str, copy: &Value) -> bool {
    std::fs::symlink_metadata(
        Path::new(root).join(copy["destination"].as_str().unwrap_or_default()),
    )
    .is_ok_and(|m| Some(m.len()) == copy["bytes"].as_u64())
}

/// evidence.py `capture_time()` with no value: an unknown civil time.
fn unknown_time() -> Value {
    json!({"original": null, "provenance": null, "originals": {}, "captured_at_absolute": null, "time_zone": null,
           "offset_seconds": null})
}

fn clipper_profile(catalog: &Catalog, profile: &str) -> Result<Value> {
    Ok(catalog
        .get("creative-profiles")?
        .map(|p| p[profile]["clipper_profile"].clone())
        .unwrap_or(Value::Null))
}

/// One volume's copies of an asset, by source path, in row order.
type Rows = Vec<(String, Value)>;

fn row<'a>(rows: &'a Rows, path: &str) -> Option<&'a Value> {
    rows.iter().find(|(p, _)| p == path).map(|(_, v)| v)
}

/// library.py `receipt_metadata`: stored receipts only.
pub(crate) fn receipt_metadata(
    catalog: &Catalog,
    asset: &Value,
    copies: &[(String, Rows)],
) -> Result<Map<String, Value>> {
    let mut rows: HashMap<String, Value> = HashMap::new();
    for (_, group) in copies {
        for (path, value) in group {
            rows.insert(path.clone(), value.clone());
        }
    }
    let bundle = asset["bundle"].as_str();
    let prefix = match bundle {
        None | Some(".") => String::new(),
        Some(b) => format!("{b}/"),
    };
    let mut editable: Vec<&str> = asset["paths"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|p| kind_of(p).is_some_and(|k| MEDIA_KINDS.contains(&k)))
        .collect();
    editable.sort();
    let members: Vec<Value> = editable
        .iter()
        .filter_map(|p| {
            rows.get(*p)
                .map(|r| json!({"path": p, "bytes": r["bytes"], "sha256": r["sha256"]}))
        })
        .collect();
    let media_bytes: u64 = members.iter().filter_map(|m| m["bytes"].as_u64()).sum();
    let manifest = if asset["kind"] == "stalker_session" {
        rows.get(&format!("{prefix}manifest.json"))
            .map(|r| r["sha256"].clone())
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let capture_time = match asset.get("capture_time") {
        Some(t) if crate::volumes::truthy(t) => t.clone(),
        _ => unknown_time(),
    };
    let mut out = Map::new();
    out.insert(
        "content_revision".into(),
        json!(uuid5(
            &NAMESPACE_URL,
            asset["revision"].as_str().unwrap_or_default()
        )),
    );
    out.insert("manifest_sha256".into(), manifest);
    out.insert("media_bytes".into(), json!(media_bytes));
    out.insert("editable_members".into(), json!(members));
    out.insert("capture_time".into(), capture_time);
    out.insert(
        "clipper_profile".into(),
        clipper_profile(catalog, asset["profile"].as_str().unwrap_or_default())?,
    );
    Ok(out)
}

/// The `sources` method.
pub fn sources(catalog: &Catalog, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let project = params
        .get("project")
        .ok_or_else(|| Error::new("operation_failed", "'project'"))?;
    let purpose = match params.get("purpose") {
        Some(Value::String(p)) if !p.is_empty() => p.as_str(),
        _ => "edit",
    };
    require(
        purpose == "edit" || purpose == "archive",
        "invalid_purpose",
        "Purpose must be edit or archive",
    )?;
    let assets: Vec<Value> = reads::library(catalog, &json!({"project": project}))?
        .into_iter()
        .filter(|a| {
            ["stalker_session", "camera_bundle", "file", "delivery"]
                .contains(&a["kind"].as_str().unwrap_or_default())
        })
        .collect();
    if assets.is_empty() {
        let shown = match project {
            Value::String(s) => s.clone(),
            Value::Null => "None".into(),
            other => other.to_string(),
        };
        return Err(Error::new(
            "unknown_project",
            format!("No ingested assets for project {shown}"),
        ));
    }
    let cached = volumes::Cached::new(backend);
    volumes::prefetch(&cached, catalog, false)?;
    let mut volumes_by_id: HashMap<String, Value> = HashMap::new();
    for mut volume in catalog.volumes()? {
        volume["online"] = json!(volumes::online(&cached, &volume).is_some());
        volumes_by_id.insert(volume["id"].as_str().unwrap_or_default().to_owned(), volume);
    }
    let preference: [&str; 2] = if purpose == "edit" {
        ["work", "archive"]
    } else {
        ["archive", "work"]
    };
    let (mut result, mut offline, mut removed, mut warnings) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for asset in &assets {
        let job_id = asset["job_id"].as_str().unwrap_or_default();
        let paths: Vec<&str> = asset["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        // `{volume_id: {path: copy}}` in row order.
        let mut copies: Vec<(String, Rows)> = Vec::new();
        for copy in catalog.json_rows("SELECT payload FROM copies WHERE job_id=?", [job_id])? {
            let path = copy["path"].as_str().unwrap_or_default().to_owned();
            if !paths.contains(&path.as_str()) {
                continue;
            }
            let volume = copy["volume_id"].as_str().unwrap_or_default().to_owned();
            match copies.iter_mut().find(|(v, _)| *v == volume) {
                Some((_, rows)) => match rows.iter_mut().find(|(p, _)| *p == path) {
                    Some(slot) => slot.1 = copy,
                    None => rows.push((path, copy)),
                },
                None => copies.push((volume, vec![(path, copy)])),
            }
        }
        let stalker = asset["kind"] == "stalker_session";
        let prefix = if asset["bundle"] == "." {
            String::new()
        } else {
            format!("{}/", asset["bundle"].as_str().unwrap_or_default())
        };
        let primary: Vec<String> = if stalker {
            vec![format!("{prefix}manifest.json")]
        } else {
            let mut media: Vec<String> = paths
                .iter()
                .filter(|p| kind_of(p).is_some_and(|k| MEDIA_KINDS.contains(&k)))
                .map(|p| (*p).to_owned())
                .collect();
            media.sort();
            media
        };
        if primary.is_empty() {
            continue; // Camera indexes/thumbnails are never editor sources.
        }
        let required: Vec<String> = if stalker || asset["kind"] == "delivery" {
            paths.iter().map(|p| (*p).to_owned()).collect()
        } else {
            primary.clone()
        };
        let mut candidates: Vec<(bool, usize, Value, String, Value)> = Vec::new();
        let mut missing: Vec<Value> = Vec::new();
        for (volume_id, rows) in &copies {
            if !required.iter().all(|p| row(rows, p).is_some()) {
                continue;
            }
            let Some(volume) = volumes_by_id.get(volume_id) else {
                return Err(Error::new("operation_failed", format!("'{volume_id}'")));
            };
            let Some(sample) = row(rows, &primary[0]).cloned() else {
                continue;
            };
            let destination = sample["destination"].as_str().unwrap_or_default();
            let relative_path = if stalker {
                let inner = sample["path"]
                    .as_str()
                    .unwrap_or_default()
                    .get(prefix.len()..)
                    .unwrap_or_default();
                destination
                    .get(..destination.len().saturating_sub(inner.len() + 1))
                    .unwrap_or_default()
                    .to_owned()
            } else {
                destination.to_owned()
            };
            let root = volume["root"].as_str().unwrap_or_default();
            let receipt = match sample.get("receipt_id") {
                Some(r) if crate::volumes::truthy(r) => r.clone(),
                _ => json!(uuid5(&NAMESPACE_URL, &format!("{job_id}:{volume_id}"))),
            };
            let online = volume["online"] == true;
            let location = json!({"volume_id": volume_id, "receipt_id": receipt, "volume": volume["name"],
                "role": volume["role"], "online": online,
                "path": Path::new(root).join(&relative_path).display().to_string()});
            if online && !rows.iter().all(|(_, r)| present(root, r)) {
                // Deleted or edited by hand: report it, never fail or resurrect it.
                missing.push(location);
                continue;
            }
            let role = volume["role"].as_str().unwrap_or_default();
            let rank = preference.iter().position(|p| *p == role).unwrap_or(9);
            candidates.push((!online, rank, location, relative_path, sample));
        }
        if candidates.is_empty() && missing.is_empty() {
            continue;
        }
        candidates.sort_by_key(|c| (c.0, c.1));
        let shown = match candidates.first() {
            Some(c) => c.3.clone(),
            None => missing[0]["path"].as_str().unwrap_or_default().to_owned(),
        };
        let name = shown.rsplit('/').next().unwrap_or_default().to_owned();
        let job = reads::job(catalog, &json!({"job_id": job_id}))?;
        let bytes: u64 = job["plan"]["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|f| paths.contains(&f["path"].as_str().unwrap_or_default()))
            .filter_map(|f| f["stamp"]["size"].as_u64())
            .sum();
        let sha = match candidates.first() {
            Some(c) if !stalker => c.4["sha256"].clone(),
            _ => Value::Null,
        };
        let mut entry = json!({"asset_id": asset["id"], "revision": asset["revision"], "kind": asset["kind"],
            "session_id": asset.get("session_id").cloned().unwrap_or(Value::Null), "name": name, "bytes": bytes,
            "sha256": sha, "alternatives": candidates.iter().skip(1).map(|c| c.2.clone()).collect::<Vec<_>>(),
            "missing": missing});
        let parsed =
            parse_name(&name).unwrap_or(json!({"captured_at": null, "duration_seconds": null}));
        for (k, v) in parsed.as_object().into_iter().flatten() {
            entry[k.as_str()] = v.clone();
        }
        for (k, v) in receipt_metadata(catalog, asset, &copies)? {
            entry[k.as_str()] = v;
        }
        if let Some(first) = candidates.first() {
            let volume_id = first.2["volume_id"].as_str().unwrap_or_default();
            let selected = &copies
                .iter()
                .find(|(v, _)| v == volume_id)
                .expect("candidate volume")
                .1;
            let root = Path::new(
                volumes_by_id[volume_id]["root"]
                    .as_str()
                    .unwrap_or_default(),
            );
            let members: Vec<Value> = entry["editable_members"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| {
                    let r = row(selected, m["path"].as_str().unwrap_or_default())?;
                    let mut m = m.clone();
                    m["local_path"] = json!(
                        root.join(r["destination"].as_str().unwrap_or_default())
                            .display()
                            .to_string()
                    );
                    Some(m)
                })
                .collect();
            entry["editable_members"] = json!(members);
            if asset["kind"] == "delivery" {
                let media: Vec<Value> = paths
                    .iter()
                    .filter_map(|p| {
                        let r = row(selected, p)?;
                        Some(json!({"original_path": p,
                            "path": root.join(r["destination"].as_str().unwrap_or_default()).display().to_string(),
                            "bytes": r["bytes"], "sha256": r["sha256"]}))
                    })
                    .collect();
                entry["media_members"] = json!(media);
                entry
                    .as_object_mut()
                    .expect("entry")
                    .remove("editable_members");
            }
            for key in [
                "path",
                "volume",
                "volume_id",
                "receipt_id",
                "role",
                "online",
            ] {
                entry[key] = first.2[key].clone();
            }
        }
        let missing = entry["missing"].as_array().cloned().unwrap_or_default();
        if missing.iter().any(|m| m["role"] == "archive") {
            warnings.push(json!({"asset_id": asset["id"], "name": entry["name"], "problem": "archive_copy_missing",
                                 "locations": missing}));
        }
        if purpose == "edit" && missing.iter().any(|m| m["role"] == "work") {
            removed.push(entry); // Taken out of the working set; it stays available from the archive.
        } else if candidates.first().is_some_and(|c| c.2["online"] == true) {
            result.push(entry);
        } else {
            offline.push(entry);
        }
    }
    let by_name = |e: &Value| e["name"].as_str().unwrap_or_default().to_owned();
    result.sort_by_key(by_name);
    offline.sort_by_key(by_name);
    removed.sort_by_key(by_name);
    for (order, entry) in result.iter_mut().enumerate() {
        entry["order"] = json!(order);
    }
    let first = &assets[0];
    Ok(
        json!({"project": first["project"], "project_id": first["project_id"], "profile": first["profile"],
              "clipper_profile": clipper_profile(catalog, first["profile"].as_str().unwrap_or_default())?,
              "purpose": purpose, "sources": result, "offline": offline, "removed_from_work": removed,
              "warnings": warnings}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dated_names_follow_layout_py() {
        assert_eq!(
            parse_name("26NOV04-163212_MAVICAIR2_05-09_MX-INTRO[].MP4").unwrap(),
            json!({"captured_at": "2026-11-04T16:32:12", "duration_seconds": 309})
        );
        assert_eq!(
            parse_name("26NOV04-163212_MAVICAIR2_MX-INTRO[].JPG").unwrap()["duration_seconds"],
            Value::Null
        );
        assert!(parse_name("26XYZ04-163212_A_").is_none());
        assert!(parse_name("IMG_0001.JPG").is_none());
    }
}
