//! The `snapshot` method: everything the native shell shows at rest, in one read. Port of
//! `media/service.py` `_snapshot` and the catalog reads it uses (`Catalog.jobs`, `Catalog.search`,
//! `workspaces.profiles`, `library.projects`, `release.release_plans`, `closure.overview`, settings and defaults).
//!
//! Not ported yet, reported the way Python reports a failing piece (`errors[piece]`, empty value):
//! - `capabilities`: it starts Clipper and Stalker executables (`integrations.capabilities`).
//! - `sony_removal_recorded` on a Sony job that failed with `FileNotFoundError`: it needs the plan's destination paths
//!   (`transfer.sony_removal_recorded`); the job is still listed, `errors.jobs` says so.
//! - `source_cards` outside Windows (the macOS volume backend is still Python).

use crate::catalog::Catalog;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, Result};
use rusqlite::OptionalExtension;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn sql(error: rusqlite::Error) -> Error {
    Error::new("catalog_unavailable", error.to_string())
}

fn parse(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|e| Error::new("catalog_corrupt", e.to_string()))
}

/// Python `transfer.sony_stage`: where a Sony card stands after its backup, from observations only.
pub fn sony_stage(state: &str, check: Option<&str>, removal_recorded: bool) -> &'static str {
    if check == Some("ready") || state == "completed" {
        return "formatted";
    }
    match check {
        Some("originals_present") => return "originals_reappeared",
        Some("waiting_camera") => return "removal_durable",
        _ => {}
    }
    if state == "failed" && removal_recorded {
        return "removal_recorded";
    }
    match state {
        "camera_action_required" => "removal_observed",
        "backup_verified" | "cleaning_source" => "backup_verified",
        _ => "",
    }
}

/// One row of the jobs query: id, state, error, updated_at, cleanup_profile, source_kind, source_volume_uuid,
/// release_stalker_source.
type JobRow = (
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);

/// Python `Catalog.jobs()`: the newest 200 jobs with what Actividad needs. Returns the rows and whether one of them
/// needed the unported `sony_removal_recorded` check.
pub fn jobs(catalog: &Catalog) -> Result<(Vec<Value>, bool)> {
    let Some(db) = catalog.db() else {
        return Ok((Vec::new(), false));
    };
    let mut statement = db
        .prepare(
            "SELECT id,state,error,updated_at,json_extract(plan,'$.cleanup_profile') AS cleanup_profile,\
             json_extract(plan,'$.source_kind') AS source_kind,\
             json_extract(plan,'$.source_identity.volume_uuid') AS source_volume_uuid,\
             json_extract(plan,'$.release_stalker_source') AS release_stalker_source FROM jobs ORDER BY created_at DESC LIMIT 200",
        )
        .map_err(sql)?;
    let rows: Vec<JobRow> = statement
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })
        .map_err(sql)?
        .collect::<std::result::Result<_, _>>()
        .map_err(sql)?;
    let mut unported = false;
    let mut values = Vec::with_capacity(rows.len());
    for (id, state, error, updated_at, cleanup_profile, source_kind, source_volume_uuid, release) in
        rows
    {
        let mut value = json!({
            "id": id, "state": state, "error": error, "updated_at": updated_at,
            "cleanup_profile": cleanup_profile, "source_kind": source_kind,
            "source_volume_uuid": source_volume_uuid,
            "release_stalker_source": release.unwrap_or(0) != 0,
            // A reminder choice only: the job keeps its state and can still resume.
            "dismissed": catalog.get(&format!("job-dismissed:{id}"))?.is_some_and(|v| v.as_bool().unwrap_or(false)),
        });
        if let Some(summary) = catalog.get(&format!("stalker-release:{id}"))? {
            value["stalker_release"] = summary;
        }
        if cleanup_profile.as_deref() == Some("sony_clips_then_format") {
            let check = catalog.get(&format!("sony-card-check:{id}"))?;
            if state == "failed" && error.as_deref() == Some("FileNotFoundError") {
                unported = true;
            }
            value["sony_stage"] = json!(sony_stage(
                &state,
                check.as_ref().and_then(Value::as_str),
                false
            ));
            value["sony_card_check"] = check.unwrap_or(Value::Null);
            value["sony_reappeared"] = catalog
                .get(&format!("sony-card-reappeared:{id}"))?
                .unwrap_or_else(|| json!([]));
        }
        values.push(value);
    }
    Ok((values, unported))
}

/// Python `Catalog.search(None, None)`: every asset with its work state and projects.
pub fn library(catalog: &Catalog) -> Result<Vec<Value>> {
    let Some(db) = catalog.db() else {
        return Ok(Vec::new());
    };
    let mut assets = db
        .prepare(
            "SELECT assets.payload,work.state FROM assets JOIN work ON work.asset_id=assets.id",
        )
        .map_err(sql)?;
    let rows: Vec<(String, String)> = assets
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql)?
        .collect::<std::result::Result<_, _>>()
        .map_err(sql)?;
    let mut projects = db
        .prepare(
            "SELECT projects.id,projects.title,projects.profile FROM projects JOIN asset_projects \
             ON asset_projects.project_id=projects.id WHERE asset_projects.asset_id=?",
        )
        .map_err(sql)?;
    let mut result = Vec::with_capacity(rows.len());
    for (payload, state) in rows {
        let mut value = parse(&payload)?;
        let id = value["id"].as_str().unwrap_or_default().to_owned();
        let linked: Vec<Value> = projects
            .query_map([&id], |r| {
                Ok(json!({"id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "profile": r.get::<_, String>(2)?}))
            })
            .map_err(sql)?
            .collect::<std::result::Result<_, _>>()
            .map_err(sql)?;
        value["work_delivery_status"] = json!(state);
        value["projects"] = Value::Array(linked);
        result.push(value);
    }
    Ok(result)
}

/// manager/inventory.py `KINDS`, read by layout.py `kind_of`.
const KINDS: [(&str, &[&str]); 5] = [
    ("video", &[".mov", ".mp4", ".mpg", ".mts", ".insv"]),
    (
        "image",
        &[".png", ".jpg", ".jpeg", ".gif", ".heic", ".insp"],
    ),
    ("raw_image", &[".cr2", ".cr3", ".nef", ".arw", ".dng"]),
    ("audio", &[".wav", ".mp3", ".caf", ".flac", ".aif", ".aiff"]),
    (
        "auxiliary",
        &[".xml", ".lrv", ".thm", ".gyro", ".aae", ".bmp", ".hprj"],
    ),
];
const MEDIA_KINDS: [&str; 4] = ["video", "audio", "image", "raw_image"];

/// Python `layout.kind_of`: camera structure is support even when it is a JPG.
pub fn kind_of(path: &str) -> Option<&'static str> {
    let lowered = format!("/{}", path.to_lowercase());
    if lowered.contains("/thmbnl/")
        || lowered.contains("/avf_info/")
        || lowered.contains("/private/sony/")
        || (lowered.contains("/m4root/") && !lowered.contains("/m4root/clip/"))
    {
        return Some("support");
    }
    let name = lowered.rsplit('/').next().unwrap_or("");
    // Python Path.suffix: the last dot, not a leading one.
    let suffix = name.rfind('.').filter(|i| *i > 0).map(|i| &name[i..])?;
    KINDS
        .iter()
        .find(|(_, suffixes)| suffixes.contains(&suffix))
        .map(|(kind, _)| *kind)
}

/// Python `library.projects`: projects with ingested editable sources, newest first.
pub fn projects(catalog: &Catalog, assets: &[Value]) -> Result<Vec<Value>> {
    let profiles = catalog
        .get("creative-profiles")?
        .unwrap_or_else(|| json!({}));
    let mut found: BTreeMap<String, (Value, BTreeSet<String>)> = BTreeMap::new();
    for asset in assets {
        let editable = asset["kind"] == "stalker_session"
            || asset["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|p| kind_of(p).is_some_and(|k| MEDIA_KINDS.contains(&k)));
        if !editable {
            continue; // Camera indexes/thumbnails are not sources.
        }
        let key = asset["project_id"].as_str().unwrap_or_default().to_owned();
        let profile = asset["profile"].clone();
        let entry = found.entry(key).or_insert_with(|| {
            let clipper = profile.as_str().map(|p| profiles[p]["clipper_profile"].clone()).unwrap_or(Value::Null);
            (
                json!({"project": asset["project"], "project_id": asset["project_id"], "profile": profile,
                       "clipper_profile": clipper, "assets": 0}),
                BTreeSet::new(),
            )
        });
        entry.0["assets"] = json!(entry.0["assets"].as_u64().unwrap_or(0) + 1);
        if let Some(job) = asset["job_id"].as_str() {
            entry.1.insert(job.to_owned());
        }
    }
    let db = catalog.db();
    let mut values = Vec::with_capacity(found.len());
    for (_, (mut entry, jobs)) in found {
        // Plans can reach 12 MiB and this runs on every snapshot: read only the timestamp.
        let mut newest: Option<String> = None;
        for job in jobs {
            let stamp: Option<String> = match db {
                Some(db) => db
                    .query_row("SELECT updated_at FROM jobs WHERE id=?", [&job], |r| {
                        r.get(0)
                    })
                    .optional()
                    .map_err(sql)?,
                None => None,
            };
            newest = newest.max(stamp);
        }
        entry["updated_at"] = json!(newest);
        values.push(entry);
    }
    values.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
    Ok(values)
}

/// Python `workspaces.profiles`: creative profiles sorted by id.
pub fn profiles(catalog: &Catalog) -> Result<Vec<Value>> {
    let mut values: Vec<Value> = catalog
        .get("creative-profiles")?
        .and_then(|v| v.as_object().cloned())
        .map(|m| m.into_iter().map(|(_, v)| v).collect())
        .unwrap_or_default();
    values.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(values)
}

/// Python `closure.overview`: closures saved per project, metadata only (never a claim that hashes still match).
pub fn project_closures(catalog: &Catalog) -> Result<Vec<Value>> {
    let Some(db) = catalog.db() else {
        return Ok(Vec::new());
    };
    let mut statement = db
        .prepare("SELECT key,value FROM metadata WHERE key LIKE 'project-closure:%'")
        .map_err(sql)?;
    let rows: Vec<(String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql)?
        .collect::<std::result::Result<_, _>>()
        .map_err(sql)?;
    let mut values = Vec::new();
    for (key, raw) in rows {
        let project_id = &key["project-closure:".len()..];
        // Python closure.project_record.
        let project: Option<(String, String, String)> = db
            .query_row(
                "SELECT id,profile,title FROM projects WHERE id=?",
                [project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(sql)?;
        let (id, profile, title) =
            project.ok_or_else(|| Error::new("unknown_project", "Proyecto desconocido"))?;
        nu9ve_json::require(
            title != "finales",
            "invalid_project",
            "Elige el proyecto original, no la colección finales",
        )?;
        let config = parse(&raw)?;
        let released = catalog
            .get(&format!("project-released:{project_id}"))?
            .unwrap_or(Value::Null);
        let finals = config["finals"].as_array().cloned().unwrap_or_default();
        let roles: BTreeSet<&str> = finals.iter().filter_map(|f| f["role"].as_str()).collect();
        let complete = config["required"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .all(|r| roles.contains(r));
        let truthy = |v: &Value| !(v.is_null() || v == &json!(false));
        let mut value = Map::new();
        value.insert("id".into(), json!(id));
        value.insert("profile".into(), json!(profile));
        value.insert("title".into(), json!(title));
        if let Some(object) = config.as_object() {
            for (k, v) in object {
                value.insert(k.clone(), v.clone());
            }
        }
        let needs =
            !truthy(&released) && truthy(&config["no_more"]) && complete && !finals.is_empty();
        value.insert("released".into(), released);
        value.insert("needs_release".into(), json!(needs));
        values.push(Value::Object(value));
    }
    Ok(values)
}

/// The `integration_settings` read (no params).
pub fn integration_settings(catalog: &Catalog) -> Result<Value> {
    let text = |key: &str| -> Result<Value> { Ok(catalog.get(key)?.unwrap_or_else(|| json!(""))) };
    Ok(json!({
        "clipper_executable": text("clipper-executable")?,
        "stalker_executable": text("stalker-executable")?,
        "capture_override": text("capture-override")?,
        "retain_stalker_source": catalog.get("retain-stalker-source")?.unwrap_or(json!(false)),
    }))
}

/// The `defaults` read (no params).
pub fn defaults(catalog: &Catalog) -> Result<Value> {
    Ok(catalog
        .get("defaults")?
        .unwrap_or_else(|| json!({"profile": "e64", "country": "mx", "city": "hermosillo"})))
}

/// Python `_snapshot(params)`. `backend` is None where volume methods are not ported (macOS).
pub fn snapshot(catalog: &Catalog, backend: Option<&dyn Backend>, params: &Value) -> Result<Value> {
    let mut errors = Map::new();
    let cached = backend.map(volumes::Cached::new);
    let mut volume_list = Vec::new();
    for record in catalog.volumes()? {
        let current = cached.as_ref().and_then(|b| volumes::online(b, &record));
        let mut record = record;
        record["online"] = json!(current.is_some());
        record["free"] = current.map(|c| c["free"].clone()).unwrap_or(Value::Null);
        volume_list.push(record);
    }
    if backend.is_none() {
        errors.insert(
            "volumes".into(),
            json!("not_ported: online state is still read by media/volumes.py on this platform"),
        );
    }
    let library = library(catalog)?;
    let (jobs, unported_sony) = jobs(catalog)?;
    if unported_sony {
        errors.insert(
            "jobs".into(),
            json!("not_ported: sony_removal_recorded for a Sony job that failed reading the card"),
        );
    }
    let projects = projects(catalog, &library)?;
    let release_plans =
        catalog.json_rows("SELECT value FROM metadata WHERE key LIKE 'release:%'", [])?;
    let mut result = json!({
        "volumes": volume_list, "library": library, "jobs": jobs, "profiles": profiles(catalog)?,
        "projects": projects, "release_plans": release_plans,
        "integration_settings": integration_settings(catalog)?, "defaults": defaults(catalog)?,
        "project_closures": project_closures(catalog)?,
        "capabilities": {},
    });
    errors.insert(
        "capabilities".into(),
        json!("not_ported: Clipper/Stalker discovery is still in media/integrations.py"),
    );
    if params["cards"].as_bool().unwrap_or(true) {
        match &cached {
            Some(b) => {
                let any_online = result["volumes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|v| v["online"] == true);
                match volumes::source_cards(catalog, b, Some(any_online)) {
                    Ok(cards) => result["source_cards"] = cards,
                    Err(e) => {
                        result["source_cards"] = json!([]);
                        errors.insert("source_cards".into(), json!(e.message));
                    }
                }
            }
            None => {
                result["source_cards"] = json!([]);
                errors.insert(
                    "source_cards".into(),
                    json!("not_ported: the volume backend of this platform is still Python"),
                );
            }
        }
    }
    result["errors"] = Value::Object(errors);
    Ok(result)
}

/// For tests and diagnosis: a path's kind as Python names it.
pub fn describe_kind(path: &Path) -> Option<&'static str> {
    kind_of(&path.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::fixture;

    #[test]
    fn kinds_follow_layout_py() {
        assert_eq!(kind_of("DCIM/100MSDCF/C0001.MP4"), Some("video"));
        assert_eq!(kind_of("PRIVATE/M4ROOT/CLIP/C0001.MP4"), Some("video"));
        assert_eq!(
            kind_of("PRIVATE/M4ROOT/THMBNL/C0001T01.JPG"),
            Some("support")
        );
        assert_eq!(kind_of("PRIVATE/M4ROOT/STATUS.BIN"), Some("support"));
        assert_eq!(kind_of("AVF_INFO/AVIN0001.INP"), Some("support"));
        assert_eq!(kind_of("a/b.DNG"), Some("raw_image"));
        assert_eq!(kind_of("a/.hidden"), None);
        assert_eq!(kind_of("a/notes"), None);
        assert_eq!(kind_of("x.XML"), Some("auxiliary"));
    }

    #[test]
    fn sony_stages_follow_transfer_py() {
        assert_eq!(sony_stage("completed", None, false), "formatted");
        assert_eq!(
            sony_stage("camera_action_required", Some("ready"), false),
            "formatted"
        );
        assert_eq!(
            sony_stage("camera_action_required", Some("originals_present"), false),
            "originals_reappeared"
        );
        assert_eq!(
            sony_stage("camera_action_required", Some("waiting_camera"), false),
            "removal_durable"
        );
        assert_eq!(sony_stage("failed", None, true), "removal_recorded");
        assert_eq!(
            sony_stage("camera_action_required", None, false),
            "removal_observed"
        );
        assert_eq!(
            sony_stage("cleaning_source", None, false),
            "backup_verified"
        );
        assert_eq!(sony_stage("failed", None, false), "");
    }

    fn catalog_with_data() -> (tempfile::TempDir, Catalog) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("state");
        let db = fixture::create(&home);
        db.execute_batch(
            "CREATE TABLE assets(id TEXT PRIMARY KEY, job_id TEXT NOT NULL, payload TEXT NOT NULL);
             CREATE TABLE work(asset_id TEXT PRIMARY KEY, state TEXT NOT NULL, target TEXT, updated_at TEXT NOT NULL);
             CREATE TABLE projects(id TEXT PRIMARY KEY, profile TEXT NOT NULL, title TEXT NOT NULL);
             CREATE TABLE asset_projects(asset_id TEXT NOT NULL, project_id TEXT NOT NULL, PRIMARY KEY(asset_id,project_id));
             INSERT INTO projects VALUES('p1','e64','proyecto-demo');
             INSERT INTO assets VALUES('a1','j1','{\"id\":\"a1\",\"job_id\":\"j1\",\"kind\":\"camera_clip\",\"paths\":[\"PRIVATE/M4ROOT/CLIP/C0001.MP4\"],\"project\":\"proyecto-demo\",\"project_id\":\"p1\",\"profile\":\"e64\"}');
             INSERT INTO assets VALUES('a2','j1','{\"id\":\"a2\",\"job_id\":\"j1\",\"kind\":\"camera_support\",\"paths\":[\"PRIVATE/M4ROOT/THMBNL/C0001T01.JPG\"],\"project\":\"proyecto-demo\",\"project_id\":\"p1\",\"profile\":\"e64\"}');
             INSERT INTO work VALUES('a1','ready',NULL,'2026-11-08');
             INSERT INTO work VALUES('a2','skipped',NULL,'2026-11-08');
             INSERT INTO asset_projects VALUES('a1','p1');
             INSERT INTO jobs VALUES('j1','d','{\"cleanup_profile\":\"sony_clips_then_format\",\"source_kind\":\"card\",\"source_identity\":{\"volume_uuid\":\"v1\"}}','camera_action_required',NULL,'2026-11-08T10:00:00Z','2026-11-08T11:00:00Z');
             INSERT INTO jobs VALUES('j2','d','{\"release_stalker_source\":true}','completed',NULL,'2026-11-07T10:00:00Z','2026-11-07T10:30:00Z');
             INSERT INTO metadata VALUES('job-dismissed:j2','true');
             INSERT INTO metadata VALUES('sony-card-check:j1','\"waiting_camera\"');
             INSERT INTO metadata VALUES('creative-profiles','{\"e64\":{\"id\":\"e64\",\"name\":\"E64\",\"raw_dir\":\"e64_raw\",\"clipper_profile\":\"clip-e64\"}}');
             INSERT INTO metadata VALUES('release:r1','{\"id\":\"r1\"}');
             INSERT INTO metadata VALUES('project-closure:p1','{\"required\":[\"horizontal\"],\"finals\":[{\"role\":\"horizontal\"}],\"no_more\":true}');",
        )
        .unwrap();
        drop(db);
        let catalog = Catalog::open(&home).unwrap();
        (temp, catalog)
    }

    #[test]
    fn snapshot_has_python_shape_and_says_what_is_not_ported() {
        let (_temp, catalog) = catalog_with_data();
        let data = snapshot(&catalog, None, &json!({})).unwrap();
        for key in [
            "volumes",
            "library",
            "jobs",
            "profiles",
            "projects",
            "release_plans",
            "integration_settings",
            "defaults",
            "errors",
            "project_closures",
            "capabilities",
            "source_cards",
        ] {
            assert!(data.get(key).is_some(), "{key}");
        }
        // Jobs: newest first, dismissal, Sony stage from the stored observation.
        assert_eq!(data["jobs"][0]["id"], "j1");
        assert_eq!(data["jobs"][0]["sony_stage"], "removal_durable");
        assert_eq!(data["jobs"][0]["source_volume_uuid"], "v1");
        assert_eq!(data["jobs"][1]["dismissed"], true);
        assert_eq!(data["jobs"][1]["release_stalker_source"], true);
        // Library and projects: the thumbnail is support, not a source.
        assert_eq!(data["library"].as_array().unwrap().len(), 2);
        assert_eq!(data["projects"].as_array().unwrap().len(), 1);
        assert_eq!(data["projects"][0]["assets"], 1);
        assert_eq!(data["projects"][0]["clipper_profile"], "clip-e64");
        assert_eq!(data["projects"][0]["updated_at"], "2026-11-08T11:00:00Z");
        assert_eq!(data["profiles"][0]["id"], "e64");
        assert_eq!(data["release_plans"], json!([{"id": "r1"}]));
        assert_eq!(data["defaults"]["city"], "hermosillo");
        assert_eq!(data["integration_settings"]["retain_stalker_source"], false);
        // Closure: complete, no_more and not released → needs release.
        assert_eq!(data["project_closures"][0]["title"], "proyecto-demo");
        assert_eq!(data["project_closures"][0]["needs_release"], true);
        // Unported pieces are named, the way Python names a failing piece.
        assert!(
            data["errors"]["capabilities"]
                .as_str()
                .unwrap()
                .starts_with("not_ported")
        );
        assert!(
            data["errors"]["source_cards"]
                .as_str()
                .unwrap()
                .starts_with("not_ported")
        );
        // Without cards requested there is no source_cards key at all.
        let lean = snapshot(&catalog, None, &json!({"cards": false})).unwrap();
        assert!(lean.get("source_cards").is_none());
    }

    #[test]
    fn an_empty_catalog_gives_an_empty_snapshot() {
        let data = snapshot(&Catalog::empty(), None, &json!({})).unwrap();
        assert_eq!(data["jobs"], json!([]));
        assert_eq!(data["library"], json!([]));
        assert_eq!(data["defaults"]["profile"], "e64");
    }
}
