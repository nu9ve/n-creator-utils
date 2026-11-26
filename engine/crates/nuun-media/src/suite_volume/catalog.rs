//! What nüün serves to the suite on its registered transport: `data_catalog` (nu9ve-data-v1) and `data_export`
//! (nu9ve-export-v1). Both read only the local catalog and the last index this machine published of each disk
//! (`suite-volumes/<volume_id>-index.json`): no hashes, no file contents, no walk of a disk.
//!
//! Privacy (data §6, export §4): the catalog is described in full, and only on the local transport; the agent
//! publishes the personal datasets with their shape only unless the person shared them. `data_export` never gives
//! bytes: nüün's datasets are media folders on disks, so it answers `too_large` with the disk and the path, as the
//! contract asks for raws and photos (export §0).
use super::{document, marker};
use crate::catalog::{Catalog, sql};
use crate::tree::Root;
use nu9ve_json::{Error, Result, require};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) const LOCAL: &str = "suite-volumes";
const MAX_SAFE: u64 = (1 << 53) - 1;
/// A page must fit in one frame (1 MiB, engine §6.1), with room for the envelope.
const PAGE_BYTES: usize = 900 * 1024;

/// Where the last published index of a disk is kept on this machine.
pub(crate) fn local_index(volume_id: &str) -> String {
    format!("{LOCAL}/{volume_id}-index.json")
}

fn key_ok(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn dataset_id_ok(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=128).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._:-".contains(c))
}

fn invalid(message: &str) -> Error {
    Error::new("invalid_params", message)
}

struct Sources {
    projects: BTreeMap<String, Value>,
    roots: BTreeMap<String, Value>,
    /// Sorted by id, which is unique: disk datasets carry their volume in the id.
    datasets: BTreeMap<String, Value>,
}

/// Every nüün project as a suite project: the local id, lowercased, is its key until a suite key is chosen.
fn projects(catalog: &Catalog) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    let Some(db) = catalog.db() else {
        return Ok(out);
    };
    let mut query = db
        .prepare(
            "SELECT p.id, p.title, m.value FROM projects p
             LEFT JOIN metadata m ON m.key='project-state:' || p.id ORDER BY p.id",
        )
        .map_err(sql)?;
    let mut rows = query.query([]).map_err(sql)?;
    while let Some(row) = rows.next().map_err(sql)? {
        let (id, title, state): (String, String, Option<String>) = (
            row.get(0).map_err(sql)?,
            row.get(1).map_err(sql)?,
            row.get(2).map_err(sql)?,
        );
        let key = id.to_ascii_lowercase();
        if !key_ok(&key) {
            continue;
        }
        let title: String = title
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        out.insert(key.clone(), json!({
            "project_key": key, "title": if title.is_empty() { "Proyecto".to_owned() } else { title },
            "parent_key": null,
            "state": if state.as_deref() == Some("\"archived\"") { "archived" } else { "active" }}));
    }
    Ok(out)
}

/// The `volume_id`s whose disk is mounted here now, by the marker at the root of each registered destination.
fn mounted(catalog: &Catalog) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for volume in catalog.volumes()? {
        let Some(root) = volume["root"].as_str() else {
            continue;
        };
        if let Ok(root) = Root::open(Path::new(root))
            && let Ok(Some(found)) = marker(&root)
            && let Some(id) = found["volume_id"].as_str()
        {
            ids.insert(id.to_owned());
        }
    }
    Ok(ids)
}

fn sources(home: &Path) -> Result<Sources> {
    let catalog = Catalog::open(home)?;
    let projects = projects(&catalog)?;
    let here = mounted(&catalog)?;
    let mut roots = BTreeMap::new();
    let mut datasets = BTreeMap::new();
    // Disks: the last index this machine published of each one.
    let mut names: Vec<String> = match std::fs::read_dir(home.join(LOCAL)) {
        Ok(entries) => entries
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with("-index.json"))
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    let local = if names.is_empty() {
        None
    } else {
        Some(Root::open(home)?)
    };
    for name in names {
        let Some(index) = local
            .as_ref()
            .and_then(|r| {
                document(
                    r,
                    &format!("{LOCAL}/{name}"),
                    super::index::LIMIT,
                    "invalid_index",
                )
                .ok()
            })
            .flatten()
        else {
            continue;
        };
        let Some(disk) = index["roots"].as_array().and_then(|r| r.first()) else {
            continue;
        };
        let Some(volume_id) = disk["volume_id"]
            .as_str()
            .filter(|id| name == format!("{id}-index.json"))
        else {
            continue;
        };
        let root_id = format!("vol-{volume_id}");
        roots.insert(
            root_id.clone(),
            json!({
            "id": root_id, "label": disk["label"], "volume_id": volume_id, "volume_path": "",
            "state": if here.contains(volume_id) { "ok" } else { "missing" }}),
        );
        for dataset in index["datasets"].as_array().into_iter().flatten() {
            let (Some(id), Some(mut dataset)) =
                (dataset["id"].as_str(), dataset.as_object().cloned())
            else {
                continue;
            };
            if !dataset_id_ok(id) || dataset.get("owner") != Some(&json!("nuun")) {
                continue;
            }
            if let Some(location) = dataset.get_mut("location").and_then(Value::as_object_mut) {
                location.insert("root_id".into(), json!(root_id));
            }
            if !here.contains(volume_id) {
                dataset.insert("state".into(), json!("missing"));
            }
            dataset.insert("nuun_volume".into(), json!(volume_id));
            datasets.insert(id.to_owned(), Value::Object(dataset));
        }
    }
    // Living collections (backups of folders that keep changing): the source on this machine.
    for collection in crate::reads::living_collections(&catalog)?
        .as_array()
        .into_iter()
        .flatten()
    {
        let (Some(cid), Some(source)) = (collection["id"].as_str(), collection["source"].as_str())
        else {
            continue;
        };
        let id = format!("living:{cid}");
        if !dataset_id_ok(&id) {
            continue;
        }
        let root_id = format!("living-{}", &cid[..cid.len().min(36)]);
        let present = Path::new(source).is_dir();
        let title: String = collection["name"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        roots.insert(
            root_id.clone(),
            json!({
            "id": root_id, "label": if title.is_empty() { "Colección" } else { title.as_str() },
            "path": source, "state": if present { "ok" } else { "missing" }}),
        );
        let mut dataset = json!({
            "id": id, "owner": "nuun", "title": if title.is_empty() { "Colección".to_owned() } else { title },
            "modality": "files", "privacy": "personal", "storage": "in_place",
            "location": {"root_id": root_id, "path": ""}, "projects": [],
            "state": if present { "unverified" } else { "missing" },
            "updated_at": collection["updated_at"].as_str().filter(|t| crate::suite_device::time(&json!(t)))
                .map_or_else(nu9ve_time::now_utc, str::to_owned)});
        if let (Some(files), Some(bytes)) =
            (collection["files"].as_u64(), collection["bytes"].as_u64())
        {
            dataset["size"] =
                json!({"bytes": bytes.min(MAX_SAFE), "items": files, "unit": "files"});
        }
        datasets.insert(dataset["id"].as_str().unwrap().to_owned(), dataset);
    }
    // Accumulations on disk (builds, caches, backups): the newest scan as a personal table, one row per finding.
    if let Some(dataset) = crate::accumulations::dataset(home) {
        datasets.insert(crate::accumulations::DATASET.to_owned(), dataset);
    }
    Ok(Sources {
        projects,
        roots,
        datasets,
    })
}

fn params_of(params: &Value, allowed: &[&str]) -> Result<Map<String, Value>> {
    let object = match params {
        Value::Null => Map::new(),
        Value::Object(o) => o.clone(),
        _ => return Err(invalid("params must be an object")),
    };
    require(
        object.keys().all(|k| allowed.contains(&k.as_str())),
        "invalid_params",
        format!("Only {} are accepted", allowed.join(", ")),
    )?;
    Ok(object)
}

/// `data_catalog` (nu9ve-data-v1 §1–§4): pages of datasets sorted by id, each page within one frame.
pub fn data_catalog(home: &Path, params: &Value) -> Result<Value> {
    let p = params_of(params, &["project_key", "modality", "limit", "cursor"])?;
    let key = match p.get("project_key") {
        None => None,
        Some(Value::String(k)) if key_ok(k) => Some(k.clone()),
        Some(_) => return Err(invalid("project_key must match ^[a-z0-9-]{1,64}$")),
    };
    let modality = match p.get("modality") {
        None => None,
        Some(Value::String(m)) if !m.is_empty() && m.len() <= 64 => Some(m.clone()),
        Some(_) => return Err(invalid("modality must be a short string")),
    };
    let limit = match p.get("limit") {
        None => 200,
        Some(v) => v
            .as_u64()
            .filter(|n| (1..=500).contains(n))
            .ok_or_else(|| invalid("limit must be an integer from 1 to 500"))?
            as usize,
    };
    let after = match p.get("cursor") {
        None => None,
        Some(Value::String(c)) if (1..=256).contains(&c.len()) => Some(
            c.strip_prefix("after:")
                .filter(|id| dataset_id_ok(id))
                .ok_or_else(|| Error::new("invalid_cursor", "Start again without a cursor"))?
                .to_owned(),
        ),
        Some(_) => return Err(invalid("cursor must be a string of 1 to 256 bytes")),
    };
    let sources = sources(home)?;
    let wanted = |d: &Value| {
        key.as_ref().is_none_or(|k| {
            d["projects"]
                .as_array()
                .is_some_and(|ps| ps.iter().any(|p| p["project_key"] == **k))
        }) && modality.as_ref().is_none_or(|m| d["modality"] == **m)
    };
    let projects: Vec<Value> = sources
        .projects
        .iter()
        .filter(|(k, _)| key.as_ref().is_none_or(|wanted| wanted == *k))
        .map(|(_, p)| p.clone())
        .collect();
    let mut page = Vec::new();
    let mut size = serde_json::to_vec(&projects).map_or(0, |b| b.len());
    let mut next = None;
    let mut candidates = sources
        .datasets
        .range::<String, _>((
            after.map_or(std::ops::Bound::Unbounded, std::ops::Bound::Excluded),
            std::ops::Bound::Unbounded,
        ))
        .map(|(_, d)| d)
        .filter(|d| wanted(d))
        .peekable();
    while let Some(dataset) = candidates.next() {
        let bytes = serde_json::to_vec(dataset).map_or(0, |b| b.len()) + 512;
        if page.len() == limit || (!page.is_empty() && size + bytes > PAGE_BYTES) {
            next = page
                .last()
                .and_then(|d: &Value| d["id"].as_str())
                .map(|id| format!("after:{id}"));
            break;
        }
        size += bytes;
        page.push(dataset.clone());
        if candidates.peek().is_none() {
            break;
        }
    }
    let used: BTreeSet<String> = page
        .iter()
        .filter_map(|d| d["location"]["root_id"].as_str().map(str::to_owned))
        .collect();
    let known: BTreeSet<&str> = sources.projects.keys().map(String::as_str).collect();
    // A dataset only names projects the response lists (data §4.7).
    for dataset in &mut page {
        if let Some(list) = dataset["projects"].as_array_mut() {
            list.retain(|p| p["project_key"].as_str().is_some_and(|k| known.contains(k)));
        }
    }
    let projects = if key.is_some() {
        projects
    } else {
        sources.projects.into_values().collect()
    };
    Ok(json!({
        "contract": "nu9ve-data/1", "app": "nuun", "generated_at": nu9ve_time::now_utc(),
        "projects": projects,
        "roots": sources.roots.into_iter().filter(|(id, _)| used.contains(id)).map(|(_, r)| r).collect::<Vec<_>>(),
        "datasets": page, "next_cursor": next}))
}

/// `data_export` (nu9ve-export-v1 §1–§2). Never bytes: a dataset of nüün is a folder on a disk or a living
/// collection on this machine, so it is `too_large` with where it is.
pub fn data_export(home: &Path, params: &Value) -> Result<Value> {
    let p = params_of(params, &["dataset", "max_bytes"])?;
    let dataset = match p.get("dataset") {
        Some(Value::String(d)) if dataset_id_ok(d) => d.clone(),
        _ => return Err(invalid("dataset must be an id of this catalog")),
    };
    // Checked as the contract asks; with no file to give, nothing is ever within it.
    if let Some(v) = p.get("max_bytes") {
        require(
            v.as_u64().is_some_and(|n| (1..=MAX_SAFE).contains(&n)),
            "invalid_params",
            "max_bytes must be an integer from 1 to 2^53 - 1",
        )?;
    }
    if dataset == crate::accumulations::DATASET {
        let max = p
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(64 * 1024 * 1024);
        return Ok(crate::accumulations::export(home, max).unwrap_or_else(
            || json!({"contract": "nu9ve-export/1", "dataset": dataset, "status": "not_found"}),
        ));
    }
    let sources = sources(home)?;
    let Some(found) = sources.datasets.get(&dataset) else {
        return Ok(
            json!({"contract": "nu9ve-export/1", "dataset": dataset, "status": "not_found"}),
        );
    };
    let root = sources
        .roots
        .get(found["location"]["root_id"].as_str().unwrap_or_default())
        .cloned()
        .unwrap_or_default();
    let place = if let Some(volume_id) = root["volume_id"].as_str() {
        json!({"volume_id": volume_id, "path": found["location"]["path"]})
    } else {
        json!({"volume_id": null, "path": root["path"]})
    };
    let mut answer = json!({"contract": "nu9ve-export/1", "dataset": dataset, "status": "too_large", "where": place});
    if let Some(bytes) = found["size"]["bytes"].as_u64() {
        answer["bytes"] = json!(bytes);
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOLUME: &str = "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f";

    fn dataset(n: usize) -> Value {
        json!({"id": format!("group:{n:064x}"), "owner": "nuun", "title": format!("Carpeta {n}"),
            "modality": "media", "privacy": "personal", "storage": "in_place",
            "location": {"root_id": "disk", "path": format!("001_raw/p{n}")},
            "projects": [{"project_key": "baches"}, {"project_key": "gone"}], "state": "ok",
            "updated_at": "2026-11-22T00:00:00Z", "size": {"bytes": 1000 + n, "items": 2, "unit": "files"}})
    }

    fn home_with_index(count: usize) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let index = json!({"contract": "nu9ve-data/1", "app": "nuun", "generated_at": "2026-11-22T00:00:00Z",
            "projects": [], "roots": [{"id": "disk", "label": "MEDIA502", "state": "ok", "volume_id": VOLUME, "volume_path": ""}],
            "datasets": (0..count).map(dataset).collect::<Vec<_>>(), "next_cursor": null});
        std::fs::create_dir_all(home.path().join(LOCAL)).unwrap();
        std::fs::write(home.path().join(local_index(VOLUME)), index.to_string()).unwrap();
        home
    }

    #[test]
    fn the_catalog_describes_the_last_index_of_each_disk_with_its_volume() {
        let home = home_with_index(3);
        let page = data_catalog(home.path(), &json!({})).unwrap();
        assert_eq!(page["app"], "nuun");
        assert_eq!(page["datasets"].as_array().unwrap().len(), 3);
        let root = &page["roots"][0];
        assert_eq!(
            (
                root["id"].as_str().unwrap(),
                root["volume_id"].as_str().unwrap()
            ),
            (format!("vol-{VOLUME}").as_str(), VOLUME)
        );
        // Not mounted here: the disk and its datasets are missing, as last known.
        assert_eq!(root["state"], "missing");
        assert_eq!(page["datasets"][0]["state"], "missing");
        assert_eq!(
            page["datasets"][0]["location"]["root_id"],
            format!("vol-{VOLUME}")
        );
        // Only projects this app lists (none in an empty catalog).
        assert_eq!(page["datasets"][0]["projects"], json!([]));
        assert_eq!(page["next_cursor"], Value::Null);
    }

    #[test]
    fn pages_follow_the_cursor_and_filters_are_lists_not_errors() {
        let home = home_with_index(5);
        let first = data_catalog(home.path(), &json!({"limit": 2})).unwrap();
        let cursor = first["next_cursor"].as_str().unwrap().to_owned();
        let second = data_catalog(home.path(), &json!({"limit": 2, "cursor": cursor})).unwrap();
        let third = data_catalog(
            home.path(),
            &json!({"limit": 2, "cursor": second["next_cursor"]}),
        )
        .unwrap();
        let ids: Vec<Value> = [&first, &second, &third]
            .iter()
            .flat_map(|p| {
                p["datasets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d["id"].clone())
            })
            .collect();
        assert_eq!(
            ids,
            (0..5).map(|n| dataset(n)["id"].clone()).collect::<Vec<_>>()
        );
        assert_eq!(third["next_cursor"], Value::Null);
        let none = data_catalog(home.path(), &json!({"project_key": "otro"})).unwrap();
        assert_eq!(
            (
                none["datasets"].as_array().unwrap().len(),
                none["projects"].as_array().unwrap().len()
            ),
            (0, 0)
        );
        assert_eq!(
            data_catalog(home.path(), &json!({"modality": "table"})).unwrap()["datasets"],
            json!([])
        );
        for bad in [
            json!({"limit": 0}),
            json!({"limit": 501}),
            json!({"other": 1}),
            json!({"project_key": "A"}),
        ] {
            assert_eq!(
                data_catalog(home.path(), &bad).unwrap_err().code,
                "invalid_params",
                "{bad}"
            );
        }
        assert_eq!(
            data_catalog(home.path(), &json!({"cursor": "zzz"}))
                .unwrap_err()
                .code,
            "invalid_cursor"
        );
    }

    #[test]
    fn an_export_is_never_bytes_but_where_the_folder_is() {
        let home = home_with_index(1);
        let id = dataset(0)["id"].as_str().unwrap().to_owned();
        let answer = data_export(home.path(), &json!({"dataset": id})).unwrap();
        assert_eq!(answer["status"], "too_large");
        assert_eq!(
            answer["where"],
            json!({"volume_id": VOLUME, "path": "001_raw/p0"})
        );
        assert_eq!(answer["bytes"], 1000);
        assert_eq!(
            data_export(home.path(), &json!({"dataset": "group:none"})).unwrap()["status"],
            "not_found"
        );
        for bad in [
            json!({}),
            json!({"dataset": "Con espacios"}),
            json!({"dataset": id, "max_bytes": 0}),
            json!({"dataset": id, "x": 1}),
        ] {
            assert_eq!(
                data_export(home.path(), &bad).unwrap_err().code,
                "invalid_params",
                "{bad}"
            );
        }
    }
}
