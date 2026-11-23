//! `nu9ve agent`: the suite's agent (nu9ve-agent-v1), command by command. It keeps the device identity
//! (nu9ve-device-v1) and its Ed25519 key, writes this machine's folder of the exchange (nu9ve-exchange-v1)
//! and reads the folders of paired machines. `serve` runs it in the background (serve.rs).

pub mod install;
mod serve;
// The suite engine serves a Unix socket; Windows has no named pipe server yet (not verified).
#[cfg_attr(not(unix), allow(dead_code, unused_imports))]
mod suite;
#[cfg_attr(not(unix), allow(dead_code, unused_imports))]
mod trade;

use nu9ve_exchange::{Key, State};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "uso: nu9ve agent init
     nu9ve agent name <NOMBRE>
     nu9ve agent exchange <CARPETA>
     nu9ve agent publish
     nu9ve agent read
     nu9ve agent pair <DEVICE_ID> <HUELLA>
     nu9ve agent unpair <DEVICE_ID>
     nu9ve agent share <OWNER:ID> | unshare <OWNER:ID>
     nu9ve agent status
     nu9ve agent serve
     nu9ve agent sync
     nu9ve agent call <MÉTODO> [JSON]
     nu9ve agent offer <ARCHIVO> [--dataset <OWNER:ID>] | unoffer <ARCHIVO>
     nu9ve agent install [--by <APP>] | uninstall | state";

/// Files of a machine's folder with a fixed name (§1), read from the other machines.
const OWN_FILES: [&str; 5] = [
    "device.json",
    "volumes.json",
    "leases.json",
    "requests.json",
    "objects.json",
];

/// nu9ve-agent-v1 §1: the agent folder, or `None`.
pub fn agent_dir() -> Option<PathBuf> {
    if let Ok(over) = std::env::var("NU9VE_AGENT_DIR") {
        return Path::new(&over).is_absolute().then(|| PathBuf::from(over));
    }
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var("LOCALAPPDATA").ok()?).join("nu9ve")
    } else {
        PathBuf::from(std::env::var("HOME").ok()?).join("Library/Application Support/nu9ve")
    };
    Some(base.join("agent"))
}

fn qa() -> bool {
    std::env::var_os("NU9VE_AGENT_DIR").is_some()
}

fn other(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn read_json(path: &Path) -> io::Result<Option<Value>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes)
                .map_err(|e| other(format!("{}: {e}", path.display())))?,
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    nu9ve_fs::atomic_write_with(path, &bytes, nu9ve_fs::SyncMode::Plain).map_err(io::Error::from)
}

/// Every read-modify-write of the agent's files (the `seq`, what was accepted) holds this lock, so a
/// command run by hand while the service runs never reuses a `seq`.
fn locked<T>(dir: &Path, work: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    fs::create_dir_all(dir)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("write.lock"))?;
    file.lock()?;
    let out = work();
    let _ = file.unlock();
    out
}

/// The key of this device: in QA a file in the agent folder, otherwise the system keychain (§4).
mod keystore {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    const SERVICE: &str = "xyz.nu9ve.agent";

    fn decode(text: &str) -> io::Result<[u8; 32]> {
        STANDARD
            .decode(text.trim())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| other("the stored key is not 32 bytes"))
    }

    fn qa_file(dir: &Path) -> PathBuf {
        dir.join("key.qa")
    }

    pub fn load(dir: &Path, device_id: &str) -> io::Result<Option<Key>> {
        if qa() {
            return match fs::read_to_string(qa_file(dir)) {
                Ok(text) => Ok(Some(Key::from_secret(&decode(&text)?))),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            };
        }
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::get_generic_password(SERVICE, device_id) {
                Ok(bytes) => Ok(Some(Key::from_secret(&decode(&String::from_utf8_lossy(
                    &bytes,
                ))?))),
                Err(e) if e.code() == -25300 => Ok(None), // errSecItemNotFound
                Err(e) => Err(other(format!("keychain: {e}"))),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = device_id;
            Err(other(
                "the keychain of this system is not supported yet; use NU9VE_AGENT_DIR for QA",
            ))
        }
    }

    pub fn store(dir: &Path, device_id: &str, key: &Key) -> io::Result<()> {
        let text = STANDARD.encode(key.secret());
        if qa() {
            fs::create_dir_all(dir)?;
            let path = qa_file(dir);
            let mut file = nu9ve_fs::create_new(&path)?;
            io::Write::write_all(&mut file, text.as_bytes())?;
            return file.sync_all();
        }
        #[cfg(target_os = "macos")]
        {
            security_framework::passwords::set_generic_password(SERVICE, device_id, text.as_bytes())
                .map_err(|e| other(format!("keychain: {e}")))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = device_id;
            Err(other(
                "the keychain of this system is not supported yet; use NU9VE_AGENT_DIR for QA",
            ))
        }
    }
}

struct Agent {
    dir: PathBuf,
    device_file: PathBuf,
    device: Value,
    key: Key,
}

impl Agent {
    /// device.json again, for a long-running service: the person may rename the device meanwhile.
    /// A file that is no longer valid, or now names another device, keeps what was read before.
    fn refresh(&mut self) {
        let platform = nu9ve_device::platform_id();
        if let Ok(bytes) = fs::read(&self.device_file)
            && let nu9ve_device::State::Ok(device) =
                nu9ve_device::state(Some(&bytes), platform.as_deref())
            && device["device_id"] == self.device["device_id"]
        {
            self.device = device;
        }
    }

    fn device_id(&self) -> &str {
        self.device["device_id"].as_str().unwrap_or_default()
    }

    fn config(&self) -> io::Result<Value> {
        Ok(read_json(&self.dir.join("config.json"))?.unwrap_or_else(|| json!({})))
    }

    fn exchange(&self) -> io::Result<PathBuf> {
        self.config()?["exchange"]
            .as_str()
            .map(PathBuf::from)
            .ok_or_else(|| other("no exchange folder yet: nu9ve agent exchange <CARPETA>"))
    }

    fn state(&self) -> io::Result<Value> {
        Ok(read_json(&self.dir.join("state.json"))?
            .unwrap_or_else(|| json!({"seq": 0, "accepted": {}})))
    }

    fn paired(&self) -> io::Result<HashMap<String, [u8; 32]>> {
        let value = read_json(&self.dir.join("paired.json"))?.unwrap_or_else(|| json!({}));
        Ok(value
            .as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| {
                        Some((k.clone(), nu9ve_exchange::decode_public(v.as_str()?)?))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn shared(&self) -> io::Result<HashSet<String>> {
        let value = read_json(&self.dir.join("shared.json"))?.unwrap_or_else(|| json!([]));
        Ok(value
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// The device and its key, created the first time.
fn open(create: bool) -> io::Result<(Agent, Value)> {
    let dir = agent_dir().ok_or_else(|| {
        other("no agent folder: NU9VE_AGENT_DIR is not absolute, or there is no home")
    })?;
    let device_file = nu9ve_device::default_file()
        .ok_or_else(|| other("no device file: NU9VE_DEVICE_FILE is not absolute"))?;
    if qa() && std::env::var_os("NU9VE_DEVICE_FILE").is_none() {
        return Err(other(
            "in QA (NU9VE_AGENT_DIR) the device file is NU9VE_DEVICE_FILE too, never the person's",
        ));
    }
    let platform = nu9ve_device::platform_id();
    let (device, outcome) = if create {
        nu9ve_device::load_or_create(&device_file, platform.as_deref())?
    } else {
        let bytes = fs::read(&device_file).map_err(|_| other("no device yet: nu9ve agent init"))?;
        match nu9ve_device::state(Some(&bytes), platform.as_deref()) {
            nu9ve_device::State::Ok(v) => (v, nu9ve_device::Outcome::Existing),
            nu9ve_device::State::Copied(_) => {
                return Err(other(
                    "device.json was copied from another machine: nu9ve agent init",
                ));
            }
            nu9ve_device::State::None => {
                return Err(other("device.json is not valid; it is left as it is"));
            }
        }
    };
    let id = device["device_id"].as_str().unwrap_or_default().to_owned();
    let key = match keystore::load(&dir, &id)? {
        Some(key) => key,
        None if create => {
            let key = Key::generate().map_err(|e| other(e.message))?;
            keystore::store(&dir, &id, &key)?;
            key
        }
        None => return Err(other("no key for this device yet: nu9ve agent init")),
    };
    let outcome = match outcome {
        nu9ve_device::Outcome::Existing => "existing",
        nu9ve_device::Outcome::Created => "created",
        nu9ve_device::Outcome::ReplacedCopy => "replaced_copy",
    };
    Ok((
        Agent {
            dir,
            device_file,
            device,
            key,
        },
        json!(outcome),
    ))
}

/// §5 of the exchange: writes one record if its body changed, with the next `seq`.
fn publish_record(
    agent: &Agent,
    state: &mut Value,
    folder: &Path,
    file: &str,
    kind: &str,
    body: Value,
) -> io::Result<bool> {
    let path = folder.join(file);
    if let Some(old) = read_json(&path)?
        && old["body"] == body
    {
        return Ok(false);
    }
    let seq = state["seq"].as_u64().unwrap_or(0) + 1;
    let record = nu9ve_exchange::record(
        &agent.key,
        agent.device_id(),
        kind,
        seq,
        &nu9ve_device::now_utc(),
        body,
    );
    let bytes = nu9ve_json::canonical(&record);
    if bytes.len() > nu9ve_exchange::MAX_JSON {
        return Err(other(format!("{file} would pass the 16 MiB limit")));
    }
    // seq is saved before the file: a crash after it skips a number, never reuses one.
    state["seq"] = json!(seq);
    write_json(&agent.dir.join("state.json"), state)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    nu9ve_fs::atomic_write_with(&path, &bytes, nu9ve_fs::SyncMode::Plain)?;
    Ok(true)
}

/// nu9ve-volume-v1 §4 as seen from here: every mounted volume with a valid marker.
fn sightings(previous: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let now = nu9ve_device::now_utc();
    // QA lists its own mounts: never the person's disks.
    let mounts = std::env::var("NU9VE_VOLUMES_DIR").unwrap_or_else(|_| "/Volumes".into());
    let Ok(entries) = fs::read_dir(mounts) else {
        return out;
    };
    for entry in entries.flatten() {
        let mount = entry.path();
        let Ok(bytes) = fs::read(mount.join(".nu9ve/volume.json")) else {
            continue;
        };
        let Ok(marker) = nu9ve_volumes::marker::parse_marker(&bytes) else {
            continue;
        };
        let volume_id = marker["volume_id"].as_str().unwrap_or_default().to_owned();
        #[cfg(target_os = "macos")]
        let serial = nu9ve_volumes::macos::diskutil(
            &["info", "-plist", &mount.to_string_lossy()],
            std::time::Duration::from_secs(10),
        )
        .ok()
        .and_then(|info| info["VolumeUUID"].as_str().map(str::to_owned));
        #[cfg(not(target_os = "macos"))]
        let serial: Option<String> = None;
        let first_seen = previous["sightings"]
            .as_array()
            .and_then(|list| list.iter().find(|s| s["volume_id"] == volume_id.as_str()))
            .and_then(|s| s["first_seen"].as_str().map(str::to_owned))
            .unwrap_or_else(|| now.clone());
        out.push(
            json!({"volume_id": volume_id, "os": nu9ve_device::this_os(), "serial": serial,
            "label": mount.file_name().map(|n| n.to_string_lossy().into_owned()), "mounted": true,
            "first_seen": first_seen, "last_seen": now}),
        );
    }
    // Volumes seen before and not mounted now stay, with their last sighting.
    if let Some(list) = previous["sightings"].as_array() {
        for old in list {
            if !out.iter().any(|s| s["volume_id"] == old["volume_id"]) {
                let mut kept = old.clone();
                kept["mounted"] = json!(false);
                out.push(kept);
            }
        }
    }
    out.sort_by(|a, b| a["volume_id"].as_str().cmp(&b["volume_id"].as_str()));
    out
}

fn catalogs(shared: &HashSet<String>) -> Vec<(String, Value)> {
    let Some(dir) = nu9ve_ipc::default_dir() else {
        return Vec::new();
    };
    let Ok(entries) = nu9ve_ipc::scan(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        let nu9ve_ipc::State::Live(record) = &entry.state else {
            continue;
        };
        if record.app == "suite" || !record.has_capability("data_catalog_v1") {
            continue;
        }
        if crate::engines::check(record).is_err() {
            continue;
        }
        if let crate::catalog::Reply::Catalog(mut catalog) =
            crate::engines::fetch_catalog(record, &Map::new())
        {
            if let Some(list) = catalog["datasets"].as_array_mut() {
                for dataset in list.iter_mut() {
                    *dataset = nu9ve_exchange::publish_personal(dataset, shared);
                }
            }
            out.push((record.app.clone(), catalog));
        }
    }
    out
}

fn publish(agent: &Agent) -> io::Result<Value> {
    locked(&agent.dir, || publish_unlocked(agent))
}

fn publish_unlocked(agent: &Agent) -> io::Result<Value> {
    let root = agent.exchange()?;
    let folder = root.join("devices").join(agent.device_id());
    let mut state = agent.state()?;
    let mut written = Vec::new();
    let device_body = json!({"name": agent.device["name"], "os": agent.device["os"],
        "public_key": nu9ve_exchange::encode(&agent.key.public()), "agent_version": env!("CARGO_PKG_VERSION")});
    if publish_record(
        agent,
        &mut state,
        &folder,
        "device.json",
        "device",
        device_body,
    )? {
        written.push("device.json".to_owned());
    }
    let previous = read_json(&folder.join("volumes.json"))?
        .map(|r| r["body"].clone())
        .unwrap_or_else(|| json!({}));
    let mut body = json!({"sightings": sightings(&previous)});
    // A newer last_seen alone is published at most once an hour: transports are not for heartbeats.
    let without_last_seen = |b: &Value| {
        let mut b = b.clone();
        if let Some(list) = b["sightings"].as_array_mut() {
            for s in list {
                if let Some(m) = s.as_object_mut() {
                    m.remove("last_seen");
                }
            }
        }
        b
    };
    let hour_ago = nu9ve_device::utc_at(nu9ve_device::unix_now() - 3600);
    let fresh = previous["sightings"].as_array().is_some_and(|l| {
        l.iter().all(|s| {
            s["last_seen"]
                .as_str()
                .is_some_and(|t| t > hour_ago.as_str())
        })
    });
    if fresh && without_last_seen(&previous) == without_last_seen(&body) {
        body = previous;
    }
    if publish_record(agent, &mut state, &folder, "volumes.json", "volumes", body)? {
        written.push("volumes.json".to_owned());
    }
    let shared = agent.shared()?;
    for (app, catalog) in catalogs(&shared) {
        let file = format!("catalog/{app}.json");
        if publish_record(
            agent,
            &mut state,
            &folder,
            &file,
            "catalog",
            json!({"app": app, "catalog": catalog}),
        )? {
            written.push(file);
        }
    }
    Ok(json!({"folder": folder, "written": written, "seq": state["seq"]}))
}

fn files_of(folder: &Path) -> Vec<String> {
    let mut files: Vec<String> = OWN_FILES.iter().map(|f| (*f).to_owned()).collect();
    if let Ok(entries) = fs::read_dir(folder.join("catalog")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let app = name.strip_suffix(".json").unwrap_or_default();
            if suite::valid_app(app) {
                files.push(format!("catalog/{name}"));
            }
        }
    }
    // §1: apps/<app>/ with an app name, and file names an app can write; anything else is ignored.
    for app in fs::read_dir(folder.join("apps"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let app = app.file_name().to_string_lossy().into_owned();
        if !suite::valid_app(&app) {
            continue;
        }
        for e in fs::read_dir(folder.join("apps").join(&app))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = e.file_name().to_string_lossy().into_owned();
            if trade::app_file(&name, ".json") || trade::app_file(&name, ".ndjson") {
                files.push(format!("apps/{app}/{name}"));
            }
        }
    }
    files.sort();
    files
}

/// A file of the exchange, read with the §2 limit before anything parses it.
fn read_capped(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    io::Read::read_to_end(&mut io::Read::take(file, limit as u64 + 1), &mut bytes).ok()?;
    Some(bytes)
}

/// A folder name that can be a device id (a UUID 4 in lower case); anything else is not read.
fn device_like(id: &str) -> bool {
    id.len() == 36
        && id
            .bytes()
            .all(|b| b == b'-' || b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Agent {
    /// The last record accepted from another device's file (§5 keeps it when a transport brings an older
    /// one), or `None`. Only for paired devices: unpairing stops trusting what was accepted before too.
    fn accepted(&self, paired: &HashMap<String, [u8; 32]>, id: &str, file: &str) -> Option<Value> {
        if !paired.contains_key(id) || !device_like(id) {
            return None;
        }
        let bytes = read_capped(
            &self.dir.join("seen").join(id).join(file),
            nu9ve_exchange::MAX_JSON,
        )?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Every device seen in the exchange folder but this one, with what was accepted from each.
    fn others(&self) -> io::Result<Vec<String>> {
        let root = self.exchange()?;
        let mut ids: Vec<String> = fs::read_dir(root.join("devices"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|id| device_like(id) && id != self.device_id())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        Ok(ids)
    }
}

fn read(agent: &Agent) -> io::Result<Value> {
    locked(&agent.dir, || read_unlocked(agent))
}

/// §2: each line of an `.ndjson` is a record of its own. The lines that verify are kept with those
/// accepted before (a transport may bring a shorter, older copy); the file's state is its worst line's.
fn read_lines(
    agent: &Agent,
    paired: &HashMap<String, [u8; 32]>,
    id: &str,
    file: &str,
    bytes: &[u8],
) -> io::Result<State> {
    let mut worst = State::Ok;
    let mut kept: Vec<Value> = accepted_lines(agent, paired, id, file);
    let mut seqs: HashSet<u64> = kept.iter().filter_map(|r| r["seq"].as_u64()).collect();
    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let got = if line.len() > nu9ve_exchange::MAX_LINE {
            State::Invalid
        } else {
            nu9ve_exchange::reader_state(line, id, paired, None)
        };
        match got {
            State::Ok => {
                let record: Value = serde_json::from_slice(line).unwrap_or(Value::Null);
                if let Some(seq) = record["seq"].as_u64()
                    && seqs.insert(seq)
                {
                    kept.push(record);
                }
            }
            State::Invalid => worst = State::Invalid,
            State::Untrusted if worst != State::Invalid => worst = State::Untrusted,
            _ => {}
        }
    }
    kept.sort_by_key(|r| r["seq"].as_u64());
    let mut out = Vec::new();
    for r in &kept {
        out.extend(nu9ve_json::canonical(r));
        out.push(b'\n');
    }
    let cached = agent.dir.join("seen").join(id).join(file);
    if let Some(parent) = cached.parent() {
        fs::create_dir_all(parent)?;
    }
    nu9ve_fs::atomic_write_with(&cached, &out, nu9ve_fs::SyncMode::Plain)?;
    Ok(worst)
}

/// The lines accepted from another device's `.ndjson`, in `seq` order.
fn accepted_lines(
    agent: &Agent,
    paired: &HashMap<String, [u8; 32]>,
    id: &str,
    file: &str,
) -> Vec<Value> {
    if !paired.contains_key(id) || !device_like(id) {
        return Vec::new();
    }
    read_capped(
        &agent.dir.join("seen").join(id).join(file),
        nu9ve_exchange::MAX_JSON,
    )
    .map(|bytes| {
        bytes
            .split(|b| *b == b'\n')
            .filter_map(|l| serde_json::from_slice(l).ok())
            .collect()
    })
    .unwrap_or_default()
}

fn sorted<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut all: Vec<&String> = keys.collect();
    all.sort();
    all
}

/// §2: appends one signed line to an `.ndjson` of this machine's folder, with the next `seq`.
fn append_line(agent: &Agent, file: &str, body: Value) -> io::Result<u64> {
    locked(&agent.dir, || {
        let path = agent.folder()?.join(file);
        let mut state = agent.state()?;
        let seq = state["seq"].as_u64().unwrap_or(0) + 1;
        let record = nu9ve_exchange::record(
            &agent.key,
            agent.device_id(),
            "app",
            seq,
            &nu9ve_device::now_utc(),
            body,
        );
        let line = nu9ve_json::canonical(&record);
        if line.len() > nu9ve_exchange::MAX_LINE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a line passes the 1 MiB limit",
            ));
        }
        let mut bytes = read_capped(&path, nu9ve_exchange::MAX_JSON).unwrap_or_default();
        if bytes.len() + line.len() + 1 > nu9ve_exchange::MAX_JSON {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the file would pass 16 MiB",
            ));
        }
        bytes.extend(line);
        bytes.push(b'\n');
        state["seq"] = json!(seq);
        write_json(&agent.dir.join("state.json"), &state)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        nu9ve_fs::atomic_write_with(&path, &bytes, nu9ve_fs::SyncMode::Plain)?;
        Ok(seq)
    })
}

/// publish, read and the trade pass (§7): what the service does every few minutes, at once.
fn sync(agent: &Agent) -> io::Result<Value> {
    let published = publish(agent)?;
    read(agent)?;
    Ok(json!({"written": published["written"], "trade": trade::pass(agent)}))
}

#[cfg_attr(not(unix), allow(dead_code))]
impl Agent {
    /// This machine's folder in the exchange.
    fn folder(&self) -> io::Result<PathBuf> {
        Ok(self.exchange()?.join("devices").join(self.device_id()))
    }

    /// A record this machine published, or `None`.
    fn own(&self, file: &str) -> Option<Value> {
        let path = self.folder().ok()?.join(file);
        serde_json::from_slice(&read_capped(&path, nu9ve_exchange::MAX_JSON)?).ok()
    }

    /// The files under `sub` (`catalog`, `apps/<app>`) of this machine's folder, or of what was
    /// accepted from `device` (`None`: this one).
    fn listed(&self, device: Option<&str>, sub: &str) -> Vec<String> {
        let base = match device {
            None => self.folder().ok(),
            Some(id) if device_like(id) => Some(self.dir.join("seen").join(id)),
            Some(_) => None,
        };
        let mut out: Vec<String> = base
            .and_then(|b| fs::read_dir(b.join(sub)).ok())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| {
                        !n.starts_with('.') && (n.ends_with(".json") || n.ends_with(".ndjson"))
                    })
                    .map(|n| format!("{sub}/{n}"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
}

/// §5: publishes one file of this machine's folder now, if its body changed. Returns the `seq` it got.
#[cfg_attr(not(unix), allow(dead_code))]
fn publish_file(agent: &Agent, file: &str, kind: &str, body: Value) -> io::Result<Option<u64>> {
    locked(&agent.dir, || {
        let folder = agent.folder()?;
        let mut state = agent.state()?;
        let written = publish_record(agent, &mut state, &folder, file, kind, body)?;
        Ok(written.then(|| state["seq"].as_u64()).flatten())
    })
}

fn read_unlocked(agent: &Agent) -> io::Result<Value> {
    let root = agent.exchange()?;
    let paired = agent.paired()?;
    let mut state = agent.state()?;
    let mut devices = Vec::new();
    for id in agent.others()? {
        let folder = root.join("devices").join(&id);
        let mut files = Map::new();
        for file in files_of(&folder) {
            let Some(bytes) = read_capped(&folder.join(&file), nu9ve_exchange::MAX_JSON) else {
                continue;
            };
            if file.ends_with(".ndjson") {
                let got = read_lines(agent, &paired, &id, &file, &bytes)?;
                files.insert(file, json!({"state": got.id()}));
                continue;
            }
            let key = format!("{id}/{file}");
            let last = state["accepted"][&key].as_u64();
            let got = nu9ve_exchange::reader_state(&bytes, &id, &paired, last);
            let record: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            // §5 calls a seq already accepted `stale`. When the file is the very copy accepted, nothing
            // changed: it is shown as `ok`, and `stale` stays for an older copy a transport brought back.
            let unchanged = got == State::Stale
                && read_capped(
                    &agent.dir.join("seen").join(&id).join(&file),
                    nu9ve_exchange::MAX_JSON,
                )
                .is_some_and(|cached| cached == bytes);
            let shown = if unchanged { State::Ok } else { got };
            if got == State::Ok {
                state["accepted"][&key] = record["seq"].clone();
                let cached = agent.dir.join("seen").join(&id).join(&file);
                if let Some(parent) = cached.parent() {
                    fs::create_dir_all(parent)?;
                }
                nu9ve_fs::atomic_write_with(&cached, &bytes, nu9ve_fs::SyncMode::Plain)?;
            }
            files.insert(file, json!({"state": shown.id(), "seq": record["seq"], "written_at": record["written_at"]}));
        }
        let mut summary = Map::new();
        let name = agent
            .accepted(&paired, &id, "device.json")
            .map_or(Value::Null, |r| r["body"]["name"].clone());
        if let Some(r) = agent.accepted(&paired, &id, "volumes.json") {
            summary.insert("volumes".into(), r["body"]["sightings"].clone());
        }
        for file in files.keys() {
            if let Some(app) = file
                .strip_prefix("catalog/")
                .and_then(|f| f.strip_suffix(".json"))
                && let Some(r) = agent.accepted(&paired, &id, file)
            {
                let datasets = r["body"]["catalog"]["datasets"]
                    .as_array()
                    .map_or(0, Vec::len);
                summary.insert(format!("catalog:{app}"), json!({"datasets": datasets}));
            }
        }
        devices.push(json!({"device_id": id, "paired": paired.contains_key(&id), "name": name, "files": files, "seen": summary}));
    }
    write_json(&agent.dir.join("state.json"), &state)?;
    Ok(json!({"devices": devices}))
}

fn pair(agent: &Agent, device_id: &str, given: &str) -> io::Result<Value> {
    let root = agent.exchange()?;
    let path = root.join("devices").join(device_id).join("device.json");
    let bytes = fs::read(&path).map_err(|_| {
        other(format!(
            "no device.json for {device_id} in the exchange folder yet"
        ))
    })?;
    if let Some(problem) = nu9ve_exchange::record_problem(&bytes, device_id) {
        return Err(other(format!("its device.json is not valid: {problem}")));
    }
    let record: Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    let public = record["body"]["public_key"]
        .as_str()
        .and_then(nu9ve_exchange::decode_public)
        .ok_or_else(|| other("its device.json has no public key"))?;
    let fingerprint = nu9ve_exchange::fingerprint(&public);
    let given: String = given
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    // The message never shows the right fingerprint: the person must read it on the other screen.
    if given.len() < 32 || !fingerprint.starts_with(&given) {
        return Err(other(
            "the fingerprint does not match the one that device publishes; read it again on its own screen (at least the first 32 characters)",
        ));
    }
    // device.json is signed with the key it publishes (§4).
    let check = HashMap::from([(device_id.to_owned(), public)]);
    if nu9ve_exchange::reader_state(&bytes, device_id, &check, None) != State::Ok {
        return Err(other(
            "its device.json is not signed by the key it publishes",
        ));
    }
    let path = agent.dir.join("paired.json");
    let mut paired = read_json(&path)?.unwrap_or_else(|| json!({}));
    paired[device_id] = json!(nu9ve_exchange::encode(&public));
    write_json(&path, &paired)?;
    forget(agent, device_id)?;
    Ok(
        json!({"paired": device_id, "name": record["body"]["name"], "fingerprint": nu9ve_exchange::shown(&fingerprint)}),
    )
}

/// What was accepted from a device, forgotten when it is paired again or unpaired: a new key starts over.
fn forget(agent: &Agent, device_id: &str) -> io::Result<()> {
    if !device_like(device_id) {
        return Ok(());
    }
    locked(&agent.dir, || {
        let mut state = agent.state()?;
        if let Some(accepted) = state["accepted"].as_object_mut() {
            accepted.retain(|k, _| !k.starts_with(&format!("{device_id}/")));
        }
        write_json(&agent.dir.join("state.json"), &state)?;
        match fs::remove_dir_all(agent.dir.join("seen").join(device_id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    })
}

fn unpair(agent: &Agent, device_id: &str) -> io::Result<Value> {
    let path = agent.dir.join("paired.json");
    let mut paired = read_json(&path)?.unwrap_or_else(|| json!({}));
    let removed = paired
        .as_object_mut()
        .and_then(|m| m.remove(device_id))
        .is_some();
    write_json(&path, &paired)?;
    forget(agent, device_id)?;
    trade::forget_device(agent, device_id)?;
    Ok(json!({"unpaired": device_id, "was_paired": removed}))
}

/// A dataset as the suite names it: `owner:id`, with the owner an app and the id one of its catalog.
fn valid_shared_id(id: &str) -> bool {
    id.split_once(':').is_some_and(|(owner, local)| {
        suite::valid_app(owner) && crate::catalog::valid_dataset_id(local)
    })
}

/// «Available on all my machines» (exchange §6, export §5). Unsharing also stops offering its files.
fn set_shared(agent: &Agent, id: &str, on: bool) -> io::Result<Value> {
    if !valid_shared_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a dataset is owner:id, as in its catalog",
        ));
    }
    let list = locked(&agent.dir, || {
        let mut shared = agent.shared()?;
        if on {
            shared.insert(id.to_owned());
        } else {
            shared.remove(id);
        }
        let mut list: Vec<String> = shared.into_iter().collect();
        list.sort();
        write_json(&agent.dir.join("shared.json"), &json!(list))?;
        Ok(list)
    })?;
    let unoffered = if on {
        0
    } else {
        trade::unoffer_dataset(agent, id)?
    };
    Ok(json!({"shared": list, "unoffered": unoffered}))
}

fn set_exchange(agent: &Agent, folder: &str) -> io::Result<Value> {
    let path = PathBuf::from(folder);
    if !path.is_absolute() {
        return Err(other("the exchange folder is an absolute path"));
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    if let Some(cloud) = nu9ve_exchange::cloud_synced(&path, &home) {
        return Err(other(format!(
            "{folder} is inside {cloud}; v1 does not encrypt, so it cannot go to someone else's cloud"
        )));
    }
    fs::create_dir_all(path.join("devices"))?;
    let mut config = agent.config()?;
    config["exchange"] = json!(folder);
    write_json(&agent.dir.join("config.json"), &config)?;
    Ok(json!({"exchange": folder}))
}

fn status(agent: &Agent) -> io::Result<Value> {
    let fingerprint = agent.key.fingerprint();
    Ok(
        json!({"device_id": agent.device_id(), "name": agent.device["name"], "fingerprint": fingerprint,
        "shown": nu9ve_exchange::shown(&fingerprint), "exchange": agent.config()?["exchange"],
        "paired": agent.paired()?.keys().collect::<Vec<_>>(), "seq": agent.state()?["seq"],
        "serving_pid": serve::running_pid(&agent.dir)}),
    )
}

/// A method of the suite engine, run here without the service (for apps without a socket, and the tests).
fn call(agent: &Agent, method: &str, arguments: &str) -> io::Result<Value> {
    let Ok(Value::Object(arguments)) = serde_json::from_str::<Value>(arguments) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the arguments are a JSON object",
        ));
    };
    suite::run(agent, method, &arguments).map_err(|e| other(format!("{method}: {e}")))
}

/// Runs one command and prints its result as JSON.
pub fn run(args: &[&str]) -> io::Result<Value> {
    match args {
        ["init"] => {
            let (agent, outcome) = open(true)?;
            let mut out = status(&agent)?;
            out["outcome"] = outcome;
            Ok(out)
        }
        ["name", name] => {
            let file = nu9ve_device::default_file().ok_or_else(|| other("no device file"))?;
            let device = nu9ve_device::rename(&file, name)?;
            Ok(json!({"device_id": device["device_id"], "name": device["name"]}))
        }
        ["exchange", folder] => set_exchange(&open(false)?.0, folder),
        ["publish"] => publish(&open(false)?.0),
        ["read"] => read(&open(false)?.0),
        ["pair", device_id, fingerprint] => pair(&open(false)?.0, device_id, fingerprint),
        ["unpair", device_id] => unpair(&open(false)?.0, device_id),
        ["share", id] => set_shared(&open(false)?.0, id, true),
        ["unshare", id] => set_shared(&open(false)?.0, id, false),
        ["status"] => status(&open(false)?.0),
        ["serve"] => serve::serve(open(true)?.0),
        ["sync"] => sync(&open(false)?.0),
        ["call", method] => call(&open(false)?.0, method, "{}"),
        ["call", method, arguments] => call(&open(false)?.0, method, arguments),
        ["offer", file] => call(&open(false)?.0, "offer", &json!({"path": file}).to_string()),
        ["unoffer", file] => call(
            &open(false)?.0,
            "unoffer",
            &json!({"path": file}).to_string(),
        ),
        ["offer", file, "--dataset", dataset] => call(
            &open(false)?.0,
            "offer",
            &json!({"path": file, "dataset": dataset}).to_string(),
        ),
        ["install"] => install::install("installer"),
        ["install", "--by", app] => install::install(app),
        ["uninstall"] => install::uninstall(),
        ["state"] => install::state_of(Some((install::VERSION, install::BUILD))),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE)),
    }
}
