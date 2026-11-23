//! nu9ve-data-v1: catalog validation (§3, §4), aggregation (§5) and what an agent sees (§6).
//! The Python reference is scripts/data-examples.py; the tests run its examples.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::text::clean;

pub const CONTRACT: &str = "nu9ve-data/1";
const MAX_SAFE: u64 = (1 << 53) - 1;
const PERSONAL_FIELDS: [&str; 6] = ["owner", "id", "modality", "privacy", "size", "state"];

fn chars_ok(text: &str, first: fn(u8) -> bool, rest: &[u8], longest: usize) -> bool {
    let b = text.as_bytes();
    !b.is_empty()
        && b.len() <= longest
        && first(b[0])
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || rest.contains(c))
}

fn lower_or_digit(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit()
}

pub fn valid_project_key(key: &str) -> bool {
    let b = key.as_bytes();
    (1..=64).contains(&b.len()) && b.iter().all(|c| lower_or_digit(*c) || *c == b'-')
}

pub fn valid_dataset_id(id: &str) -> bool {
    chars_ok(id, lower_or_digit, b"._:-", 128)
}

fn valid_root_id(id: &str) -> bool {
    chars_ok(id, lower_or_digit, b"._-", 64)
}

/// A UUID 4 in lower case with hyphens, as `volume_id` (§3).
pub fn valid_uuid4(id: &str) -> bool {
    let b = id.as_bytes();
    let hex = |c: &u8| c.is_ascii_digit() || (b'a'..=b'f').contains(c);
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => *c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => hex(c),
        })
}

fn valid_namespace(name: &str) -> bool {
    chars_ok(name, lower_or_digit, b":_-", 64)
}

fn valid_app(app: &str) -> bool {
    chars_ok(app, |b| b.is_ascii_lowercase(), b"-", 32)
}

fn text_ok(value: Option<&Value>, longest: usize) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|s| (1..=longest).contains(&s.chars().count()))
}

/// RFC 3339; `utc_only` requires `Z`.
fn rfc3339(value: Option<&Value>, utc_only: bool) -> bool {
    let Some(text) = value.and_then(Value::as_str) else {
        return false;
    };
    let b = text.as_bytes();
    let digits = |r: std::ops::Range<usize>| {
        r.into_iter()
            .all(|i| b.get(i).is_some_and(u8::is_ascii_digit))
    };
    if b.len() < 20
        || !(digits(0..4)
            && b[4] == b'-'
            && digits(5..7)
            && b[7] == b'-'
            && digits(8..10)
            && b[10] == b'T'
            && digits(11..13)
            && b[13] == b':'
            && digits(14..16)
            && b[16] == b':'
            && digits(17..19))
    {
        return false;
    }
    let mut rest = &b[19..];
    if rest.first() == Some(&b'.') {
        let n = rest[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if !(1..=9).contains(&n) {
            return false;
        }
        rest = &rest[1 + n..];
    }
    if rest == b"Z" {
        return true;
    }
    !utc_only
        && rest.len() == 6
        && matches!(rest[0], b'+' | b'-')
        && rest[1..3].iter().all(u8::is_ascii_digit)
        && rest[3] == b':'
        && rest[4..6].iter().all(u8::is_ascii_digit)
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|_| value.is_number())
}

fn axes_problem(axes: &Value) -> Option<&'static str> {
    let Some(axes) = axes.as_object() else {
        return Some("bad_axes");
    };
    if let Some(time) = axes.get("time") {
        let kind = time.get("kind").and_then(Value::as_str);
        let precision = time.get("precision").and_then(Value::as_str);
        if !matches!(kind, Some("observed" | "published" | "collected" | "media"))
            || !matches!(
                precision,
                Some("second" | "minute" | "hour" | "day" | "month" | "year")
            )
            || !rfc3339(time.get("start"), false)
            || !rfc3339(time.get("end"), false)
        {
            return Some("bad_time_axis");
        }
        if time["end"].as_str() < time["start"].as_str() {
            return Some("time_axis_reversed");
        }
    }
    if let Some(place) = axes.get("place") {
        let Some(place) = place.as_object() else {
            return Some("bad_place_axis");
        };
        if !place.contains_key("bbox") && !place.contains_key("territory") {
            return Some("bad_place_axis");
        }
        if let Some(bbox) = place.get("bbox") {
            let values: Vec<f64> = bbox
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(number)
                .collect();
            if bbox.as_array().map(Vec::len) != Some(4) || values.len() != 4 {
                return Some("bad_place_axis");
            }
            let (west, south, east, north) = (values[0], values[1], values[2], values[3]);
            if !((-180.0..=180.0).contains(&west)
                && (-180.0..=180.0).contains(&east)
                && -90.0 <= south
                && south <= north
                && north <= 90.0)
            {
                return Some("bad_place_axis");
            }
        }
        if let Some(territory) = place.get("territory")
            && !territory
                .as_array()
                .is_some_and(|t| t.iter().all(|k| k.as_str().is_some_and(valid_namespace)))
        {
            return Some("bad_place_axis");
        }
        if let Some(crs) = place.get("crs_source")
            && !crs
                .as_str()
                .and_then(|c| c.strip_prefix("EPSG:"))
                .is_some_and(|n| {
                    (1..=6).contains(&n.len()) && n.bytes().all(|c| c.is_ascii_digit())
                })
        {
            return Some("bad_place_axis");
        }
    }
    if let Some(entity) = axes.get("entity") {
        let names = entity.get("namespaces").and_then(Value::as_array);
        if !names.is_some_and(|n| {
            !n.is_empty() && n.iter().all(|k| k.as_str().is_some_and(valid_namespace))
        }) {
            return Some("bad_entity_axis");
        }
    }
    None
}

fn relative_path_ok(path: Option<&Value>) -> bool {
    let Some(path) = path.and_then(Value::as_str) else {
        return false;
    };
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

fn dataset_problem(
    app: &str,
    dataset: &Value,
    roots: &HashSet<String>,
    keys: &HashMap<String, Option<String>>,
) -> Option<&'static str> {
    let Some(d) = dataset.as_object() else {
        return Some("dataset_not_object");
    };
    for name in [
        "id",
        "owner",
        "title",
        "modality",
        "privacy",
        "storage",
        "projects",
        "state",
        "updated_at",
    ] {
        if !d.contains_key(name) {
            return Some("missing_field");
        }
    }
    if !d["id"].as_str().is_some_and(valid_dataset_id) {
        return Some("bad_id");
    }
    if d["owner"].as_str() != Some(app) {
        return Some("foreign_owner");
    }
    if !text_ok(d.get("title"), 200) {
        return Some("bad_text");
    }
    if let Some(description) = d.get("description")
        && description
            .as_str()
            .is_none_or(|s| s.chars().count() > 2000)
    {
        return Some("bad_text");
    }
    if let Some(tags) = d.get("tags")
        && !tags
            .as_array()
            .is_some_and(|t| t.len() <= 32 && t.iter().all(|tag| text_ok(Some(tag), 64)))
    {
        return Some("bad_text");
    }
    if !d["modality"]
        .as_str()
        .is_some_and(|m| (1..=32).contains(&m.len()))
    {
        return Some("bad_modality");
    }
    if !matches!(
        d["privacy"].as_str(),
        Some("public" | "restricted" | "personal")
    ) {
        return Some("bad_privacy");
    }
    if !matches!(
        d["state"].as_str(),
        Some("ok" | "changed" | "missing" | "unverified")
    ) {
        return Some("bad_state");
    }
    match d["storage"].as_str() {
        Some("in_place") => {
            let location = d.get("location").and_then(Value::as_object);
            let root = location
                .and_then(|l| l.get("root_id"))
                .and_then(Value::as_str);
            if !root.is_some_and(|r| roots.contains(r)) {
                return Some("unknown_root");
            }
            if !relative_path_ok(location.and_then(|l| l.get("path"))) {
                return Some("bad_path");
            }
        }
        Some("copied") => {
            if d.contains_key("location") {
                return Some("location_on_copied");
            }
        }
        _ => return Some("bad_storage"),
    }
    if let Some(hash) = d.get("hash") {
        let value = hash.get("value").and_then(Value::as_str);
        if hash.get("algorithm").and_then(Value::as_str) != Some("sha256")
            || !value.is_some_and(|v| {
                v.len() == 64
                    && v.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
            || !matches!(
                hash.get("of").and_then(Value::as_str),
                Some("original" | "normalized" | "manifest")
            )
            || !rfc3339(hash.get("verified_at"), true)
        {
            return Some("bad_hash");
        }
    } else if d["state"] == "ok" {
        return Some("ok_without_hash");
    }
    if let Some(size) = d.get("size") {
        let Some(size) = size.as_object() else {
            return Some("bad_size");
        };
        for name in ["bytes", "items"] {
            if let Some(n) = size.get(name)
                && n.as_u64().is_none_or(|n| n > MAX_SAFE)
            {
                return Some("bad_size");
            }
        }
        if size.contains_key("unit") && !text_ok(size.get("unit"), 32) {
            return Some("bad_size");
        }
    }
    if let Some(axes) = d.get("axes")
        && let Some(problem) = axes_problem(axes)
    {
        return Some(problem);
    }
    if let Some(origin) = d.get("origin_key") {
        let ok = origin.as_str().is_some_and(|o| {
            o.len() <= 200
                && o.split_once(':')
                    .is_some_and(|(head, _)| valid_namespace(head))
        });
        if !ok {
            return Some("bad_origin_key");
        }
    }
    if let Some(revision) = d.get("revision") {
        if !revision.is_object() || !text_ok(revision.get("id"), 128) {
            return Some("bad_revision");
        }
        for key in ["collected_at", "published_at"] {
            if revision.get(key).is_some() && !rfc3339(revision.get(key), true) {
                return Some("bad_revision");
            }
        }
    }
    let Some(projects) = d["projects"].as_array() else {
        return Some("bad_projects");
    };
    for link in projects {
        if !link
            .get("project_key")
            .and_then(Value::as_str)
            .is_some_and(|k| keys.contains_key(k))
        {
            return Some("undeclared_project");
        }
        if let Some(role) = link.get("role")
            && !matches!(role.as_str(), Some("base" | "context" | "output"))
        {
            return Some("bad_role");
        }
    }
    if !rfc3339(d.get("updated_at"), true) {
        return Some("bad_time");
    }
    None
}

/// The first rule a `data_catalog` reply from `app` breaks, or `None`.
pub fn catalog_problem(app: &str, catalog: &Value) -> Option<&'static str> {
    let Some(c) = catalog.as_object() else {
        return Some("not_object");
    };
    for name in [
        "contract",
        "app",
        "generated_at",
        "projects",
        "roots",
        "datasets",
        "next_cursor",
    ] {
        if !c.contains_key(name) {
            return Some("missing_field");
        }
    }
    if c["contract"] != CONTRACT {
        return Some("unknown_contract");
    }
    if c["app"].as_str() != Some(app) || !valid_app(app) {
        return Some("app_mismatch");
    }
    if !rfc3339(c.get("generated_at"), true) {
        return Some("bad_time");
    }
    match &c["next_cursor"] {
        Value::Null => {}
        Value::String(s) if (1..=256).contains(&s.len()) => {}
        _ => return Some("bad_cursor"),
    }
    let (Some(projects), Some(roots), Some(datasets)) = (
        c["projects"].as_array(),
        c["roots"].as_array(),
        c["datasets"].as_array(),
    ) else {
        return Some("wrong_type");
    };
    let mut keys: HashMap<String, Option<String>> = HashMap::new();
    for project in projects {
        let Some(key) = project
            .get("project_key")
            .and_then(Value::as_str)
            .filter(|k| valid_project_key(k))
        else {
            return Some("bad_project_key");
        };
        if keys.contains_key(key) {
            return Some("duplicate_project");
        }
        if !text_ok(project.get("title"), 200)
            || !matches!(
                project.get("state").and_then(Value::as_str),
                Some("active" | "archived")
            )
            || project.get("parent_key").is_none()
        {
            return Some("bad_project");
        }
        keys.insert(
            key.to_owned(),
            project["parent_key"].as_str().map(str::to_owned),
        );
    }
    for (key, parent) in &keys {
        let mut seen = HashSet::from([key.as_str()]);
        let mut parent = parent.as_deref();
        while let Some(p) = parent {
            let Some(next) = keys.get(p) else {
                return Some("unknown_parent");
            };
            if !seen.insert(p) {
                return Some("project_cycle");
            }
            parent = next.as_deref();
        }
    }
    let mut root_ids = HashSet::new();
    for root in roots {
        let id = root
            .get("id")
            .and_then(Value::as_str)
            .filter(|i| valid_root_id(i));
        if id.is_none()
            || !text_ok(root.get("label"), 120)
            || !matches!(
                root.get("state").and_then(Value::as_str),
                Some("ok" | "missing")
            )
        {
            return Some("bad_root");
        }
        if let Some(path) = root.get("path") {
            let absolute = path.as_str().is_some_and(|p| {
                p.starts_with('/')
                    || (p.len() > 2 && p.as_bytes()[0].is_ascii_alphabetic() && &p[1..3] == ":\\")
            });
            if !absolute {
                return Some("bad_root");
            }
        }
        match (root.get("volume_id"), root.get("volume_path")) {
            (None, None) => {}
            (Some(id), Some(path)) => {
                let id_ok = id.as_str().is_some_and(valid_uuid4);
                let path_ok = path.as_str() == Some("") || relative_path_ok(Some(path));
                if !id_ok || !path_ok {
                    return Some("bad_root");
                }
            }
            _ => return Some("bad_root"),
        }
        root_ids.insert(id.unwrap_or_default().to_owned());
    }
    let mut ids = HashSet::new();
    for dataset in datasets {
        if let Some(problem) = dataset_problem(app, dataset, &root_ids, &keys) {
            return Some(problem);
        }
        if !ids.insert(dataset["id"].as_str().unwrap_or_default()) {
            return Some("duplicate_dataset");
        }
    }
    None
}

/// One app's answer to an aggregator: its catalog, or why there is none.
pub enum Reply {
    NotOpen,
    Failed(String),
    Catalog(Value),
}

/// §5 over the given replies, in the shape of `aggregate.expected` in the examples.
pub fn aggregate(replies: &[(String, Reply)]) -> Value {
    let mut ordered: Vec<&(String, Reply)> = replies.iter().collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let mut apps = Vec::new();
    let mut projects: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut datasets = Vec::new();
    for (app, reply) in ordered {
        let catalog = match reply {
            Reply::NotOpen => {
                apps.push(json!({"app": app, "state": "not_open"}));
                continue;
            }
            Reply::Failed(reason) => {
                apps.push(json!({"app": app, "state": "invalid", "reason": reason}));
                continue;
            }
            Reply::Catalog(catalog) => catalog,
        };
        if let Some(problem) = catalog_problem(app, catalog) {
            apps.push(json!({"app": app, "state": "invalid", "reason": problem}));
            continue;
        }
        let list = catalog["datasets"].as_array().cloned().unwrap_or_default();
        apps.push(json!({"app": app, "state": "ok", "datasets": list.len()}));
        for project in catalog["projects"].as_array().into_iter().flatten() {
            let key = project["project_key"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            projects.entry(key).or_default().insert(
                app.clone(),
                json!({"title": project["title"], "parent_key": project["parent_key"],
                       "state": project["state"], "datasets": []}),
            );
        }
        for dataset in &list {
            let reference = format!("{app}:{}", dataset["id"].as_str().unwrap_or_default());
            datasets.push(reference.clone());
            for link in dataset["projects"].as_array().into_iter().flatten() {
                let key = link["project_key"].as_str().unwrap_or_default();
                if let Some(Value::Array(refs)) = projects
                    .get_mut(key)
                    .and_then(|apps| apps.get_mut(app.as_str()))
                    .and_then(|entry| entry.get_mut("datasets"))
                {
                    refs.push(json!(reference));
                }
            }
        }
    }
    datasets.sort();
    json!({
        "apps": apps,
        "projects": projects.into_iter().map(|(key, apps)| json!({"project_key": key, "apps": apps})).collect::<Vec<_>>(),
        "datasets": datasets,
    })
}

/// What `nu9ve mcp` shows of one dataset: personal ones reduced to their shape (§6),
/// observed text as `_untrusted`, cleaned and bounded (§4.3).
pub fn mcp_view(app: &str, dataset: &Value) -> Value {
    let Some(d) = dataset.as_object() else {
        return Value::Null;
    };
    let mut view = Map::new();
    if d.get("privacy").and_then(Value::as_str) == Some("personal") {
        for key in PERSONAL_FIELDS {
            if let Some(value) = d.get(key) {
                view.insert(key.to_owned(), value.clone());
            }
        }
        if let Some(time) = d.get("axes").and_then(|a| a.get("time")) {
            view.insert("axes".into(), json!({"time": time}));
        }
    } else {
        for (key, value) in d {
            match key.as_str() {
                "title" | "description" => {
                    view.insert(format!("{key}_untrusted"), clean(value));
                }
                "tags" => {
                    view.insert("tags_untrusted".into(), clean(value));
                }
                _ => {
                    view.insert(key.clone(), value.clone());
                }
            }
        }
    }
    view.insert(
        "ref".into(),
        json!(format!(
            "{app}:{}",
            d.get("id").and_then(Value::as_str).unwrap_or_default()
        )),
    );
    Value::Object(view)
}

/// Whether a dataset is personal, for leaving its reference out of project lists by MCP.
pub fn is_personal(dataset: &Value) -> bool {
    dataset.get("privacy").and_then(Value::as_str) == Some("personal")
}
