//! nu9ve-exchange-v1: the exchange folder between a person's machines. Each machine writes only its own
//! folder, in records signed with Ed25519 over their canonical JSON; a reader trusts only paired devices
//! and orders by the writer's `seq`, never by clocks. No app is named here: apps are parameters.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const CONTRACT: &str = "nu9ve-exchange/1";
/// §2: the largest `.json` record.
pub const MAX_JSON: usize = 16 * 1024 * 1024;
/// §2: the largest line of a `.ndjson` under `apps/<app>/`.
pub const MAX_LINE: usize = 1024 * 1024;
/// §3: the largest object.
pub const MAX_OBJECT: u64 = 64 * 1024 * 1024;
/// §3: the kinds a v1 reader knows. Any other is valid and ignored.
pub const KINDS: [&str; 7] = [
    "device", "volumes", "catalog", "leases", "requests", "objects", "app",
];

/// A device key. The secret never leaves the agent's keychain (§4).
pub struct Key(SigningKey);

impl Key {
    pub fn from_secret(secret: &[u8; 32]) -> Self {
        Key(SigningKey::from_bytes(secret))
    }

    /// A new key from the system's random source.
    pub fn generate() -> nu9ve_json::Result<Self> {
        Ok(Key::from_secret(&nu9ve_json::random_bytes::<32>()?))
    }

    pub fn secret(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.public())
    }
}

/// §4: SHA-256 of the 32 public key bytes, lower-case hex.
pub fn fingerprint(public: &[u8; 32]) -> String {
    nu9ve_json::hex(&Sha256::digest(public))
}

/// §4: the first 32 hex characters of a fingerprint, in groups of four, as people compare it.
pub fn shown(fingerprint: &str) -> String {
    let head: Vec<char> = fingerprint.chars().take(32).collect();
    head.chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn decode_public(text: &str) -> Option<[u8; 32]> {
    STANDARD.decode(text).ok()?.try_into().ok()
}

fn unsigned(record: &Map<String, Value>) -> Value {
    let mut copy = record.clone();
    copy.remove("signature");
    Value::Object(copy)
}

/// §2: the record with its signature over the canonical bytes of everything but `signature`.
pub fn sign(key: &Key, record: &Value) -> Value {
    let Some(map) = record.as_object() else {
        return record.clone();
    };
    let body = unsigned(map);
    let signature = key.0.sign(&nu9ve_json::canonical(&body));
    let mut out = body;
    out["signature"] =
        json!({"alg": "ed25519", "key": key.fingerprint(), "value": encode(&signature.to_bytes())});
    out
}

/// A record of `kind` with this device's `seq` and time, signed.
pub fn record(
    key: &Key,
    device_id: &str,
    kind: &str,
    seq: u64,
    written_at: &str,
    body: Value,
) -> Value {
    sign(
        key,
        &json!({"contract": CONTRACT, "device_id": device_id, "kind": kind, "seq": seq, "written_at": written_at, "body": body}),
    )
}

fn uuid4(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => *c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

fn utc_time(text: &str) -> bool {
    let b = text.as_bytes();
    let digits = |r: std::ops::Range<usize>| {
        r.into_iter()
            .all(|i| b.get(i).is_some_and(u8::is_ascii_digit))
    };
    if b.len() < 20
        || !digits(0..4)
        || b[4] != b'-'
        || !digits(5..7)
        || b[7] != b'-'
        || !digits(8..10)
        || b[10] != b'T'
        || !digits(11..13)
        || b[13] != b':'
        || !digits(14..16)
        || b[16] != b':'
        || !digits(17..19)
    {
        return false;
    }
    match &b[19..] {
        [b'Z'] => true,
        [b'.', rest @ .., b'Z'] => {
            (1..=9).contains(&rest.len()) && rest.iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

/// §2: the first rule a record in `folder_device`'s folder breaks (the reasons of `invalid`), or `None`.
pub fn record_problem(bytes: &[u8], folder_device: &str) -> Option<&'static str> {
    if bytes.len() > MAX_JSON {
        return Some("too_large");
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Some("not_json");
    };
    let Some(map) = value.as_object() else {
        return Some("not_object");
    };
    for (field, check) in [
        ("contract", Value::is_string as fn(&Value) -> bool),
        ("device_id", Value::is_string),
        ("kind", Value::is_string),
        ("seq", |v: &Value| v.is_u64() || v.is_i64()),
        ("written_at", Value::is_string),
        ("body", Value::is_object),
        ("signature", Value::is_object),
    ] {
        match map.get(field) {
            None => return Some("missing_field"),
            Some(v) if !check(v) => return Some("wrong_type"),
            _ => {}
        }
    }
    if map["contract"] != CONTRACT {
        return Some("unknown_contract");
    }
    let device = map["device_id"].as_str().unwrap_or_default();
    if !uuid4(device) || device != folder_device {
        return Some("other_device");
    }
    if !map["seq"]
        .as_u64()
        .is_some_and(|s| (1..=(1u64 << 53) - 1).contains(&s))
    {
        return Some("bad_seq");
    }
    if !utc_time(map["written_at"].as_str().unwrap_or_default()) {
        return Some("bad_time");
    }
    let sig = &map["signature"];
    if sig.get("alg").and_then(Value::as_str) != Some("ed25519")
        || !sig.get("key").is_some_and(Value::is_string)
        || !sig.get("value").is_some_and(Value::is_string)
    {
        return Some("bad_signature_field");
    }
    None
}

/// §5: what a reader makes of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ok,
    Stale,
    Untrusted,
    Invalid,
}

impl State {
    pub fn id(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Stale => "stale",
            State::Untrusted => "untrusted",
            State::Invalid => "invalid",
        }
    }
}

/// §4 and §5. `paired` maps a device to its public key; `last_seq` is the last accepted for this file.
pub fn reader_state(
    bytes: &[u8],
    folder_device: &str,
    paired: &HashMap<String, [u8; 32]>,
    last_seq: Option<u64>,
) -> State {
    if record_problem(bytes, folder_device).is_some() {
        return State::Invalid;
    }
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(bytes) else {
        return State::Invalid;
    };
    let Some(public) = paired.get(folder_device) else {
        return State::Untrusted;
    };
    if map["signature"]["key"].as_str() != Some(fingerprint(public).as_str()) {
        return State::Untrusted;
    }
    let (Ok(key), Some(Ok(bytes))) = (
        VerifyingKey::from_bytes(public),
        map["signature"]["value"]
            .as_str()
            .map(|v| STANDARD.decode(v)),
    ) else {
        return State::Untrusted;
    };
    let Ok(signature) = Signature::from_slice(&bytes) else {
        return State::Untrusted;
    };
    if key
        .verify_strict(&nu9ve_json::canonical(&unsigned(&map)), &signature)
        .is_err()
    {
        return State::Untrusted;
    }
    match (map["seq"].as_u64(), last_seq) {
        (Some(seq), Some(last)) if seq <= last => State::Stale,
        _ => State::Ok,
    }
}

/// §3: true for the kinds a v1 reader uses; a reader ignores the others.
pub fn known_kind(record: &Value) -> bool {
    record["kind"].as_str().is_some_and(|k| KINDS.contains(&k))
}

const PERSONAL_SHAPE: [&str; 6] = ["owner", "id", "modality", "privacy", "size", "state"];

/// §6: a personal dataset goes whole only if the person shared `owner:id`; otherwise its shape.
pub fn publish_personal(dataset: &Value, shared: &HashSet<String>) -> Value {
    if dataset["privacy"] != "personal" {
        return dataset.clone();
    }
    let id = format!(
        "{}:{}",
        dataset["owner"].as_str().unwrap_or_default(),
        dataset["id"].as_str().unwrap_or_default()
    );
    if shared.contains(&id) {
        return dataset.clone();
    }
    let mut shape = Map::new();
    for field in PERSONAL_SHAPE {
        if let Some(v) = dataset.get(field) {
            shape.insert(field.into(), v.clone());
        }
    }
    if let Some(time) = dataset.get("axes").and_then(|a| a.get("time")) {
        shape.insert("axes".into(), json!({"time": time}));
    }
    Value::Object(shape)
}

/// §6: the name of the cloud drive `root` is inside, if any. v1 does not encrypt, so an agent refuses
/// these roots. `home` is the person's home folder.
pub fn cloud_synced(root: &Path, home: &Path) -> Option<&'static str> {
    let inside = |p: &str| root.starts_with(home.join(p));
    if inside("Library/Mobile Documents") || inside("iCloud Drive") {
        return Some("iCloud Drive");
    }
    if inside("Library/CloudStorage") {
        return Some("a cloud storage provider");
    }
    let text = root.to_string_lossy();
    for (needle, name) in [
        ("Dropbox", "Dropbox"),
        ("Google Drive", "Google Drive"),
        ("OneDrive", "OneDrive"),
    ] {
        if text.contains(needle) {
            return Some(name);
        }
    }
    None
}

/// §7.1: what a machine does with its lease after reading the same one from another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseOutcome {
    /// The other took it from this one after this one took it: drop it.
    Release,
    /// This one took it from the other, which has not read that yet.
    Keep,
    /// Both say they own it.
    Conflict,
}

impl LeaseOutcome {
    pub fn id(self) -> &'static str {
        match self {
            LeaseOutcome::Release => "release",
            LeaseOutcome::Keep => "keep",
            LeaseOutcome::Conflict => "conflict",
        }
    }
}

/// §7.1. `me` holds the lease, taken in its record `taken_seq` with `my_from` (or none); `theirs` is the same
/// lease in the accepted `leases.json` of `other`, whose record has `their_seq`.
pub fn lease_outcome(
    me: &str,
    taken_seq: u64,
    my_from: Option<&Value>,
    other: &str,
    theirs: &Value,
    their_seq: u64,
) -> LeaseOutcome {
    let names = |from: &Value, device: &str| from["device_id"].as_str() == Some(device);
    if let Some(from) = theirs.get("from")
        && names(from, me)
        && from["seq"].as_u64().is_some_and(|s| s >= taken_seq)
    {
        return LeaseOutcome::Release;
    }
    if let Some(from) = my_from
        && names(from, other)
        && from["seq"].as_u64().is_some_and(|s| their_seq <= s)
    {
        return LeaseOutcome::Keep;
    }
    LeaseOutcome::Conflict
}

/// §7: the `objects.json` entry for request `id`, given what the asked machine holds for it
/// (`{sha256, bytes, media_type, where}`, or `{refused}` for one it will not give, like `not_shared`),
/// or `not_found`.
pub fn object_answer(id: &str, held: Option<&Value>) -> Value {
    let Some(held) = held else {
        return json!({"request": id, "refused": "not_found"});
    };
    if let Some(refused) = held["refused"].as_str() {
        return json!({"request": id, "refused": refused});
    }
    if held["bytes"].as_u64().is_none_or(|b| b > MAX_OBJECT) {
        return json!({"request": id, "refused": "too_large", "where": held["where"]});
    }
    json!({"sha256": held["sha256"], "bytes": held["bytes"], "media_type": held["media_type"], "request": id})
}
