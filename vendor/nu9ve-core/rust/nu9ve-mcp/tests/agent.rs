//! `nu9ve agent` end to end: two QA agents (two machines) share one exchange folder in a temporary
//! directory. Nothing touches the person's device, keychain, engines or folders.

#[cfg(unix)]
mod common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Waits up to `secs` for `check`, polling every 100 ms.
fn eventually(secs: u64, mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The first marker of the volume examples, mounted as `label` in the machine's QA mounts folder.
fn mount(machine: &Machine, label: &str) {
    let examples: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/volume-examples-v1.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let staged = machine.volumes().with_file_name(format!("staged-{label}"));
    std::fs::create_dir_all(staged.join(".nu9ve")).unwrap();
    std::fs::write(
        staged.join(".nu9ve/volume.json"),
        examples["markers"][0]["text"].as_str().unwrap(),
    )
    .unwrap();
    // A rename, as a mount appears: whole, in one change of the mounts folder.
    std::fs::create_dir_all(machine.volumes()).unwrap();
    std::fs::rename(staged, machine.volumes().join(label)).unwrap();
}

fn stop(child: &mut Child) -> std::process::ExitStatus {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "the agent did not stop");
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct Machine {
    agent: PathBuf,
    device: PathBuf,
    engines: PathBuf,
}

impl Machine {
    fn new(root: &Path, name: &str) -> Self {
        let engines = root.join(name).join("engines");
        std::fs::create_dir_all(&engines).unwrap();
        Machine {
            agent: root.join(name).join("agent"),
            device: root.join(name).join("device.json"),
            engines,
        }
    }

    fn volumes(&self) -> PathBuf {
        self.engines.parent().unwrap().join("volumes")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nu9ve"));
        command
            .arg("agent")
            .args(args)
            .env("NU9VE_AGENT_DIR", &self.agent)
            .env("NU9VE_DEVICE_FILE", &self.device)
            .env("NU9VE_ENGINES_DIR", &self.engines)
            .env("NU9VE_VOLUMES_DIR", self.volumes());
        command
    }

    fn run(&self, args: &[&str]) -> Result<Value, String> {
        let out = self.command(args).output().unwrap();
        if out.status.success() {
            Ok(serde_json::from_slice(&out.stdout).unwrap())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).into_owned())
        }
    }

    fn ok(&self, args: &[&str]) -> Value {
        self.run(args).unwrap_or_else(|e| panic!("{args:?}: {e}"))
    }
}

fn file_state(read: &Value, device: &str, file: &str) -> String {
    let entry = read["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["device_id"] == device)
        .unwrap();
    entry["files"][file]["state"].as_str().unwrap().to_owned()
}

#[test]
fn two_machines_pair_read_and_reject_what_they_should() {
    let tmp = tempfile::tempdir().unwrap();
    let exchange = tmp.path().join("exchange");
    let (m1, m4) = (
        Machine::new(tmp.path(), "m1"),
        Machine::new(tmp.path(), "m4"),
    );

    let a = m1.ok(&["init"]);
    let b = m4.ok(&["init"]);
    assert_eq!(a["outcome"], "created");
    assert_eq!(m1.ok(&["init"])["outcome"], "existing");
    let (id1, id4) = (
        a["device_id"].as_str().unwrap().to_owned(),
        b["device_id"].as_str().unwrap().to_owned(),
    );
    m1.ok(&["name", "M1"]);
    m4.ok(&["name", "M4"]);
    for m in [&m1, &m4] {
        m.ok(&["exchange", exchange.to_str().unwrap()]);
    }
    // A disk with a marker mounted on the M4 (from the volume examples).
    mount(&m4, "MEDIA501");
    let written = m4.ok(&["publish"]);
    assert!(
        written["written"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "device.json")
    );
    m1.ok(&["publish"]);

    // Not paired yet: nothing of the M4 is trusted.
    let before = m1.ok(&["read"]);
    assert_eq!(file_state(&before, &id4, "device.json"), "untrusted");

    // A wrong fingerprint does not pair; the right one does.
    let shown = b["shown"].as_str().unwrap();
    let wrong = m1
        .run(&["pair", &id4, "0000000000000000000000000000000000"])
        .unwrap_err();
    assert!(
        !wrong.contains(&shown[..9]),
        "the error never gives away the right fingerprint: {wrong}"
    );
    let paired = m1.ok(&["pair", &id4, shown]);
    assert_eq!(paired["name"], "M4");

    let after = m1.ok(&["read"]);
    assert_eq!(file_state(&after, &id4, "device.json"), "ok");
    assert_eq!(file_state(&after, &id4, "volumes.json"), "ok");
    let m4_seen = after["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["device_id"] == id4.as_str())
        .unwrap();
    assert_eq!(m4_seen["name"], "M4");
    let sighting = &m4_seen["seen"]["volumes"][0];
    assert_eq!(sighting["label"], "MEDIA501");
    assert_eq!(sighting["mounted"], true);

    // Nothing changed: nothing is rewritten, and reading again finds the same seq, now stale.
    assert!(
        m4.ok(&["publish"])["written"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // Unchanged since it was accepted: ok, not stale (stale is for an older copy brought back).
    assert_eq!(file_state(&m1.ok(&["read"]), &id4, "device.json"), "ok");

    // A renamed M4 publishes a newer device.json, which is ok again.
    let device_file = exchange.join("devices").join(&id4).join("device.json");
    let older = std::fs::read(&device_file).unwrap();
    m4.ok(&["name", "M4 estudio"]);
    m4.ok(&["publish"]);
    assert_eq!(file_state(&m1.ok(&["read"]), &id4, "device.json"), "ok");
    // A transport brings the older copy back: stale, and the newer name stays.
    let newer = std::fs::read(&device_file).unwrap();
    std::fs::write(&device_file, &older).unwrap();
    let back = m1.ok(&["read"]);
    assert_eq!(file_state(&back, &id4, "device.json"), "stale");
    let m4_entry = back["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["device_id"] == id4.as_str())
        .unwrap()
        .clone();
    assert_eq!(m4_entry["name"], "M4 estudio");
    std::fs::write(&device_file, newer).unwrap();

    // A file changed by someone else is untrusted.
    let volumes = exchange.join("devices").join(&id4).join("volumes.json");
    let mut record: Value = serde_json::from_slice(&std::fs::read(&volumes).unwrap()).unwrap();
    record["body"]["sightings"] = serde_json::json!([{"volume_id": "fake"}]);
    std::fs::write(&volumes, serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(
        file_state(&m1.ok(&["read"]), &id4, "volumes.json"),
        "untrusted"
    );

    // The M1 never trusts its own folder as another device, and the M4 has not paired the M1.
    let from_m4 = m4.ok(&["read"]);
    assert_eq!(file_state(&from_m4, &id1, "device.json"), "untrusted");

    // Unpairing stops trusting.
    m1.ok(&["unpair", &id4]);
    assert_eq!(
        file_state(&m1.ok(&["read"]), &id4, "device.json"),
        "untrusted"
    );

    // A root inside someone else's cloud is refused (§6 of the exchange).
    let home = std::env::var("HOME").unwrap();
    let icloud =
        format!("{home}/Library/Mobile Documents/com~apple~CloudDocs/nu9ve-test-never-created");
    assert!(
        m1.run(&["exchange", &icloud])
            .unwrap_err()
            .contains("iCloud Drive")
    );
    assert!(!Path::new(&icloud).exists());
}

#[test]
fn qa_never_uses_the_persons_device_file() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_nu9ve"))
        .args(["agent", "init"])
        .env("NU9VE_AGENT_DIR", tmp.path().join("agent"))
        .env_remove("NU9VE_DEVICE_FILE")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("NU9VE_DEVICE_FILE"));
}

#[cfg(unix)]
#[test]
fn serve_publishes_on_change_one_agent_per_machine_and_stops_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let exchange = tmp.path().join("exchange");
    let m4 = Machine::new(tmp.path(), "m4");
    let id = m4.ok(&["init"])["device_id"].as_str().unwrap().to_owned();
    m4.ok(&["exchange", exchange.to_str().unwrap()]);
    std::fs::create_dir_all(m4.volumes()).unwrap();
    let mut serve = m4
        .command(&["serve"])
        // A long interval: only a change can make it publish again.
        .env("NU9VE_AGENT_EVERY_SECS", "3600")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let folder = exchange.join("devices").join(&id);
    assert!(eventually(10, || folder.join("device.json").exists()));
    assert_eq!(m4.ok(&["status"])["serving_pid"], serve.id());

    // A second agent on the same machine refuses to serve.
    let second = m4.run(&["serve"]).unwrap_err();
    assert!(second.contains("already serving"), "{second}");

    // A disk mounts: the agent publishes it without waiting for its interval.
    mount(&m4, "MEDIA501");
    assert!(eventually(10, || {
        read_json(&folder.join("volumes.json")).is_some_and(|r| {
            r["body"]["sightings"].as_array().is_some_and(|l| {
                l.iter()
                    .any(|s| s["label"] == "MEDIA501" && s["mounted"] == true)
            })
        })
    }));

    // SIGTERM stops it in order: exit 0, and the lock is free again.
    assert!(stop(&mut serve).success());
    assert!(m4.ok(&["status"])["serving_pid"].is_null());
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
#[test]
fn install_update_and_uninstall_in_qa_never_touch_identity_or_startup() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let exchange = tmp.path().join("exchange");
    let m = Machine::new(tmp.path(), "m1");
    // HOME is a temporary folder too: even a mistake could not reach the person's LaunchAgents.
    let run = |args: &[&str]| -> Value {
        let out = m.command(args).env("HOME", &home).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let agent_json = m.agent.join("agent.json");
    let write_installed = |version: &str, build: &str| {
        std::fs::write(&agent_json, serde_json::json!({"contract": "nu9ve-agent/1", "version": version,
            "build": build, "installed_at": "2026-11-22T22:00:00Z", "installed_by": "una-app", "binary": "nu9ve"}).to_string()).unwrap();
    };
    let version = env!("CARGO_PKG_VERSION");

    let absent = run(&["state"]);
    assert_eq!(absent["state"], "absent");
    assert_eq!(absent["action"], "install");

    let installed = run(&["install", "--by", "una-app"]);
    assert_eq!(installed["action"], "install");
    assert_eq!(installed["startup"], "none");
    let binary = m.agent.join("nu9ve");
    assert_eq!(
        (mode(&m.agent), mode(&binary), mode(&agent_json)),
        (0o700, 0o700, 0o600)
    );
    let record = read_json(&agent_json).unwrap();
    assert_eq!(
        (record["version"].as_str(), record["installed_by"].as_str()),
        (Some(version), Some("una-app"))
    );
    let reported = Command::new(&binary).arg("--version").output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&reported.stdout).trim(),
        format!("nu9ve {version}")
    );
    assert_eq!(run(&["state"])["state"], "installed");
    assert!(
        m.command(&["install", "--by", "Una-App"])
            .output()
            .unwrap()
            .status
            .code()
            != Some(0)
    );

    // Same version and build: keep. Never down, and a public build never replaces a cloud one.
    assert_eq!(run(&["install"])["action"], "keep");
    write_installed("999.0.0", "public");
    assert_eq!(run(&["install"])["action"], "keep");
    assert_eq!(run(&["state"])["state"], "installed");
    write_installed(version, "cloud");
    assert_eq!(run(&["install"])["action"], "keep");
    assert_eq!(read_json(&agent_json).unwrap()["build"], "cloud");

    // An older installed agent: outdated, and install updates it, stopping the running one first.
    run(&["init"]);
    run(&["exchange", exchange.to_str().unwrap()]);
    write_installed("0.0.1", "public");
    assert_eq!(run(&["state"])["state"], "outdated");
    let mut serving = Command::new(&binary);
    serving
        .args(["agent", "serve"])
        .env("NU9VE_AGENT_DIR", &m.agent)
        .env("NU9VE_DEVICE_FILE", &m.device)
        .env("NU9VE_ENGINES_DIR", &m.engines)
        .env("NU9VE_VOLUMES_DIR", m.volumes())
        .env("HOME", &home)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut serve = serving.spawn().unwrap();
    assert!(eventually(10, || run(&["status"])["serving_pid"] == serve.id()));
    let updated = run(&["install", "--by", "una-app"]);
    assert_eq!(updated["action"], "update");
    assert_eq!(updated["stopped_running"], true);
    assert!(eventually(5, || serve.try_wait().unwrap().is_some()));
    assert_eq!(read_json(&agent_json).unwrap()["version"], version);

    // Uninstall stops it and removes the binary and agent.json, and nothing of the person.
    let mut serve = serving.spawn().unwrap();
    assert!(eventually(10, || run(&["status"])["serving_pid"] == serve.id()));
    let device_before = std::fs::read(&m.device).unwrap();
    let gone = run(&["uninstall"]);
    assert_eq!(gone["stopped_running"], true);
    assert!(eventually(5, || serve.try_wait().unwrap().is_some()));
    assert!(!binary.exists() && !agent_json.exists());
    assert_eq!(std::fs::read(&m.device).unwrap(), device_before);
    for kept in ["key.qa", "state.json", "config.json"] {
        assert!(m.agent.join(kept).exists(), "{kept}");
    }
    assert!(exchange.join("devices").exists());
    assert_eq!(run(&["state"])["state"], "absent");
    assert!(
        !home.join("Library").exists(),
        "QA never writes a LaunchAgent"
    );
}

/// The record a machine published, from the exchange folder.
fn published(exchange: &Path, device: &str, file: &str) -> Value {
    read_json(&exchange.join("devices").join(device).join(file)).unwrap_or(Value::Null)
}

#[test]
fn two_machines_hand_over_leases_pull_objects_and_share_app_files() {
    let tmp = tempfile::tempdir().unwrap();
    let exchange = tmp.path().join("exchange");
    let (m1, m4) = (
        Machine::new(tmp.path(), "m1"),
        Machine::new(tmp.path(), "m4"),
    );
    let (a, b) = (m1.ok(&["init"]), m4.ok(&["init"]));
    let (id1, id4) = (
        a["device_id"].as_str().unwrap().to_owned(),
        b["device_id"].as_str().unwrap().to_owned(),
    );
    m4.ok(&["name", "M4"]);
    for m in [&m1, &m4] {
        m.ok(&["exchange", exchange.to_str().unwrap()]);
        m.ok(&["publish"]);
    }
    m1.ok(&["pair", &id4, b["shown"].as_str().unwrap()]);
    m4.ok(&["pair", &id1, a["shown"].as_str().unwrap()]);
    let call = |m: &Machine, method: &str, args: Value| m.ok(&["call", method, &args.to_string()]);

    // --- Objects (§7): a small file is sent; a large one is answered with its disk; another is not found.
    mount(&m4, "MEDIA501");
    let small_path = tmp.path().join("m4/files/baches.geojson");
    std::fs::create_dir_all(small_path.parent().unwrap()).unwrap();
    let small: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&small_path, &small).unwrap();
    let sha = m4.ok(&["offer", small_path.to_str().unwrap()])["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let big_dir = m4.volumes().join("MEDIA501/001_raw/dron");
    std::fs::create_dir_all(&big_dir).unwrap();
    let big = std::fs::File::create(big_dir.join("tanda.dng")).unwrap();
    big.set_len(64 * 1024 * 1024 + 1).unwrap();
    let offered = m4.ok(&[
        "offer",
        big_dir.join("tanda.dng").to_str().unwrap(),
        "--dataset",
        "una-app:tanda-7",
    ]);
    assert!(offered["sha256"].is_null(), "over 64 MiB is not hashed");
    assert!(
        m4.run(&["offer", big_dir.join("tanda.dng").to_str().unwrap()])
            .is_err()
    );

    let r_small = call(&m1, "pull", serde_json::json!({"from": id4, "object": sha}));
    let r_big = call(
        &m1,
        "pull",
        serde_json::json!({"from": id4, "dataset": "una-app:tanda-7"}),
    );
    let unknown = "cd".repeat(32);
    call(
        &m1,
        "pull",
        serde_json::json!({"from": id4, "object": unknown}),
    );
    assert_eq!(r_small["status"], "pending");
    assert_eq!(
        published(&exchange, &id1, "requests.json")["body"]["requests"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let m4_pass = m4.ok(&["sync"]);
    assert_eq!(m4_pass["trade"]["answered"].as_array().unwrap().len(), 3);
    let object_file = exchange
        .join("devices")
        .join(&id4)
        .join("objects/sha256")
        .join(&sha);
    assert_eq!(std::fs::read(&object_file).unwrap(), small);
    let entries = published(&exchange, &id4, "objects.json")["body"]["objects"].clone();
    let refusal = |rid: &Value| {
        entries
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["request"] == *rid)
            .unwrap()
            .clone()
    };
    assert_eq!(refusal(&r_small["request"])["sha256"], sha.as_str());
    let too_large = refusal(&r_big["request"]);
    assert_eq!(too_large["refused"], "too_large");
    assert_eq!(too_large["where"]["path"], "001_raw/dron/tanda.dng");
    assert!(
        too_large["where"]["volume_id"].is_string(),
        "the disk it is on"
    );

    // A copy whose hash is not the announced one is never taken in.
    std::fs::write(&object_file, b"otra cosa").unwrap();
    let first = m1.ok(&["sync"]);
    assert_eq!(
        first["trade"]["received"].as_array().unwrap().len(),
        2,
        "only the two refusals"
    );
    assert_eq!(
        call(&m1, "pull", serde_json::json!({"from": id4, "object": sha}))["status"],
        "pending"
    );
    std::fs::write(&object_file, &small).unwrap();
    let m1_pass = m1.ok(&["sync"]);
    assert_eq!(m1_pass["trade"]["received"].as_array().unwrap().len(), 1);
    let got = call(&m1, "pull", serde_json::json!({"from": id4, "object": sha}));
    assert_eq!(got["status"], "received");
    assert_eq!(std::fs::read(got["path"].as_str().unwrap()).unwrap(), small);
    let got = call(
        &m1,
        "pull",
        serde_json::json!({"from": id4, "dataset": "una-app:tanda-7"}),
    );
    assert_eq!(
        (got["status"].as_str(), got["refused"].as_str()),
        (Some("refused"), Some("too_large"))
    );
    assert_eq!(got["where_untrusted"]["path"], "001_raw/dron/tanda.dng");
    assert_eq!(
        got["where_untrusted"]["label"], "MEDIA501",
        "the disk by its name"
    );
    let got = call(
        &m1,
        "pull",
        serde_json::json!({"from": id4, "object": unknown}),
    );
    assert_eq!(got["refused"], "not_found");
    assert!(
        published(&exchange, &id1, "requests.json")["body"]["requests"]
            .as_array()
            .unwrap()
            .is_empty(),
        "answered requests are dropped"
    );
    let found = call(&m1, "where", serde_json::json!({"object": sha}));
    assert!(
        found["found"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["here"] == true)
    );
    // Nobody asks any more: the M4 keeps its answers for a while (7 days), not forever.
    m4.ok(&["sync"]);
    assert_eq!(
        published(&exchange, &id4, "objects.json")["body"]["objects"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    // --- Leases (§7.1): handed over without clocks, and a conflict when nobody handed it over.
    let lease = serde_json::json!({"app": "una-app", "project_key": "subaereo"});
    assert_eq!(call(&m4, "lease_take", lease.clone())["taken"], true);
    m1.ok(&["sync"]);
    let taken = call(&m1, "lease_take", lease.clone());
    assert_eq!(taken["from_device_id"], id4.as_str());
    let both = call(&m1, "leases", serde_json::json!({}));
    let rows = both["leases"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|r| r["conflict"] == false && r["handing_over"] == true)
    );
    assert_eq!(
        m4.ok(&["sync"])["trade"]["released"],
        serde_json::json!(["una-app/subaereo"])
    );
    m1.ok(&["sync"]);
    let rows = call(&m1, "leases", serde_json::json!({}))["leases"].clone();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["here"], true);
    // And back: the M4 takes it again, and the M1 lets go.
    call(&m4, "lease_take", lease.clone());
    assert_eq!(
        m1.ok(&["sync"])["trade"]["released"],
        serde_json::json!(["una-app/subaereo"])
    );

    let other = serde_json::json!({"app": "una-app", "project_key": "otro"});
    call(&m1, "lease_take", other.clone());
    call(&m4, "lease_take", other.clone());
    m1.ok(&["sync"]);
    let rows = call(&m1, "leases", serde_json::json!({}))["leases"].clone();
    let otro: Vec<&Value> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["project_key"] == "otro")
        .collect();
    assert_eq!(otro.len(), 2);
    assert!(otro.iter().all(|r| r["conflict"] == true), "{rows}");
    assert!(
        m1.ok(&["sync"])["trade"]["released"]
            .as_array()
            .unwrap()
            .is_empty(),
        "nobody lets go alone"
    );

    // --- apps/<app>/: a file and a diary, signed line by line.
    let app = |extra: Value| {
        let mut args = serde_json::json!({"app": "una-app"});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        args
    };
    call(
        &m4,
        "app_publish",
        app(serde_json::json!({"file": "index.json", "body": {"projects": 1}})),
    );
    for n in [1, 2] {
        call(
            &m4,
            "app_append",
            app(serde_json::json!({"file": "decisions.ndjson", "body": {"n": n}})),
        );
    }
    assert!(
        m4.run(&[
            "call",
            "app_publish",
            &app(serde_json::json!({"file": "../x.json", "body": {}})).to_string()
        ])
        .is_err()
    );
    m1.ok(&["sync"]);
    let seen = call(&m1, "app_read", serde_json::json!({"app": "una-app"}));
    let files = seen["files"].as_array().unwrap();
    let file = |name: &str| files.iter().find(|f| f["file"] == name).unwrap().clone();
    assert_eq!(file("index.json")["body_untrusted"]["projects"], 1);
    assert_eq!(
        file("decisions.ndjson")["lines"].as_array().unwrap().len(),
        2
    );
    assert_eq!(file("decisions.ndjson")["device_name_untrusted"], "M4");

    // A forged line is dropped; the ones signed before stay.
    let diary = exchange
        .join("devices")
        .join(&id4)
        .join("apps/una-app/decisions.ndjson");
    let text = std::fs::read_to_string(&diary).unwrap();
    let mut forged: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    forged["seq"] = serde_json::json!(9999);
    forged["body"] = serde_json::json!({"n": 666});
    std::fs::write(&diary, format!("{text}{forged}\n")).unwrap();
    let read = m1.ok(&["read"]);
    let m4_files = &read["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["device_id"] == id4.as_str())
        .unwrap()["files"];
    assert_eq!(
        m4_files["apps/una-app/decisions.ndjson"]["state"],
        "untrusted"
    );
    let seen = call(
        &m1,
        "app_read",
        serde_json::json!({"app": "una-app", "file": "decisions.ndjson"}),
    );
    let lines = seen["files"][0]["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| l["body_untrusted"]["n"] != 666));

    // --- Undoing: let a lease go, drop a pull and its copy, stop offering, unpair.
    let released = call(&m1, "lease_release", other.clone());
    assert_eq!(released["released"], true);
    assert_eq!(call(&m1, "lease_release", other)["released"], false);
    let received = call(&m1, "pull", serde_json::json!({"from": id4, "object": sha}));
    let copy = PathBuf::from(received["path"].as_str().unwrap());
    assert!(copy.exists());
    let dropped = call(
        &m1,
        "pull",
        serde_json::json!({"from": id4, "object": sha, "drop": true}),
    );
    assert_eq!(
        (
            dropped["dropped"].as_bool(),
            dropped["removed_copy"].as_bool()
        ),
        (Some(true), Some(true))
    );
    assert!(!copy.exists());
    assert_eq!(
        call(&m4, "unoffer", serde_json::json!({"path": small_path}))["unoffered"],
        true
    );
    // A new request for it is now not found, and unpairing drops what is still pending with the M4.
    call(&m1, "pull", serde_json::json!({"from": id4, "object": sha}));
    m1.ok(&["sync"]);
    m4.ok(&["sync"]);
    m1.ok(&["sync"]);
    assert_eq!(
        call(&m1, "pull", serde_json::json!({"from": id4, "object": sha}))["refused"],
        "not_found"
    );
    let pending = call(
        &m1,
        "pull",
        serde_json::json!({"from": id4, "object": "ef".repeat(32)}),
    );
    assert_eq!(pending["status"], "pending");
    m1.ok(&["unpair", &id4]);
    assert!(
        published(&exchange, &id1, "requests.json")["body"]["requests"]
            .as_array()
            .unwrap()
            .is_empty(),
        "nothing pending with an unpaired machine"
    );
}

#[cfg(unix)]
#[test]
fn a_long_agent_folder_still_serves_the_suite_engine() {
    let tmp = tempfile::tempdir().unwrap();
    // Deep enough that <agent>/suite.sock does not fit in sun_path (104 bytes on macOS).
    let deep = tmp
        .path()
        .join("una-carpeta-con-un-nombre-largo/y-otra-mas-para-pasar-el-limite/de-la-ruta");
    let m = Machine::new(&deep, "m4");
    m.ok(&["init"]);
    let mut serve = m
        .command(&["serve"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let record = m.engines.join("suite.json");
    assert!(eventually(10, || record.exists()));
    let path = read_json(&record).unwrap()["transport"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(path.len() < 104, "{path}");
    assert!(!path.starts_with(m.agent.to_str().unwrap()));
    assert!(stop(&mut serve).success());
    assert!(!record.exists() && !Path::new(&path).exists());
}

/// The catalog of the export examples (data/export-examples-v1.json): its app's name, and the catalog.
#[cfg(unix)]
fn export_catalog() -> (String, Value) {
    let examples: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/export-examples-v1.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let catalog: Value = serde_json::from_str(examples["catalog"].as_str().unwrap()).unwrap();
    (catalog["app"].as_str().unwrap().to_owned(), catalog)
}

#[cfg(unix)]
#[test]
fn two_machines_pull_datasets_their_app_exports() {
    use common::{Exports, fake_engine, ready};
    use serde_json::json;

    let tmp = tempfile::tempdir().unwrap();
    let exchange = tmp.path().join("exchange");
    let (m1, m4) = (
        Machine::new(tmp.path(), "m1"),
        Machine::new(tmp.path(), "m4"),
    );
    let (a, b) = (m1.ok(&["init"]), m4.ok(&["init"]));
    let id4 = b["device_id"].as_str().unwrap().to_owned();
    for m in [&m1, &m4] {
        m.ok(&["exchange", exchange.to_str().unwrap()]);
        m.ok(&["publish"]);
    }
    m1.ok(&["pair", &id4, b["shown"].as_str().unwrap()]);
    m4.ok(&[
        "pair",
        a["device_id"].as_str().unwrap(),
        a["shown"].as_str().unwrap(),
    ]);
    let call = |m: &Machine, method: &str, args: Value| m.ok(&["call", method, &args.to_string()]);

    // The M4: an app with a catalog (a small public table, a large raw on a disk, a personal one, one
    // without a size) that exports what it can. Nothing is offered by hand.
    let (app, catalog) = export_catalog();
    let ds = |id: &str| format!("{app}:{id}");
    let files = tmp.path().join("m4/export");
    std::fs::create_dir_all(&files).unwrap();
    let baches = files.join("baches.geojson");
    std::fs::write(&baches, br#"{"type":"FeatureCollection","features":[]}"#).unwrap();
    let notas = files.join("notas.md");
    std::fs::write(&notas, b"# notas\n").unwrap();
    let exports = Exports::default();
    exports.set(
        "baches-2026-11",
        ready("baches-2026-11", &baches, "application/geo+json"),
    );
    exports.set("notas", ready("notas", &notas, "text/markdown"));
    exports.set(
        "sin-tamano",
        json!({"contract": "nu9ve-export/1", "dataset": "sin-tamano", "status": "pending"}),
    );
    let _engine = fake_engine(
        &m4.engines,
        &tmp.path().join("sockets"),
        &app,
        catalog,
        Some(exports.clone()),
    );
    m4.ok(&["publish"]);

    let pull = |dataset: &str| call(&m1, "pull", json!({"from": id4, "dataset": dataset}));
    let wanted = ["baches-2026-11", "tanda-7", "notas", "sin-tamano", "otro"];
    for id in wanted {
        assert_eq!(pull(&ds(id))["status"], "pending");
    }
    let pass = m4.ok(&["sync"]);
    assert_eq!(
        pass["trade"]["answered"].as_array().unwrap().len(),
        4,
        "all but the one its app is still preparing: {pass}"
    );
    // The engine is asked only for what is small, listed and shared when personal.
    let mut asked = exports.asked();
    asked.sort();
    assert_eq!(asked, vec!["baches-2026-11", "sin-tamano"]);

    m1.ok(&["sync"]);
    let got = pull(&ds("baches-2026-11"));
    assert_eq!(got["status"], "received", "{got}");
    assert_eq!(got["media_type_untrusted"], "application/geo+json");
    assert_eq!(
        std::fs::read(got["path"].as_str().unwrap()).unwrap(),
        std::fs::read(&baches).unwrap()
    );
    let got = pull(&ds("tanda-7"));
    assert_eq!(got["refused"], "too_large");
    assert_eq!(got["where_untrusted"]["path"], "001_raw/dron/tanda-7");
    assert!(got["where_untrusted"]["volume_id"].is_string(), "its disk");
    assert_eq!(pull(&ds("notas"))["refused"], "not_shared");
    assert_eq!(pull(&ds("otro"))["refused"], "not_found");
    assert_eq!(pull(&ds("sin-tamano"))["status"], "pending");

    // Ready, but the file is not what the engine announced: nothing is answered, and it is asked again.
    let tabla = files.join("tabla.csv");
    std::fs::write(&tabla, b"a,b\n1,2\n").unwrap();
    let mut wrong = ready("sin-tamano", &tabla, "text/csv");
    wrong["sha256"] = json!("00".repeat(32));
    exports.set("sin-tamano", wrong);
    assert!(
        m4.ok(&["sync"])["trade"]["answered"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    exports.set("sin-tamano", ready("sin-tamano", &tabla, "text/csv"));
    assert_eq!(
        m4.ok(&["sync"])["trade"]["answered"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    m1.ok(&["sync"]);
    let got = pull(&ds("sin-tamano"));
    assert_eq!(got["status"], "received");
    assert_eq!(
        std::fs::read(got["path"].as_str().unwrap()).unwrap(),
        b"a,b\n1,2\n"
    );

    // The person shares the personal one on the M4: asked again, its app exports it.
    call(&m4, "share", json!({"dataset": ds("notas")}));
    assert_eq!(
        call(&m4, "shared", json!({"app": app}))["shared"],
        json!([ds("notas")])
    );
    assert!(
        call(&m4, "shared", json!({"app": "otra-app"}))["shared"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    m4.ok(&["publish"]);
    call(
        &m1,
        "pull",
        json!({"from": id4, "dataset": ds("notas"), "drop": true}),
    );
    pull(&ds("notas"));
    m4.ok(&["sync"]);
    m1.ok(&["sync"]);
    let got = pull(&ds("notas"));
    assert_eq!(got["status"], "received", "{got}");
    assert_eq!(
        std::fs::read(got["path"].as_str().unwrap()).unwrap(),
        b"# notas\n"
    );

    // Export §5: the app offers what is shared by itself, for when it is closed. Unsharing stops that
    // offer, and an offer of a personal dataset that is not shared is still refused.
    let offline = files.join("notas-2.md");
    std::fs::write(&offline, b"# notas 2\n").unwrap();
    let offered = call(
        &m4,
        "offer",
        json!({"path": offline, "dataset": ds("notas")}),
    );
    assert_eq!(offered["offered"], true);
    let unshared = call(&m4, "unshare", json!({"dataset": ds("notas")}));
    assert_eq!(unshared["unoffered"], 1);
    assert!(
        m4.run(&[
            "call",
            "share",
            &json!({"dataset": "sin-dueno"}).to_string()
        ])
        .is_err()
    );
    let sha = call(
        &m4,
        "offer",
        json!({"path": offline, "dataset": ds("notas")}),
    )["sha256"]
        .clone();
    m4.ok(&["publish"]);
    call(
        &m1,
        "pull",
        json!({"from": id4, "dataset": ds("notas"), "drop": true}),
    );
    pull(&ds("notas"));
    call(&m1, "pull", json!({"from": id4, "object": sha}));
    m4.ok(&["sync"]);
    m1.ok(&["sync"]);
    assert_eq!(pull(&ds("notas"))["refused"], "not_shared");
    assert_eq!(
        call(&m1, "pull", json!({"from": id4, "object": sha}))["refused"],
        "not_shared"
    );

    // With the app closed, a new request waits: nothing is answered until its engine is back.
    drop(_engine);
    exports.set(
        "baches-2026-11",
        ready("baches-2026-11", &baches, "application/geo+json"),
    );
    call(
        &m1,
        "pull",
        json!({"from": id4, "dataset": ds("baches-2026-11"), "drop": true}),
    );
    pull(&ds("baches-2026-11"));
    assert!(
        m4.ok(&["sync"])["trade"]["answered"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
