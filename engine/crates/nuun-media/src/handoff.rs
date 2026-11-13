//! Portable handoff assembled exclusively from the catalog's verified receipts.

use crate::catalog::Catalog;
use crate::layout::MEDIA_KINDS;
use crate::planner::destination_path;
use crate::reads;
use crate::snapshot::kind_of;
use nu9ve_json::{Error, NAMESPACE_URL, Result, require, uid, uuid5};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn editable(path: &str) -> bool {
    kind_of(path).is_some_and(|kind| MEDIA_KINDS.contains(&kind))
}

fn location(
    volume: &Value,
    plan: &Value,
    asset: &Value,
    copies: &BTreeMap<String, Value>,
) -> Result<Option<Value>> {
    let expected = asset["paths"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|path| {
            destination_path(volume, plan, asset, path, false).map(|dest| dest.map(|_| path))
        })
        .collect::<Result<Vec<_>>>()?;
    let expected: BTreeSet<_> = expected.into_iter().flatten().collect();
    if expected.is_empty() || expected != copies.keys().map(String::as_str).collect() {
        return Ok(None);
    }
    let kind = text(asset, "kind");
    let stalker = kind == "stalker_session";
    let mut selected: Vec<_> = copies
        .values()
        .filter(|copy| stalker || kind == "delivery" || editable(text(copy, "path")))
        .collect();
    if selected.is_empty() {
        selected.extend(copies.values());
    }
    if kind == "delivery" {
        selected.sort_by_key(|copy| (!editable(text(copy, "path")), text(copy, "path")));
    }
    let sample = selected[0];
    let dest = text(sample, "destination");
    let location_path = if stalker {
        let bundle = text(asset, "bundle");
        let inner = if bundle == "." {
            text(sample, "path")
        } else {
            text(sample, "path")
                .strip_prefix(bundle)
                .and_then(|path| path.strip_prefix('/'))
                .ok_or_else(|| Error::new("missing_receipt", "Stalker member outside its bundle"))?
        };
        dest.strip_suffix(inner)
            .and_then(|parent| parent.strip_suffix('/'))
            .ok_or_else(|| {
                Error::new(
                    "missing_receipt",
                    "Stalker receipt did not preserve its bundle paths",
                )
            })?
    } else {
        dest
    };
    let members = selected
        .into_iter()
        .map(|copy| {
            let dest = text(copy, "destination");
            let path = if stalker {
                dest.strip_prefix(location_path)
                    .and_then(|path| path.strip_prefix('/'))
                    .ok_or_else(|| {
                        Error::new("missing_receipt", "Stalker destination outside its bundle")
                    })?
            } else {
                dest.rsplit('/').next().unwrap_or_default()
            };
            Ok(
                json!({"relative_path": path, "original_path": copy["path"], "bytes": copy["bytes"],
            "sha256": copy["sha256"], "volume_relative_path": dest}),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let volume_id = text(volume, "id");
    let receipt = sample["receipt_id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid5(&NAMESPACE_URL, &format!("{}:{volume_id}", text(plan, "id"))));
    Ok(Some(
        json!({"volume_id": volume_id, "relative_path": location_path,
        "path_kind": if stalker {"directory"} else {"file"}, "role": volume["role"],
        "members": members, "receipt_id": receipt}),
    ))
}

pub fn handoff(catalog: &Catalog, params: &Value) -> Result<Value> {
    let ids = params
        .get("asset_ids")
        .ok_or_else(|| Error::new("operation_failed", "'asset_ids'"))?;
    let ids = ids
        .as_array()
        .filter(|ids| !ids.is_empty() && ids.iter().all(Value::is_string))
        .ok_or_else(|| Error::new("invalid_selection", "Select distinct assets"))?;
    require(
        ids.iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            == ids.len(),
        "invalid_selection",
        "Select distinct assets",
    )?;
    let assets = ids
        .iter()
        .map(|id| {
            catalog
                .json_rows(
                    "SELECT payload FROM assets WHERE id=?",
                    [id.as_str().unwrap_or_default()],
                )?
                .into_iter()
                .next()
                .ok_or_else(|| Error::new("unknown_asset", "Asset not found"))
        })
        .collect::<Result<Vec<_>>>()?;
    let project = &assets[0]["project_id"];
    let profile = &assets[0]["profile"];
    require(
        assets
            .iter()
            .all(|asset| asset["project_id"] == *project && asset["profile"] == *profile),
        "mixed_project",
        "Handoff must be one project/profile",
    )?;
    let volumes = catalog.volumes()?;
    let mut items = Vec::new();
    for (order, asset) in assets.iter().enumerate() {
        let job = reads::job(catalog, &json!({"job_id": asset["job_id"]}))?;
        let plan = &job["plan"];
        let paths = asset["paths"]
            .as_array()
            .ok_or_else(|| Error::new("missing_receipt", "Asset paths missing"))?;
        let mut locations = Vec::new();
        let mut groups = Vec::new();
        for volume in &volumes {
            if !params["volume_id"].is_null() && params["volume_id"] != volume["id"] {
                continue;
            }
            let id = text(volume, "id");
            let copies: BTreeMap<_, _> = catalog
                .json_rows(
                    "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
                    [text(plan, "id"), id],
                )?
                .into_iter()
                .filter(|copy| paths.contains(&copy["path"]))
                .map(|copy| (text(&copy, "path").to_owned(), copy))
                .collect();
            if let Some(location) = location(volume, plan, asset, &copies)? {
                locations.push(location);
            }
            groups.push((id.to_owned(), copies.into_iter().collect()));
        }
        require(
            !locations.is_empty(),
            "missing_receipt",
            "No complete verified location",
        )?;
        let mut seen = BTreeMap::new();
        for member in locations
            .iter()
            .flat_map(|loc| loc["members"].as_array().into_iter().flatten())
        {
            let identity = (&member["bytes"], &member["sha256"]);
            require(
                seen.insert(text(member, "original_path"), identity)
                    .is_none_or(|prior| prior == identity),
                "revision_mismatch",
                "Replica content differs",
            )?;
        }
        let bytes: u64 = plan["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|file| paths.contains(&file["path"]))
            .filter_map(|file| file["stamp"]["size"].as_u64())
            .sum();
        let mut item = json!({"asset_id": asset["id"], "revision": asset["revision"], "kind": asset["kind"],
            "order": order, "members": locations[0]["members"], "locations": locations, "bytes": bytes});
        item.as_object_mut()
            .unwrap()
            .extend(crate::sources::receipt_metadata(catalog, asset, &groups)?);
        if let Some(session) = asset["session_id"].as_str().filter(|id| !id.is_empty()) {
            item["capture_session_id"] = json!(crate::captures::python_uuid(session).ok_or_else(
                || Error::new("operation_failed", "badly formed hexadecimal UUID string")
            )?);
        }
        items.push(item);
    }
    Ok(
        json!({"schema": "creator-media-handoff/1", "handoff_id": uid()?,
        "created_at": nu9ve_time::now_utc(), "project_id": project, "profile_id": profile,
        "clipper_profile": items[0]["clipper_profile"], "assets": items}),
    )
}
