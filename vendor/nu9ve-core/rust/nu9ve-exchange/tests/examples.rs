//! nu9ve-exchange-v1 in Rust against data/exchange-examples-v1.json, written by the Python reference.
//! Ed25519 is deterministic: signing with the example key must give the reference's signatures exactly.

use nu9ve_exchange::{
    Key, State, decode_public, fingerprint, known_kind, publish_personal, reader_state, shown, sign,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/exchange-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn example_key() -> Key {
    let secret: [u8; 32] = Sha256::digest(b"nu9ve-exchange-v1 example key M4").into();
    Key::from_secret(&secret)
}

#[test]
fn the_key_and_its_fingerprint_match() {
    let examples = examples();
    let key = example_key();
    assert_eq!(
        nu9ve_exchange::encode(&key.public()),
        examples["key"]["public_key"]
    );
    assert_eq!(key.fingerprint(), examples["key"]["fingerprint"]);
    assert_eq!(shown(&key.fingerprint()), examples["key"]["shown"]);
}

#[test]
fn signing_gives_the_reference_signatures() {
    let examples = examples();
    let key = example_key();
    for case in examples["records"].as_array().unwrap() {
        let record: Value = serde_json::from_str(case["text"].as_str().unwrap()).unwrap();
        assert_eq!(sign(&key, &record), record, "{}", case["name"]);
    }
}

#[test]
fn every_reading_case_gets_its_state() {
    let examples = examples();
    for case in examples["reading"].as_array().unwrap() {
        let paired: HashMap<String, [u8; 32]> = case["paired"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), decode_public(v.as_str().unwrap()).unwrap()))
            .collect();
        let state = reader_state(
            case["text"].as_str().unwrap().as_bytes(),
            case["folder_device"].as_str().unwrap(),
            &paired,
            case["last_seq"].as_u64(),
        );
        assert_eq!(state.id(), case["state"], "{}", case["name"]);
    }
}

#[test]
fn every_kind_reads_ok_and_an_unknown_one_is_ignored() {
    let examples = examples();
    let key = example_key();
    let device = examples["key"]["device_id"].as_str().unwrap();
    let paired = HashMap::from([(device.to_owned(), key.public())]);
    for case in examples["records"].as_array().unwrap() {
        assert_eq!(
            reader_state(
                case["text"].as_str().unwrap().as_bytes(),
                device,
                &paired,
                None
            ),
            State::Ok
        );
    }
    let unknown: Value =
        serde_json::from_str(examples["unknown_kind"]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        reader_state(
            examples["unknown_kind"]["text"]
                .as_str()
                .unwrap()
                .as_bytes(),
            device,
            &paired,
            None
        ),
        State::Ok
    );
    assert!(!known_kind(&unknown));
    assert_eq!(fingerprint(&key.public()).len(), 64);
}

#[test]
fn personal_datasets_go_whole_only_when_shared() {
    let examples = examples();
    let personal = &examples["personal"];
    assert_eq!(
        publish_personal(&personal["dataset"], &HashSet::new()),
        personal["not_shared"]
    );
    let dataset = &personal["dataset"];
    let id = format!(
        "{}:{}",
        dataset["owner"].as_str().unwrap(),
        dataset["id"].as_str().unwrap()
    );
    let shared = HashSet::from([id]);
    assert_eq!(
        publish_personal(&personal["dataset"], &shared),
        personal["shared"]
    );
}

#[test]
fn roots_in_a_cloud_drive_are_refused() {
    use nu9ve_exchange::cloud_synced;
    let home = Path::new("/Users/ana");
    assert_eq!(
        cloud_synced(
            Path::new("/Users/ana/Library/Mobile Documents/com~apple~CloudDocs/x"),
            home
        ),
        Some("iCloud Drive")
    );
    assert_eq!(
        cloud_synced(Path::new("/Users/ana/Library/CloudStorage/Dropbox/x"), home),
        Some("a cloud storage provider")
    );
    assert_eq!(
        cloud_synced(Path::new("/Users/ana/Dropbox/x"), home),
        Some("Dropbox")
    );
    assert_eq!(cloud_synced(Path::new("/Users/ana/Sync/nu9ve"), home), None);
}

#[test]
fn leases_are_handed_over_without_clocks() {
    for case in examples()["leases"].as_array().unwrap() {
        let got = nu9ve_exchange::lease_outcome(
            case["me"].as_str().unwrap(),
            case["taken_seq"].as_u64().unwrap(),
            case.get("my_from").filter(|v| !v.is_null()),
            case["other"].as_str().unwrap(),
            &case["theirs"],
            case["their_seq"].as_u64().unwrap(),
        );
        assert_eq!(got.id(), case["outcome"], "{}", case["name"]);
    }
}

#[test]
fn a_request_is_answered_by_what_is_held() {
    for case in examples()["answers"].as_array().unwrap() {
        let held = case.get("held").filter(|v| !v.is_null());
        let got = nu9ve_exchange::object_answer(case["request"]["id"].as_str().unwrap(), held);
        assert_eq!(got, case["entry"], "{}", case["name"]);
    }
}
