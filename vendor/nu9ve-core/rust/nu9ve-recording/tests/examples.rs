//! nu9ve-recording-v1 against data/recording-examples-v1.json, case by case.

use base64::Engine;
use nu9ve_recording::{
    Epoch, Process, Seen, State, derive, directory_for, is_listed, is_newer, parse, problem,
    session_time, status,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// app, session id, state, captures and epoch.
type SessionRow = (String, String, String, Vec<String>, Option<Epoch>);

fn examples() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/recording-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn cases<'a>(root: &'a Value, key: &str) -> &'a Vec<Value> {
    root[key].as_array().unwrap()
}

fn table(v: &Value) -> impl Fn(u32) -> Option<Process> + '_ {
    move |pid| {
        let row = v.get(pid.to_string())?;
        Some(Process {
            own_user: row["own_user"].as_bool()?,
            start_us: row["start_us"].as_u64()?,
        })
    }
}

fn epoch(v: &Value) -> Option<Epoch> {
    Some(Epoch {
        clock: v.get("clock")?.as_str()?.to_owned(),
        value: v["value"].as_i64()?,
        timescale: v["timescale"].as_i64()?,
    })
}

fn bytes(case: &Value) -> Vec<u8> {
    match case.get("bytes_base64").and_then(Value::as_str) {
        Some(b64) => base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap(),
        None => case["text"].as_str().unwrap().as_bytes().to_vec(),
    }
}

#[test]
fn folders_follow_the_examples_on_both_systems() {
    let root = examples();
    for case in cases(&root, "folders") {
        let env = case["env"].as_object().unwrap();
        let lookup = |k: &str| env.get(k).and_then(Value::as_str).map(str::to_owned);
        let dir = directory_for(
            case["platform"] == "windows",
            &lookup,
            case["qa_root"].as_str(),
        );
        assert_eq!(
            dir.map(|d| d.to_string_lossy().into_owned()).as_deref(),
            case["dir"].as_str(),
            "{case}"
        );
    }
}

#[test]
fn only_app_state_files_are_listed() {
    let root = examples();
    for case in cases(&root, "names") {
        assert_eq!(
            is_listed(case["file"].as_str().unwrap()),
            case["listed"].as_bool().unwrap(),
            "{case}"
        );
    }
}

#[test]
fn every_state_file_gets_its_problem() {
    let root = examples();
    let records = cases(&root, "records");
    assert_eq!(records.len(), 36);
    for case in records {
        let file = case["file"].as_str().unwrap();
        let found = problem(file, &bytes(case));
        assert_eq!(found, case["problem"].as_str(), "{}", case["name"]);
        assert_eq!(parse(file, &bytes(case)).is_some(), found.is_none());
    }
}

#[test]
fn liveness_follows_the_pid_and_its_start_time() {
    let root = examples();
    for case in cases(&root, "liveness") {
        let s = status(
            case["file"].as_str().unwrap(),
            case["text"].as_str().unwrap().as_bytes(),
            &table(&case["processes"]),
        );
        let name = match s {
            nu9ve_recording::Status::Live => "live",
            nu9ve_recording::Status::Stale => "stale",
            nu9ve_recording::Status::Invalid => "invalid",
        };
        assert_eq!(name, case["status"], "{}", case["name"]);
    }
}

#[test]
fn consumers_derive_capturing_quiet_and_sessions() {
    let root = examples();
    for case in cases(&root, "derive") {
        let files: BTreeMap<String, Vec<u8>> = case["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().as_bytes().to_vec()))
            .collect();
        let d = derive(&files, &table(&case["processes"]));
        let want = &case["derived"];
        assert_eq!(d.capturing, want["capturing"], "{}", case["name"]);
        assert_eq!(d.quiet, want["quiet"], "{}", case["name"]);
        let sessions: Vec<SessionRow> = want["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["app"].as_str().unwrap().to_owned(),
                    s["session_id"].as_str().unwrap().to_owned(),
                    s["state"].as_str().unwrap().to_owned(),
                    s["captures"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c.as_str().unwrap().to_owned())
                        .collect(),
                    epoch(&s["epoch"]),
                )
            })
            .collect();
        let ours: Vec<_> = d
            .sessions
            .iter()
            .map(|s| {
                (
                    s.app.clone(),
                    s.session_id.clone(),
                    s.state.name().to_owned(),
                    s.captures.clone(),
                    s.epoch.clone(),
                )
            })
            .collect();
        assert_eq!(ours, sessions, "{}", case["name"]);
        let recording: Vec<&str> = d.recording_apps();
        assert_eq!(
            recording.len(),
            d.sessions
                .iter()
                .filter(|s| s.state == State::Recording)
                .count()
        );
    }
}

#[test]
fn notifications_are_ordered_by_seq_per_process() {
    let root = examples();
    let seen = |v: &Value| {
        v.as_object().map(|_| Seen {
            pid: v["pid"].as_u64().unwrap() as u32,
            process_start_us: v["process_start_us"].as_u64().unwrap(),
            seq: v["seq"].as_u64().unwrap(),
        })
    };
    for case in cases(&root, "order") {
        assert_eq!(
            is_newer(seen(&case["seen"]), seen(&case["file"]).unwrap()),
            case["act"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn session_time_counts_microseconds_from_the_epoch() {
    let root = examples();
    for case in cases(&root, "session_time") {
        let t = session_time(
            &epoch(&case["epoch"]).unwrap(),
            case["now_ns"].as_i64().unwrap(),
        );
        assert_eq!(t, case["t_us"].as_i64(), "{}", case["name"]);
    }
}

#[test]
fn a_real_folder_with_this_process_recording() {
    let dir = tempfile_dir();
    let me = nu9ve_recording::process(std::process::id()).unwrap();
    let root = examples();
    let text = cases(&root, "records")
        .iter()
        .find(|c| c["name"] == "recording")
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .replace("\"pid\":5120", &format!("\"pid\":{}", std::process::id()))
        .replace(
            "\"process_start_us\":1795600000123456",
            &format!("\"process_start_us\":{}", me.start_us),
        );
    let name = cases(&root, "records")[0]["file"].as_str().unwrap();
    std::fs::write(dir.join(name), text).unwrap();
    std::fs::write(dir.join(".partial.tmp"), "{").unwrap();
    let d = nu9ve_recording::read_dir(&dir);
    assert!(d.capturing && d.quiet && d.sessions.len() == 1);
    assert!(!nu9ve_recording::read_dir(&dir.join("missing")).capturing);
    let _ = std::fs::remove_dir_all(&dir);
}

fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nu9ve-recording-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
