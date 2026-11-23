use super::{INDEX, document};
use crate::catalog::{Catalog, sql};
use crate::suite_device::{digest, time, uuid};
use crate::tree::{Root, absent, stamp_json};
use nu9ve_json::{Error, Result, require, sha256_hex};
use nu9ve_volumes::fs;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Read;

pub(super) const LIMIT: usize = 8 * 1024 * 1024;
const BACKUPS: &str = ".ncreator/catalog-backups";
const RECEIPTS: &str = ".ncreator/receipts";
const MAX_SAFE: u64 = (1 << 53) - 1;

type ProjectLinks = HashMap<(String, String), BTreeSet<String>>;

fn project_links(catalog: &Catalog) -> Result<(BTreeMap<String, Value>, ProjectLinks)> {
    let mut projects = BTreeMap::new();
    let mut members = ProjectLinks::new();
    let Some(db) = catalog.db() else {
        return Ok((projects, members));
    };
    let mut query = db
        .prepare(
            "SELECT p.id, p.title, m.value, aj.job_id, member.value
         FROM asset_jobs aj JOIN assets a ON a.id=aj.asset_id
         JOIN asset_projects ap ON ap.asset_id=a.id JOIN projects p ON p.id=ap.project_id
         LEFT JOIN metadata m ON m.key='project-state:' || p.id,
         json_each(a.payload, '$.paths') member",
        )
        .map_err(sql)?;
    let mut rows = query.query([]).map_err(sql)?;
    while let Some(row) = rows.next().map_err(sql)? {
        let (id, title, state): (String, String, Option<String>) = (
            row.get(0).map_err(sql)?,
            row.get(1).map_err(sql)?,
            row.get(2).map_err(sql)?,
        );
        // Existing project keys are the local stable ids until an explicit suite key is assigned.
        let key = id.to_ascii_lowercase();
        if !identifier(&key, 64, b"-", false) {
            continue;
        }
        projects.entry(key.clone()).or_insert_with(|| json!({
            "project_key":key,"title":short(&title, "Proyecto"),"parent_key":null,
            "state": if state.as_deref() == Some("\"archived\"") { "archived" } else { "active" }
        }));
        members
            .entry((row.get(3).map_err(sql)?, row.get(4).map_err(sql)?))
            .or_default()
            .insert(key);
    }
    Ok((projects, members))
}

fn short(s: &str, fallback: &str) -> String {
    let value: String = s.chars().filter(|c| !c.is_control()).take(200).collect();
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value
    }
}

pub(super) fn build(
    root: &Root,
    marker: &Value,
    catalog: &Catalog,
    registration: &Value,
) -> Result<Value> {
    let now = nu9ve_time::now_utc();
    let id = marker["volume_id"].as_str().unwrap();
    let (mut projects, links) = project_links(catalog)?;
    let mut used_projects = BTreeSet::new();
    let copies = catalog.json_rows(
        "SELECT payload FROM copies WHERE volume_id=?1 ORDER BY json_extract(payload,'$.verified_at') DESC, job_id, relative_path",
        [registration["id"].as_str().unwrap_or_default()],
    )?;
    let mut paths = HashSet::new();
    let mut datasets = Vec::new();
    for copy in copies {
        let Some(path) = copy["destination"].as_str().filter(|p| relative(p)) else {
            continue;
        };
        if !paths.insert(path.to_owned()) {
            continue;
        }
        let stamp = match root.stat(path) {
            Ok(Some(stamp)) => stamp,
            Ok(None) => continue,
            // Legacy rows can refer to files that were replaced with a link or directory.
            Err(error) if error.code == "unsafe_file" => continue,
            Err(error) => return Err(error),
        };
        let checked = &copy["verified_at"];
        let verified = digest(&copy["sha256"]) && time(checked);
        let state = if !verified {
            "unverified"
        } else if stamp_json(&stamp) == copy["stamp"] {
            "ok"
        } else {
            "changed"
        };
        let key = (
            copy["job_id"].as_str().unwrap_or_default().to_owned(),
            copy["path"].as_str().unwrap_or_default().to_owned(),
        );
        let projects: Vec<Value> = links
            .get(&key)
            .into_iter()
            .flatten()
            .map(|project| {
                used_projects.insert(project.clone());
                json!({"project_key":project})
            })
            .collect();
        let mut dataset = json!({
            "id":format!("copy:{}", sha256_hex(format!("{id}\n{path}").as_bytes())),
            "owner":"nuun","title":short(path.rsplit('/').next().unwrap(), "Archivo"),
            "modality":"media","privacy":"personal","storage":"in_place",
            "location":{"root_id":"disk","path":path}, "projects":projects, "state":state,
            "updated_at":if time(checked) { checked.as_str().unwrap() } else { &now }
        });
        if stamp.size <= MAX_SAFE {
            dataset["size"] = json!({"bytes":stamp.size, "items":1, "unit":"files"});
        }
        if verified {
            dataset["hash"] = json!({"algorithm":"sha256","value":copy["sha256"],"of":"original","verified_at":checked});
        }
        datasets.push(dataset);
    }
    projects.retain(|key, _| used_projects.contains(key));
    let mut value = json!({
        "contract":"nu9ve-data/1","app":"nuun","generated_at":now,
        "projects":projects.into_values().collect::<Vec<_>>(),
        "roots":[{"id":"disk","label":marker["label"],"state":"ok","volume_id":id,"volume_path":""}],
        "datasets":datasets,"next_cursor":null,
        "nuun_receipts":{"path":RECEIPTS,"count":json_files(root, RECEIPTS)?.len()}
    });
    if registration["role"] == "archive" {
        for path in json_files(root, BACKUPS)?
            .into_iter()
            .rev()
            .filter(|name| name.ends_with(".sqlite3.json"))
        {
            let Some(receipt) = document(root, &path, 16 * 1024, "invalid_snapshot")? else {
                continue;
            };
            if snapshot_receipt(&receipt).is_some_and(|file| format!("{file}.json") == path) {
                value["nuun_snapshot"] = receipt;
                verify_snapshot(root, &value)?;
                break;
            }
        }
    }
    Ok(value)
}

fn json_files(root: &Root, folder: &str) -> Result<Vec<String>> {
    let dir = match root.parent(&format!("{folder}/entry"), false) {
        Ok((dir, _)) => dir,
        Err(error) if absent(&error) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| fs::io_error(e, &dir))? {
        let entry = entry.map_err(|e| fs::io_error(e, &dir))?;
        let name = entry.file_name();
        if let Some(name) = name
            .to_str()
            .filter(|s| !s.starts_with('.') && s.ends_with(".json"))
            && entry
                .file_type()
                .map_err(|e| fs::io_error(e, &entry.path()))?
                .is_file()
        {
            names.push(format!("{folder}/{name}"));
        }
    }
    names.sort();
    Ok(names)
}

fn snapshot_receipt(value: &Value) -> Option<&str> {
    let path = value["path"].as_str()?;
    let file = path.strip_prefix(".ncreator/catalog-backups/")?;
    (relative(file)
        && !file.contains('/')
        && file.ends_with(".sqlite3")
        && value["schema"] == "nu9ve-catalog-backup/1"
        && digest(&value["sha256"])
        && time(&value["created_at"]))
    .then_some(path)
}

pub(super) fn verify_snapshot(root: &Root, index: &Value) -> Result<Value> {
    let Some(receipt) = index.get("nuun_snapshot") else {
        return Ok(Value::Null);
    };
    let invalid = || Error::new("invalid_snapshot", "Invalid catalog backup or receipt");
    let path = snapshot_receipt(receipt).ok_or_else(invalid)?;
    let on_disk = document(root, &format!("{path}.json"), 16 * 1024, "invalid_snapshot")?;
    require(
        on_disk.as_ref() == Some(receipt),
        "invalid_snapshot",
        "Catalog backup receipt disagrees with the index",
    )?;
    let mut file = root.open_read(path)?.ok_or_else(invalid)?;
    let mut header = [0; 16];
    file.read_exact(&mut header).map_err(|_| invalid())?;
    require(
        &header == b"SQLite format 3\0",
        "invalid_snapshot",
        "Backup is not a SQLite snapshot",
    )?;
    let mut hash = Sha256::new();
    hash.update(header);
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let len = file.read(&mut buffer).map_err(|_| invalid())?;
        if len == 0 {
            break;
        }
        hash.update(&buffer[..len]);
    }
    require(
        receipt["sha256"] == nu9ve_json::hex(&hash.finalize()),
        "invalid_snapshot",
        "Catalog snapshot checksum does not match its receipt",
    )?;
    Ok(json!({"state":"verified","path":path,"created_at":receipt["created_at"]}))
}

fn identifier(s: &str, max: usize, punctuation: &[u8], start_alnum: bool) -> bool {
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    !s.is_empty()
        && s.len() <= max
        && (!start_alnum || alnum(s.as_bytes()[0]))
        && s.bytes().all(|b| alnum(b) || punctuation.contains(&b))
}

fn relative(s: &str) -> bool {
    !s.contains(['\\', '\0'])
        && s.split('/')
            .all(|p| !matches!(p, "" | "." | "..") && !p.contains(':'))
}

fn text(value: &Value, max: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| (1..=max).contains(&s.chars().count()))
}

fn list<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value[field]
        .as_array()
        .ok_or_else(|| Error::new("invalid_index", format!("Invalid {field} in {INDEX}")))
}

pub(super) fn validate(value: &Value) -> Result<()> {
    require(
        value["app"] == "nuun",
        "app_mismatch",
        "Index does not belong to nüün",
    )?;
    require(
        value["contract"] == "nu9ve-data/1"
            && value.get("next_cursor") == Some(&Value::Null)
            && time(&value["generated_at"]),
        "invalid_index",
        "Invalid portable index envelope",
    )?;
    let mut keys = HashMap::new();
    for project in list(value, "projects")? {
        let key = project["project_key"]
            .as_str()
            .filter(|s| identifier(s, 64, b"-", false));
        require(
            key.is_some()
                && text(&project["title"], 200)
                && project.get("parent_key").is_some()
                && matches!(project["state"].as_str(), Some("active" | "archived"))
                && keys.insert(key.unwrap(), &project["parent_key"]).is_none(),
            "invalid_index",
            "Invalid project in portable index",
        )?;
    }
    for (&key, &parent) in &keys {
        let mut seen = HashSet::from([key]);
        let mut current = parent;
        while !current.is_null() {
            let next = current
                .as_str()
                .filter(|s| seen.insert(s))
                .and_then(|s| keys.get(s));
            require(
                next.is_some(),
                "invalid_index",
                "Unknown project parent or cycle",
            )?;
            current = next.unwrap();
        }
    }
    let mut roots = HashSet::new();
    for root in list(value, "roots")? {
        let id = root["id"]
            .as_str()
            .filter(|s| identifier(s, 64, b"._-", true));
        let path_ok = root.get("path").is_none_or(|v| {
            v.as_str().is_some_and(|s| {
                s.starts_with('/')
                    || (s.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                        && s.as_bytes().get(1..3) == Some(b":\\"))
            })
        });
        require(
            id.is_some()
                && text(&root["label"], 120)
                && path_ok
                && matches!(root["state"].as_str(), Some("ok" | "missing"))
                && match (root.get("volume_id"), root.get("volume_path")) {
                    (None, None) => true,
                    (Some(id), Some(path)) => {
                        uuid(id) && path.as_str().is_some_and(|s| s.is_empty() || relative(s))
                    }
                    _ => false,
                }
                && roots.insert(id.unwrap()),
            "invalid_index",
            "Invalid root in portable index",
        )?;
    }
    let mut datasets = HashSet::new();
    for dataset in list(value, "datasets")? {
        validate_dataset(dataset, &roots, &keys)?;
        require(
            datasets.insert(dataset["id"].as_str().unwrap()),
            "invalid_index",
            "Duplicate dataset id",
        )?;
    }
    Ok(())
}

fn validate_dataset(
    dataset: &Value,
    roots: &HashSet<&str>,
    projects: &HashMap<&str, &Value>,
) -> Result<()> {
    let extensions_valid = dataset
        .get("description")
        .is_none_or(|d| d.as_str().is_some_and(|s| s.chars().count() <= 2000))
        && dataset.get("tags").is_none_or(|t| {
            t.as_array()
                .is_some_and(|a| a.len() <= 32 && a.iter().all(|v| text(v, 64)))
        })
        && dataset.get("size").is_none_or(|s| {
            s.is_object()
                && ["bytes", "items"].into_iter().all(|k| {
                    s.get(k)
                        .is_none_or(|n| n.as_u64().is_some_and(|n| n <= MAX_SAFE))
                })
                && s.get("unit").is_none_or(|u| text(u, 32))
        })
        && dataset.get("revision").is_none_or(|r| {
            text(&r["id"], 128)
                && ["collected_at", "published_at"]
                    .into_iter()
                    .all(|k| r.get(k).is_none_or(time))
        })
        && dataset.get("origin_key").is_none_or(|k| {
            k.as_str().is_some_and(|s| {
                s.chars().count() <= 200
                    && s.split_once(':')
                        .is_some_and(|(ns, _)| identifier(ns, 64, b":_-", true))
            })
        })
        && dataset.get("axes").is_none_or(axes_valid);
    let location_valid = match dataset["storage"].as_str() {
        Some("copied") => dataset.get("location").is_none(),
        Some("in_place") => {
            let location = &dataset["location"];
            location["root_id"]
                .as_str()
                .is_some_and(|id| roots.contains(id))
                && location["path"].as_str().is_some_and(relative)
        }
        _ => false,
    };
    let hash_valid = dataset.get("hash").map_or(dataset["state"] != "ok", |h| {
        h["algorithm"] == "sha256"
            && digest(&h["value"])
            && time(&h["verified_at"])
            && matches!(
                h["of"].as_str(),
                Some("original" | "normalized" | "manifest")
            )
    });
    let projects_valid = list(dataset, "projects")?.iter().all(|p| {
        p["project_key"]
            .as_str()
            .is_some_and(|k| projects.contains_key(k))
            && p.get("role")
                .is_none_or(|r| matches!(r.as_str(), Some("base" | "context" | "output")))
    });
    require(
        dataset["id"]
            .as_str()
            .is_some_and(|s| identifier(s, 128, b"._:-", true))
            && dataset["owner"] == "nuun"
            && text(&dataset["title"], 200)
            && text(&dataset["modality"], 32)
            && matches!(
                dataset["privacy"].as_str(),
                Some("public" | "restricted" | "personal")
            )
            && matches!(
                dataset["state"].as_str(),
                Some("ok" | "changed" | "missing" | "unverified")
            )
            && time(&dataset["updated_at"])
            && location_valid
            && hash_valid
            && projects_valid
            && extensions_valid,
        "invalid_index",
        "Invalid dataset in portable index",
    )
}

fn axes_valid(v: &Value) -> bool {
    let timestamp = |v: &Value| {
        let s = v.as_str()?;
        let pos = s.len().checked_sub(6)?;
        let offset = s
            .as_bytes()
            .get(pos)
            .is_some_and(|b| matches!(b, b'+' | b'-'))
            && s.as_bytes().get(pos + 3) == Some(&b':');
        if time(v) {
            nu9ve_time::DateTime::fromisoformat(&format!("{}+00:00", s.strip_suffix('Z')?))?
                .timestamp()
        } else if offset && time(&json!(format!("{}Z", s.get(..pos)?))) {
            nu9ve_time::DateTime::fromisoformat(s)?.timestamp()
        } else {
            None
        }
    };
    let namespaces = |items: &Value| {
        items.as_array().is_some_and(|a| {
            a.iter()
                .all(|n| n.as_str().is_some_and(|s| identifier(s, 64, b":_-", true)))
        })
    };
    v.is_object()
        && v.get("time").is_none_or(|t| {
            matches!(
                t["kind"].as_str(),
                Some("observed" | "published" | "collected" | "media")
            ) && matches!(
                t["precision"].as_str(),
                Some("second" | "minute" | "hour" | "day" | "month" | "year")
            ) && timestamp(&t["start"])
                .zip(timestamp(&t["end"]))
                .is_some_and(|(a, b)| a <= b)
        })
        && v.get("place").is_none_or(|p| {
            (p.get("bbox").is_some() || p.get("territory").is_some())
                && p.get("bbox").is_none_or(|b| {
                    b.as_array().is_some_and(|a| {
                        a.len() == 4
                            && a.iter().enumerate().all(|(i, n)| {
                                let range = if i % 2 == 0 {
                                    -180.0..=180.0
                                } else {
                                    -90.0..=90.0
                                };
                                n.as_f64().is_some_and(|n| range.contains(&n))
                            })
                            && a[1].as_f64() <= a[3].as_f64()
                    })
                })
                && p.get("territory").is_none_or(namespaces)
                && p.get("crs_source").is_none_or(|c| {
                    c.as_str()
                        .and_then(|s| s.strip_prefix("EPSG:"))
                        .is_some_and(|s| {
                            (1..=6).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
                        })
                })
        })
        && v.get("entity").is_none_or(|e| {
            namespaces(&e["namespaces"])
                && e["namespaces"].as_array().is_some_and(|a| !a.is_empty())
        })
}
