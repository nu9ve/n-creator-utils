//! What machines ask of each other through the exchange (nu9ve-exchange-v1 §3, §7 and §7.1): leases (one
//! owner per project, handed over without clocks), requests, the small objects that answer them, and
//! what an app publishes for itself on its other machines (`apps/<app>/`).
//!
//! [`pass`] runs after each read: it answers the requests of paired machines, takes in the answers to this
//! machine's requests, lets go of leases another machine took, and forgets objects nobody asks for.

use super::suite::{valid_app as app_name, valid_device};
use super::{Agent, append_line, locked, publish_file, write_json};
use nu9ve_engine::RpcError;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Methods for apps, beyond the tools.
pub(super) const METHODS: [&str; 8] = [
    "offer",
    "unoffer",
    "shared",
    "share",
    "unshare",
    "app_publish",
    "app_append",
    "app_read",
];

/// §7: an object nobody asks for any more is forgotten after this long.
const FORGET_AFTER_SECS: i64 = 7 * 24 * 3600;

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new("invalid_params", message)
}

fn failed(e: io::Error) -> RpcError {
    RpcError::new("internal_error", e.to_string())
}

fn sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A request id: short, and safe as a key and in a file.
fn request_id(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A file an app publishes in `apps/<app>/`: a name, never a path.
pub(super) fn app_file(name: &str, extension: &str) -> bool {
    let Some(stem) = name.strip_suffix(extension) else {
        return false;
    };
    let b = stem.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_' | b'.')
        })
}

fn list(record: Option<Value>, field: &str) -> Vec<Value> {
    record
        .and_then(|r| r["body"][field].as_array().cloned())
        .unwrap_or_default()
}

/// The agent's private state: updated under the write lock.
fn update_state(a: &Agent, change: impl FnOnce(&mut Value)) -> io::Result<()> {
    locked(&a.dir, || {
        let mut state = a.state()?;
        change(&mut state);
        write_json(&a.dir.join("state.json"), &state)
    })
}

fn lease_key(app: &str, key: &str) -> String {
    format!("{app}/{key}")
}

/// `suite_lease_take`: this machine becomes the owner of `(app, project_key)` (§7.1).
pub(super) fn lease_take(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let (app, key) = lease_args(args)?;
    a.exchange().map_err(failed)?;
    let mut leases = list(a.own("leases.json"), "leases");
    if leases
        .iter()
        .any(|l| l["app"] == app && l["project_key"] == key)
    {
        return Ok(json!({"app": app, "project_key": key, "taken": false, "already_here": true}));
    }
    // From whom: the paired machine whose accepted leases.json has it, with that record's seq.
    let paired = a.paired().map_err(failed)?;
    let from = paired.keys().find_map(|id| {
        let record = a.accepted(&paired, id, "leases.json")?;
        let holds = record["body"]["leases"]
            .as_array()?
            .iter()
            .any(|l| l["app"] == app && l["project_key"] == key);
        holds.then(|| json!({"device_id": id, "seq": record["seq"]}))
    });
    let mut lease = json!({"app": app, "project_key": key, "since": nu9ve_device::now_utc()});
    if let Some(from) = &from {
        lease["from"] = from.clone();
    }
    leases.push(lease);
    let seq =
        publish_file(a, "leases.json", "leases", json!({"leases": leases})).map_err(failed)?;
    if let Some(seq) = seq {
        update_state(a, |s| s["lease_seq"][lease_key(app, key)] = json!(seq)).map_err(failed)?;
    }
    Ok(json!({"app": app, "project_key": key, "taken": true,
        "from_device_id": from.as_ref().map(|f| f["device_id"].clone())}))
}

fn lease_args(args: &Map<String, Value>) -> Result<(&str, &str), RpcError> {
    let app = args.get("app").and_then(Value::as_str).unwrap_or_default();
    let key = args
        .get("project_key")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !app_name(app) || !crate::catalog::valid_project_key(key) {
        return Err(invalid("Da app y project_key válidos."));
    }
    Ok((app, key))
}

/// `suite_lease_release`: this machine stops owning `(app, project_key)` without handing it to another.
pub(super) fn lease_release(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let (app, key) = lease_args(args)?;
    a.exchange().map_err(failed)?;
    let mut leases = list(a.own("leases.json"), "leases");
    let before = leases.len();
    leases.retain(|l| !(l["app"] == app && l["project_key"] == key));
    if leases.len() == before {
        return Ok(json!({"app": app, "project_key": key, "released": false}));
    }
    publish_file(a, "leases.json", "leases", json!({"leases": leases})).map_err(failed)?;
    update_state(a, |s| {
        if let Some(m) = s["lease_seq"].as_object_mut() {
            m.remove(&lease_key(app, key));
        }
    })
    .map_err(failed)?;
    Ok(json!({"app": app, "project_key": key, "released": true}))
}

/// Forgets what this machine asked of `device`: its pending requests and their answers, with the copies
/// received from it. Run when the device is unpaired: nothing it sent is trusted any more.
pub(super) fn forget_device(a: &Agent, device: &str) -> io::Result<()> {
    let state = a.state()?;
    let received: Vec<String> = state["answers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, v)| v["to"] == device)
        .filter_map(|(_, v)| v["received"].as_str().map(str::to_owned))
        .collect();
    for hex in received.iter().filter(|h| sha256_hex(h)) {
        let _ = fs::remove_file(received_dir(a).join(hex));
    }
    update_state(a, |s| {
        if let Some(m) = s["answers"].as_object_mut() {
            m.retain(|_, v| v["to"] != device);
        }
    })?;
    if a.exchange().is_ok() {
        let mut requests = list(a.own("requests.json"), "requests");
        let before = requests.len();
        requests.retain(|r| r["to"] != device);
        if requests.len() != before {
            publish_file(
                a,
                "requests.json",
                "requests",
                json!({"requests": requests}),
            )?;
        }
    }
    Ok(())
}

/// The answer this machine keeps for one of its requests, as `suite_pull` shows it.
fn answer_view(a: &Agent, id: &str, answer: &Value) -> Value {
    if let Some(hex) = answer["received"].as_str() {
        return json!({"request": id, "status": "received", "sha256": hex, "bytes": answer["bytes"],
            "media_type_untrusted": answer["media_type"], "path": received_dir(a).join(hex)});
    }
    let mut view = json!({"request": id, "status": "refused", "refused": answer["refused"], "where_untrusted": answer["where"]});
    // The disk by its name too, as this machine last saw it (its own sightings or the other's).
    if let Some(volume) = answer["where"]["volume_id"].as_str()
        && let Some(label) = volume_label(a, volume)
    {
        view["where_untrusted"]["label"] = label;
    }
    view
}

fn volume_label(a: &Agent, volume: &str) -> Option<Value> {
    let paired = a.paired().ok()?;
    let mut records: Vec<Value> = a.own("volumes.json").into_iter().collect();
    records.extend(
        paired
            .keys()
            .filter_map(|id| a.accepted(&paired, id, "volumes.json")),
    );
    records.iter().find_map(|r| {
        r["body"]["sightings"]
            .as_array()?
            .iter()
            .find(|s| s["volume_id"] == volume)
            .map(|s| s["label"].clone())
    })
}

/// A media type from the file's extension, for what `offer` publishes when the caller gives none.
fn media_type_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "geojson" => "application/geo+json",
        "json" => "application/json",
        "ndjson" | "jsonl" => "application/x-ndjson",
        "csv" => "text/csv",
        "txt" | "md" => "text/plain",
        "parquet" => "application/vnd.apache.parquet",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "tif" | "tiff" => "image/tiff",
        "dng" => "image/x-adobe-dng",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// `suite_pull`: asks a paired machine for an object or a dataset (§7). Asking again for the same thing
/// gives the state of the request.
pub(super) fn pull(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let from = args.get("from").and_then(Value::as_str).unwrap_or_default();
    if !valid_device(from) || !a.paired().map_err(failed)?.contains_key(from) {
        return Err(invalid("from es una máquina emparejada (su device_id)."));
    }
    let object = args.get("object").and_then(Value::as_str);
    let dataset = args.get("dataset").and_then(Value::as_str);
    let (field, wanted) = match (object, dataset) {
        (Some(o), None) if sha256_hex(o) => ("object", o),
        (None, Some(d)) if d.contains(':') && d.len() <= 161 => ("dataset", d),
        _ => {
            return Err(invalid(
                "Da object (sha256) o dataset (owner:id), uno de los dos.",
            ));
        }
    };
    if args.get("drop") == Some(&json!(true)) {
        return drop_pull(a, from, field, wanted);
    }
    let state = a.state().map_err(failed)?;
    if let Some((id, answer)) = state["answers"]
        .as_object()
        .into_iter()
        .flatten()
        .find(|(_, v)| v["to"] == from && v[field] == wanted)
    {
        return Ok(answer_view(a, id, answer));
    }
    let mut requests = list(a.own("requests.json"), "requests");
    if let Some(existing) = requests
        .iter()
        .find(|r| r["to"] == from && r[field] == wanted)
    {
        return Ok(json!({"request": existing["id"], "status": "pending"}));
    }
    let id = nu9ve_json::random_bytes::<8>()
        .map(|b| format!("r-{}", nu9ve_json::hex(&b)))
        .map_err(|e| RpcError::new("internal_error", e.message))?;
    let mut request = json!({"id": id, "to": from});
    request[field] = json!(wanted);
    requests.push(request);
    publish_file(
        a,
        "requests.json",
        "requests",
        json!({"requests": requests}),
    )
    .map_err(failed)?;
    Ok(json!({"request": id, "status": "pending"}))
}

/// `suite_pull` with `drop`: undoes a pull. The request goes, and the answer and copy if it came.
fn drop_pull(a: &Agent, from: &str, field: &str, wanted: &str) -> Result<Value, RpcError> {
    let state = a.state().map_err(failed)?;
    let answered: Vec<(String, Option<String>)> = state["answers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, v)| v["to"] == from && v[field] == wanted)
        .map(|(k, v)| (k.clone(), v["received"].as_str().map(str::to_owned)))
        .collect();
    let mut removed_copy = false;
    for (_, hex) in &answered {
        // Another answer may name the same object: the copy goes only when none is left.
        if let Some(hex) = hex.as_ref().filter(|h| sha256_hex(h)) {
            let others = state["answers"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(k, v)| {
                    v["received"] == hex.as_str() && !answered.iter().any(|(a, _)| a == *k)
                })
                .count();
            if others == 0 && fs::remove_file(received_dir(a).join(hex)).is_ok() {
                removed_copy = true;
            }
        }
    }
    update_state(a, |s| {
        if let Some(m) = s["answers"].as_object_mut() {
            m.retain(|k, _| !answered.iter().any(|(a, _)| a == k));
        }
    })
    .map_err(failed)?;
    let mut requests = list(a.own("requests.json"), "requests");
    let before = requests.len();
    requests.retain(|r| !(r["to"] == from && r[field] == wanted));
    if requests.len() != before {
        publish_file(
            a,
            "requests.json",
            "requests",
            json!({"requests": requests}),
        )
        .map_err(failed)?;
    }
    Ok(
        json!({"dropped": !answered.is_empty() || requests.len() != before, "removed_copy": removed_copy}),
    )
}

fn received_dir(a: &Agent) -> PathBuf {
    a.dir.join("received")
}

/// An object this machine holds locally (received, published or offered), as `suite_where` shows it.
pub(super) fn local_object(a: &Agent, sha: &str) -> Option<Value> {
    if !sha256_hex(sha) {
        return None;
    }
    if let Ok(meta) = fs::metadata(received_dir(a).join(sha)) {
        return Some(json!({"sha256": sha, "bytes": meta.len(), "received": true}));
    }
    offers(a)
        .into_iter()
        .find(|o| o["sha256"] == sha)
        .map(|o| json!({"sha256": sha, "bytes": o["bytes"], "offered": true}))
}

// --- Offers: what this machine can give when asked ---------------------------------------------------

fn offers(a: &Agent) -> Vec<Value> {
    super::read_json(&a.dir.join("offers.json"))
        .ok()
        .flatten()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

/// The bytes of a file of at most `limit` bytes, or `None` if it is larger or cannot be read.
fn read_small(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= limit).then_some(bytes))
}

/// `offer`: a file this machine gives when another asks for it, by its SHA-256 or by `dataset`. The file
/// stays where it is; only a file of at most 64 MiB is hashed and can be sent. A larger one is answered
/// with the disk it is on.
fn offer(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
    let path = PathBuf::from(path);
    let meta = fs::metadata(&path).ok().filter(|m| m.is_file());
    let (true, Some(meta)) = (path.is_absolute(), meta) else {
        return Err(invalid("path es la ruta absoluta de un archivo."));
    };
    let dataset = args.get("dataset").and_then(Value::as_str);
    if dataset.is_some_and(|d| !d.contains(':') || d.len() > 161) {
        return Err(invalid("dataset es owner:id."));
    }
    let media_type = args
        .get("media_type")
        .and_then(Value::as_str)
        .unwrap_or_else(|| media_type_of(&path));
    let sha = match read_small(&path, nu9ve_exchange::MAX_OBJECT).map_err(failed)? {
        Some(bytes) => Some(nu9ve_json::sha256_hex(&bytes)),
        None if dataset.is_none() => {
            return Err(invalid(
                "Un archivo de más de 64 MiB se ofrece por su dataset (owner:id).",
            ));
        }
        None => None,
    };
    let entry = json!({"sha256": sha, "bytes": meta.len(), "media_type": media_type, "path": path,
        "dataset": dataset, "offered_at": nu9ve_device::now_utc()});
    locked(&a.dir, || {
        let mut all = offers(a);
        all.retain(|o| o["path"] != entry["path"]);
        all.push(entry.clone());
        write_json(&a.dir.join("offers.json"), &json!(all))
    })
    .map_err(failed)?;
    Ok(json!({"offered": true, "sha256": sha, "bytes": meta.len(), "dataset": dataset}))
}

/// `unoffer`: the file is no longer given. What was already sent stays until nobody asks for it.
fn unoffer(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
    if !Path::new(path).is_absolute() {
        return Err(invalid("path es la ruta absoluta de un archivo ofrecido."));
    }
    let removed = locked(&a.dir, || {
        let mut all = offers(a);
        let before = all.len();
        all.retain(|o| o["path"] != path);
        write_json(&a.dir.join("offers.json"), &json!(all))?;
        Ok(before != all.len())
    })
    .map_err(failed)?;
    Ok(json!({"unoffered": removed}))
}

/// Unshare (export §5): the files offered for `dataset` are no longer given. Returns how many went.
pub(super) fn unoffer_dataset(a: &Agent, dataset: &str) -> io::Result<usize> {
    locked(&a.dir, || {
        let mut all = offers(a);
        let before = all.len();
        all.retain(|o| o["dataset"] != dataset);
        if all.len() != before {
            write_json(&a.dir.join("offers.json"), &json!(all))?;
        }
        Ok(before - all.len())
    })
}

/// `shared`: what the person marked «available on all my machines», only `app`'s if given (export §5).
fn shared(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let app = args.get("app").and_then(Value::as_str);
    if args.get("app").is_some() && !app.is_some_and(app_name) {
        return Err(invalid("app es el nombre de una app."));
    }
    let mut list: Vec<String> = a
        .shared()
        .map_err(failed)?
        .into_iter()
        .filter(|d| app.is_none_or(|app| d.split_once(':').is_some_and(|(o, _)| o == app)))
        .collect();
    list.sort();
    Ok(json!({"app": app, "shared": list}))
}

/// `share` and `unshare`, as `nu9ve agent share`: the mark stays in the agent.
fn set_shared(a: &Agent, args: &Map<String, Value>, on: bool) -> Result<Value, RpcError> {
    let dataset = args
        .get("dataset")
        .and_then(Value::as_str)
        .unwrap_or_default();
    super::set_shared(a, dataset, on).map_err(|e| {
        if e.kind() == io::ErrorKind::InvalidInput {
            invalid("dataset es owner:id, como en su catálogo.")
        } else {
            failed(e)
        }
    })
}

/// Where a file is, for a `too_large` answer: the disk with a marker it is on and the path inside it, or
/// no disk and its path on this machine.
fn where_is(path: &Path) -> Value {
    let mounts = std::env::var("NU9VE_VOLUMES_DIR").unwrap_or_else(|_| "/Volumes".into());
    for entry in fs::read_dir(mounts).into_iter().flatten().flatten() {
        let mount = entry.path();
        let Ok(rest) = path.strip_prefix(&mount) else {
            continue;
        };
        let marker = fs::read(mount.join(".nu9ve/volume.json"))
            .ok()
            .and_then(|b| nu9ve_volumes::marker::parse_marker(&b).ok());
        if let Some(marker) = marker {
            return json!({"volume_id": marker["volume_id"], "path": rest.to_string_lossy()});
        }
    }
    json!({"volume_id": null, "path": path.to_string_lossy()})
}

/// What this machine holds for a request.
enum Held {
    /// `{sha256, bytes, media_type, where}` (or `{refused}`), and the offered file to copy if it is small.
    Yes(Value, Option<PathBuf>),
    /// What the owner engine exported (export §2), and its file: copied only if its hash is the one given.
    Exported(Value, PathBuf),
    No,
    /// It has it (a dataset of its catalog) but cannot give it yet.
    NotYet,
}

/// The dataset `owner:id` as this machine's published catalogs list it, with its catalog.
fn listed_dataset(a: &Agent, dataset: &str) -> Option<(Value, Value)> {
    for file in a.listed(None, "catalog") {
        let Some(record) = a.own(&file) else {
            continue;
        };
        let catalog = &record["body"]["catalog"];
        for d in catalog["datasets"].as_array().into_iter().flatten() {
            let id = format!(
                "{}:{}",
                d["owner"].as_str().unwrap_or_default(),
                d["id"].as_str().unwrap_or_default()
            );
            if id == dataset {
                return Some((d.clone(), catalog.clone()));
            }
        }
    }
    None
}

/// Exchange §6 and export §4: a personal dataset goes whole only if the person shared it. A dataset no
/// catalog of this machine lists is what whoever offered it decided.
fn not_shared(a: &Agent, dataset: &str, shared: &HashSet<String>) -> bool {
    !shared.contains(dataset)
        && listed_dataset(a, dataset).is_some_and(|(d, _)| d["privacy"] == "personal")
}

fn refused(reason: &str) -> Held {
    Held::Yes(json!({"refused": reason}), None)
}

/// Export §3, step 5: asks the owner engine for the dataset, if it is live with `data_export_v1`.
fn exported(owner: &str, id: &str) -> Held {
    if owner == super::suite::APP {
        return Held::NotYet;
    }
    let record = nu9ve_ipc::default_dir()
        .and_then(|dir| nu9ve_ipc::read_entry(&dir, owner).ok().flatten())
        .and_then(|entry| match entry.state {
            nu9ve_ipc::State::Live(record) => Some(record),
            _ => None,
        });
    let Some(reply) = record.and_then(|r| crate::export::fetch(&r, id, nu9ve_exchange::MAX_OBJECT))
    else {
        return Held::NotYet;
    };
    match crate::export::read_status(id, &reply, nu9ve_exchange::MAX_OBJECT) {
        "ready" => {
            let path = PathBuf::from(reply["path"].as_str().unwrap_or_default());
            Held::Exported(
                json!({"sha256": reply["sha256"], "bytes": reply["bytes"], "media_type": reply["media_type"],
                    "where": {"volume_id": null, "path": path}}),
                path,
            )
        }
        "too_large" => Held::Yes(
            json!({"sha256": null, "bytes": null, "media_type": null, "where": reply["where"]}),
            None,
        ),
        "not_found" => Held::No,
        _ => Held::NotYet,
    }
}

fn held_for(a: &Agent, request: &Value, offered: &[Value], shared: &HashSet<String>) -> Held {
    let from_offer = |o: &Value| {
        let path = PathBuf::from(o["path"].as_str().unwrap_or_default());
        Held::Yes(
            json!({"sha256": o["sha256"], "bytes": o["bytes"], "media_type": o["media_type"], "where": where_is(&path)}),
            Some(path),
        )
    };
    if let Some(sha) = request["object"].as_str() {
        if !sha256_hex(sha) {
            return Held::No;
        }
        let Some(o) = offered.iter().find(|o| o["sha256"] == sha) else {
            return Held::No;
        };
        // An object offered for a personal dataset follows the dataset's rule.
        if o["dataset"]
            .as_str()
            .is_some_and(|d| not_shared(a, d, shared))
        {
            return refused("not_shared");
        }
        return from_offer(o);
    }
    let Some(dataset) = request["dataset"].as_str() else {
        return Held::No;
    };
    // Export §3, in order: not shared, offered, not listed, too large by its catalog, its engine.
    let listed = listed_dataset(a, dataset);
    if !shared.contains(dataset)
        && listed
            .as_ref()
            .is_some_and(|(d, _)| d["privacy"] == "personal")
    {
        return refused("not_shared");
    }
    if let Some(o) = offered.iter().find(|o| o["dataset"] == dataset) {
        return from_offer(o);
    }
    let Some((d, catalog)) = listed else {
        return Held::No;
    };
    if let Some(bytes) = d["size"]["bytes"]
        .as_u64()
        .filter(|b| *b > nu9ve_exchange::MAX_OBJECT)
    {
        let root = d["location"]["root_id"].as_str();
        let volume = catalog["roots"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["id"].as_str() == root)
            .map_or(Value::Null, |r| r["volume_id"].clone());
        return Held::Yes(
            json!({"sha256": null, "bytes": bytes, "media_type": null,
                "where": {"volume_id": volume, "path": d["location"]["path"].as_str().unwrap_or_default()}}),
            None,
        );
    }
    exported(
        d["owner"].as_str().unwrap_or_default(),
        d["id"].as_str().unwrap_or_default(),
    )
}

/// Copies a small file into `dst` only if its SHA-256 is `sha`.
fn copy_verified(src: &Path, dst: &Path, sha: &str) -> io::Result<bool> {
    let Some(bytes) = read_small(src, nu9ve_exchange::MAX_OBJECT)? else {
        return Ok(false);
    };
    if nu9ve_json::sha256_hex(&bytes) != sha {
        return Ok(false);
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    nu9ve_fs::atomic_write_with(dst, &bytes, nu9ve_fs::SyncMode::Plain)?;
    Ok(true)
}

/// §7, the asked machine: answers what paired machines ask of this one, and forgets what nobody asks for.
fn answer_requests(a: &Agent) -> io::Result<Vec<String>> {
    let me = a.device_id().to_owned();
    let paired = a.paired()?;
    let folder = a.folder()?;
    let mut objects = list(a.own("objects.json"), "objects");
    let before = objects.clone();
    let offered = offers(a);
    let shared = a.shared()?;
    let mut asked = HashSet::new();
    let mut answered = Vec::new();
    for id in paired.keys() {
        let Some(record) = a.accepted(&paired, id, "requests.json") else {
            continue;
        };
        for request in record["body"]["requests"].as_array().into_iter().flatten() {
            let Some(rid) = request["id"].as_str().filter(|r| request_id(r)) else {
                continue;
            };
            if request["to"] != me.as_str() {
                continue;
            }
            asked.insert(rid.to_owned());
            if objects.iter().any(|o| o["request"] == rid) {
                continue;
            }
            let entry = match held_for(a, request, &offered, &shared) {
                Held::NotYet => continue,
                Held::No => nu9ve_exchange::object_answer(rid, None),
                Held::Exported(held, path) => {
                    let entry = nu9ve_exchange::object_answer(rid, Some(&held));
                    let Some(sha) = entry["sha256"].as_str() else {
                        continue;
                    };
                    let target = folder.join("objects/sha256").join(sha);
                    // Gone, or not the bytes it announced: asked again on the next pass (export §2).
                    if !target.exists() && !copy_verified(&path, &target, sha).unwrap_or(false) {
                        continue;
                    }
                    entry
                }
                Held::Yes(held, path) => {
                    let mut entry = nu9ve_exchange::object_answer(rid, Some(&held));
                    if let (Some(sha), Some(path)) = (entry["sha256"].as_str(), path) {
                        let target = folder.join("objects/sha256").join(sha);
                        // A file that changed since it was offered is not what was offered.
                        if !target.exists() && !copy_verified(&path, &target, sha)? {
                            entry = nu9ve_exchange::object_answer(rid, None);
                        }
                    }
                    entry
                }
            };
            answered.push(rid.to_owned());
            objects.push(entry);
        }
    }
    // Answers nobody asks for any more are forgotten after a while, with their objects (§7).
    let now = nu9ve_device::unix_now();
    let mut state = a.state()?;
    let mut keep = Vec::new();
    for o in objects {
        let rid = o["request"].as_str().unwrap_or_default().to_owned();
        if asked.contains(&rid) || o["request"].is_null() {
            if let Some(m) = state["unrequested"].as_object_mut() {
                m.remove(&rid);
            }
            keep.push(o);
            continue;
        }
        let since = state["unrequested"][&rid].as_i64().unwrap_or(now);
        if now - since < FORGET_AFTER_SECS {
            state["unrequested"][&rid] = json!(since);
            keep.push(o);
        } else if let Some(m) = state["unrequested"].as_object_mut() {
            m.remove(&rid);
        }
    }
    let unrequested = match &state["unrequested"] {
        Value::Object(m) => Value::Object(m.clone()),
        _ => json!({}),
    };
    update_state(a, |s| s["unrequested"] = unrequested)?;
    if keep != before {
        // Objects no entry names any more go too.
        let named: HashSet<&str> = keep.iter().filter_map(|o| o["sha256"].as_str()).collect();
        for entry in fs::read_dir(folder.join("objects/sha256"))
            .into_iter()
            .flatten()
            .flatten()
        {
            if !named.contains(entry.file_name().to_string_lossy().as_ref()) {
                let _ = fs::remove_file(entry.path());
            }
        }
        publish_file(a, "objects.json", "objects", json!({"objects": keep}))?;
    }
    Ok(answered)
}

/// §7, the machine that asked: takes in the answers to its requests, verified, and drops the requests.
fn take_answers(a: &Agent) -> io::Result<Vec<String>> {
    let paired = a.paired()?;
    let root = a.exchange()?;
    let requests = list(a.own("requests.json"), "requests");
    let mut still = Vec::new();
    let mut answers = Map::new();
    for request in &requests {
        let (Some(rid), Some(to)) = (request["id"].as_str(), request["to"].as_str()) else {
            continue;
        };
        let objects = a
            .accepted(&paired, to, "objects.json")
            .map(|r| list(Some(r), "objects"))
            .unwrap_or_default();
        let Some(entry) = objects.iter().find(|o| o["request"] == rid) else {
            still.push(request.clone());
            continue;
        };
        let mut answer = json!({"to": to, "object": request["object"], "dataset": request["dataset"],
            "at": nu9ve_device::now_utc()});
        if let Some(sha) = entry["sha256"].as_str().filter(|s| sha256_hex(s)) {
            let src = root
                .join("devices")
                .join(to)
                .join("objects/sha256")
                .join(sha);
            // Valid only if its hash is the one announced (§3); otherwise it waits for a good copy.
            if !copy_verified(&src, &received_dir(a).join(sha), sha)? {
                still.push(request.clone());
                continue;
            }
            answer["received"] = json!(sha);
            answer["bytes"] = entry["bytes"].clone();
            answer["media_type"] = entry["media_type"].clone();
        } else {
            let refused = entry["refused"].as_str().unwrap_or("not_found");
            // An unknown refusal is not_found (§7).
            let refused = if matches!(refused, "too_large" | "not_shared") {
                refused
            } else {
                "not_found"
            };
            answer["refused"] = json!(refused);
            answer["where"] = entry["where"].clone();
        }
        answers.insert(rid.to_owned(), answer);
    }
    if answers.is_empty() {
        return Ok(Vec::new());
    }
    update_state(a, |s| {
        for (k, v) in &answers {
            s["answers"][k] = v.clone();
        }
    })?;
    publish_file(a, "requests.json", "requests", json!({"requests": still}))?;
    Ok(answers.keys().cloned().collect())
}

/// §7.1: lets go of the leases another paired machine took from this one.
fn release_leases(a: &Agent) -> io::Result<Vec<String>> {
    let me = a.device_id().to_owned();
    let paired = a.paired()?;
    let state = a.state()?;
    let mine = list(a.own("leases.json"), "leases");
    let mut released = Vec::new();
    let kept: Vec<Value> = mine
        .into_iter()
        .filter(|lease| {
            let (app, key) = (
                lease["app"].as_str().unwrap_or_default(),
                lease["project_key"].as_str().unwrap_or_default(),
            );
            let taken = state["lease_seq"][lease_key(app, key)]
                .as_u64()
                .unwrap_or(0);
            let gone = paired.keys().any(|other| {
                let Some(record) = a.accepted(&paired, other, "leases.json") else {
                    return false;
                };
                let their_seq = record["seq"].as_u64().unwrap_or(0);
                record["body"]["leases"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|t| t["app"] == app && t["project_key"] == key)
                    .any(|t| {
                        nu9ve_exchange::lease_outcome(
                            &me,
                            taken,
                            lease.get("from"),
                            other,
                            t,
                            their_seq,
                        ) == nu9ve_exchange::LeaseOutcome::Release
                    })
            });
            if gone {
                released.push(lease_key(app, key));
            }
            !gone
        })
        .collect();
    if !released.is_empty() {
        publish_file(a, "leases.json", "leases", json!({"leases": kept}))?;
        update_state(a, |s| {
            if let Some(m) = s["lease_seq"].as_object_mut() {
                for k in &released {
                    m.remove(k);
                }
            }
        })?;
    }
    Ok(released)
}

/// One pass after reading the paired machines. Each part goes on if another fails.
pub(super) fn pass(a: &Agent) -> Value {
    let mut out = Map::new();
    for (name, result) in [
        ("answered", answer_requests(a)),
        ("received", take_answers(a)),
        ("released", release_leases(a)),
    ] {
        out.insert(
            name.into(),
            match result {
                Ok(list) => json!(list),
                Err(e) => json!({"error": e.to_string()}),
            },
        );
    }
    Value::Object(out)
}

// --- apps/<app>/ ----------------------------------------------------------------------------------

fn app_args<'a>(
    args: &'a Map<String, Value>,
    extension: &str,
) -> Result<(&'a str, &'a str), RpcError> {
    let app = args.get("app").and_then(Value::as_str).unwrap_or_default();
    let file = args.get("file").and_then(Value::as_str).unwrap_or_default();
    if !app_name(app) || !app_file(file, extension) {
        return Err(invalid(format!(
            "Da app y file (un nombre que termina en {extension})."
        )));
    }
    Ok((app, file))
}

fn body(args: &Map<String, Value>) -> Result<Value, RpcError> {
    args.get("body")
        .filter(|b| b.is_object())
        .cloned()
        .ok_or_else(|| invalid("body es un objeto."))
}

/// What the other machines published for `app` (and only `file`, if given).
fn app_read(a: &Agent, args: &Map<String, Value>) -> Result<Value, RpcError> {
    let app = args.get("app").and_then(Value::as_str).unwrap_or_default();
    let only = args.get("file").and_then(Value::as_str);
    if !app_name(app) {
        return Err(invalid("Da app."));
    }
    let paired = a.paired().map_err(failed)?;
    let mut out = Vec::new();
    for id in super::sorted(paired.keys()) {
        let name = a
            .accepted(&paired, id, "device.json")
            .map_or(Value::Null, |r| r["body"]["name"].clone());
        for file in a.listed(Some(id), &format!("apps/{app}")) {
            let short = file.rsplit('/').next().unwrap_or_default();
            if only.is_some_and(|o| o != short) {
                continue;
            }
            let mut row = json!({"device_id": id, "device_name_untrusted": name, "file": short});
            if file.ends_with(".ndjson") {
                let lines: Vec<Value> = super::accepted_lines(a, &paired, id, &file)
                    .into_iter()
                    .map(|r| json!({"seq": r["seq"], "written_at": r["written_at"], "body_untrusted": r["body"]}))
                    .collect();
                row["lines"] = json!(lines);
            } else if let Some(r) = a.accepted(&paired, id, &file) {
                row["written_at"] = r["written_at"].clone();
                row["body_untrusted"] = r["body"].clone();
            } else {
                continue;
            }
            out.push(row);
        }
    }
    Ok(json!({"app": app, "files": out}))
}

/// Methods for apps that are not tools.
pub(super) fn method(a: &Agent, name: &str, args: &Map<String, Value>) -> Result<Value, RpcError> {
    match name {
        "offer" => offer(a, args),
        "unoffer" => unoffer(a, args),
        "shared" => shared(a, args),
        "share" => set_shared(a, args, true),
        "unshare" => set_shared(a, args, false),
        "app_publish" => {
            let (app, file) = app_args(args, ".json")?;
            a.exchange().map_err(failed)?;
            let seq = publish_file(a, &format!("apps/{app}/{file}"), "app", body(args)?)
                .map_err(failed)?;
            Ok(json!({"app": app, "file": file, "written": seq.is_some(), "seq": seq}))
        }
        "app_append" => {
            let (app, file) = app_args(args, ".ndjson")?;
            a.exchange().map_err(failed)?;
            let seq = append_line(a, &format!("apps/{app}/{file}"), body(args)?).map_err(|e| {
                if e.kind() == io::ErrorKind::InvalidInput {
                    invalid(e.to_string())
                } else {
                    failed(e)
                }
            })?;
            Ok(json!({"app": app, "file": file, "seq": seq}))
        }
        "app_read" => app_read(a, args),
        _ => Err(RpcError::new("unknown_method", "No such method.")),
    }
}
