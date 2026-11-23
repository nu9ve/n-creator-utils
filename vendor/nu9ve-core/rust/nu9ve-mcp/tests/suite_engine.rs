//! The agent as the `suite` engine, seen through `nu9ve mcp`: two QA machines share an exchange folder,
//! the M4 has a fake engine with a catalog, the M1 serves its agent, and an MCP client asks the M1.
//! The fake app takes its name and catalog from data/data-examples-v1.json, so this file names none.
#![cfg(any(target_os = "macos", target_os = "linux"))]

mod common;

use common::{Exports, fake_engine, ready};
use nu9ve_ipc::Record;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

struct Machine {
    root: PathBuf,
}

impl Machine {
    fn new(root: &Path, name: &str) -> Self {
        let root = root.join(name);
        std::fs::create_dir_all(root.join("volumes")).unwrap();
        Machine { root }
    }

    fn engines(&self) -> PathBuf {
        self.root.join("engines")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nu9ve"));
        command
            .args(args)
            .env("NU9VE_AGENT_DIR", self.root.join("agent"))
            .env("NU9VE_DEVICE_FILE", self.root.join("device.json"))
            .env("NU9VE_ENGINES_DIR", self.engines())
            .env("NU9VE_VOLUMES_DIR", self.root.join("volumes"));
        command
    }

    fn ok(&self, args: &[&str]) -> Value {
        let mut all = vec!["agent"];
        all.extend_from_slice(args);
        let out = self.command(&all).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}

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

fn examples(file: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The example catalog with a personal dataset: (app, catalog).
fn catalog_with_personal() -> (String, Value) {
    examples("data-examples-v1.json")["valid"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["app"].as_str().unwrap().to_owned(),
                serde_json::from_str::<Value>(c["text"].as_str().unwrap()).unwrap(),
            )
        })
        .find(|(_, c)| {
            c["datasets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["privacy"] == "personal")
        })
        .unwrap()
}

struct Client {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Value>,
    next: u64,
}

impl Client {
    fn start(engines: &Path, actions: bool, confirm: &str) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nu9ve"));
        command.arg("mcp");
        if actions {
            command.arg("--allow-actions");
        }
        let mut child = command
            .env("NU9VE_ENGINES_DIR", engines)
            .env("NU9VE_QA_MCP_CONFIRM", confirm)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if send.send(serde_json::from_str(&line).unwrap()).is_err() {
                    return;
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            lines,
            next: 1,
        };
        client.rpc(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "clientInfo": {"name": "Prueba"}}),
        );
        client
    }

    fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        )
        .unwrap();
        loop {
            let message = self
                .lines
                .recv_timeout(Duration::from_secs(20))
                .expect("reply");
            if message.get("id") == Some(&json!(id)) {
                return message;
            }
        }
    }

    fn names(&mut self) -> Vec<String> {
        self.rpc("tools/list", json!({}))["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The structured result of a tool, or its error text.
    fn call(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let reply = self.rpc("tools/call", json!({"name": name, "arguments": arguments}));
        let result = &reply["result"];
        if result["isError"] == true {
            Err(result["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_owned())
        } else {
            Ok(result["structuredContent"].clone())
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn suite_record(engines: &Path) -> Option<Record> {
    match nu9ve_ipc::read_entry(engines, "suite").ok()?? {
        nu9ve_ipc::Entry {
            state: nu9ve_ipc::State::Live(r),
            ..
        } => Some(r),
        _ => None,
    }
}

#[test]
fn the_suite_engine_through_nu9ve_mcp() {
    let tmp = tempfile::tempdir().unwrap();
    let exchange = tmp.path().join("exchange");
    let (m1, m4) = (
        Machine::new(tmp.path(), "m1"),
        Machine::new(tmp.path(), "m4"),
    );
    let id1 = m1.ok(&["init"])["device_id"].as_str().unwrap().to_owned();
    let b = m4.ok(&["init"]);
    let id4 = b["device_id"].as_str().unwrap().to_owned();
    m4.ok(&["name", "M4"]);
    for m in [&m1, &m4] {
        m.ok(&["exchange", exchange.to_str().unwrap()]);
        m.ok(&["publish"]);
    }
    m1.ok(&["pair", &id4, b["shown"].as_str().unwrap()]);
    let a = m1.ok(&["status"]);
    m4.ok(&["pair", &id1, a["shown"].as_str().unwrap()]);

    // The M4: an app with a catalog, a disk mounted, the personal dataset shared between machines.
    let (app, mut catalog) = catalog_with_personal();
    // Small enough to travel whole (the example's collection is larger than 64 MiB).
    let personal = catalog["datasets"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|d| d["privacy"] == "personal")
        .unwrap();
    personal["size"]["bytes"] = json!(22);
    let personal = personal.clone();
    let personal_id = format!(
        "{}:{}",
        personal["owner"].as_str().unwrap(),
        personal["id"].as_str().unwrap()
    );
    let project = catalog["datasets"][0]["projects"][0]["project_key"]
        .as_str()
        .unwrap()
        .to_owned();
    // Its app exports the personal dataset (nu9ve-export-v1): a small file, ready when asked.
    let exports = Exports::default();
    let personal_file = m4.root.join("export/personal.ndjson");
    std::fs::create_dir_all(personal_file.parent().unwrap()).unwrap();
    std::fs::write(&personal_file, b"{\"turn\":1}\n{\"turn\":2}\n").unwrap();
    let local_id = personal["id"].as_str().unwrap();
    exports.set(
        local_id,
        ready(local_id, &personal_file, "application/x-ndjson"),
    );
    let _engine = fake_engine(
        &m4.engines(),
        &tmp.path().join("sockets"),
        &app,
        catalog.clone(),
        Some(exports.clone()),
    );
    let marker = examples("volume-examples-v1.json")["markers"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::create_dir_all(m4.root.join("volumes/MEDIA501/.nu9ve")).unwrap();
    std::fs::write(m4.root.join("volumes/MEDIA501/.nu9ve/volume.json"), marker).unwrap();
    m4.ok(&["share", &personal_id]);
    let written = m4.ok(&["publish"]);
    assert!(
        written["written"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == &json!(format!("catalog/{app}.json"))),
        "{written}"
    );

    // The M1 serves its agent: the suite engine shows up live in its registry.
    let mut serve = m1
        .command(&["agent", "serve"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(eventually(10, || suite_record(&m1.engines()).is_some()));
    let record = suite_record(&m1.engines()).unwrap();
    for capability in ["devices_v1", "exchange_v1", "mcp_tools_v1"] {
        assert!(record.has_capability(capability));
    }
    assert_eq!(
        m1.ok(&["state"])["state"],
        "absent",
        "serving by hand is not installing"
    );

    // Read-only by default: the four read tools, and the server's own.
    let mut client = Client::start(&m1.engines(), false, "no");
    let names = client.names();
    for tool in [
        "suite_devices",
        "suite_volumes",
        "suite_where",
        "suite_leases",
        "suite_engines",
    ] {
        assert!(names.contains(&tool.to_owned()), "{tool}: {names:?}");
    }
    assert!(
        !names
            .iter()
            .any(|n| n == "suite_lease_take" || n == "suite_pull")
    );

    let devices = client.call("suite_devices", json!({})).unwrap();
    assert_eq!(devices["this"]["device_id"], id1.as_str());
    let other = &devices["devices"][0];
    assert_eq!(other["device_id"], id4.as_str());
    assert_eq!(other["paired"], true);
    assert_eq!(other["name_untrusted"], "M4");
    assert_eq!(other["catalogs"][0]["app"], app.as_str());
    assert_eq!(other["volumes"], 1);
    assert!(
        other.get("name").is_none(),
        "another machine's name is _untrusted"
    );

    let volumes = client.call("suite_volumes", json!({})).unwrap();
    let disk = &volumes["volumes"][0];
    assert_eq!(disk["mounted_here"], false);
    assert_eq!(disk["seen"][0]["device_id"], id4.as_str());
    assert_eq!(disk["seen"][0]["mounted"], true);
    assert_eq!(disk["seen"][0]["label_untrusted"], "MEDIA501");

    let found = client
        .call("suite_where", json!({"project_key": project}))
        .unwrap();
    let rows = found["found"].as_array().unwrap();
    assert!(!rows.is_empty());
    assert!(
        rows.iter()
            .all(|r| r["device_id"] == id4.as_str() && r["here"] == false)
    );
    assert!(
        rows.iter()
            .any(|r| r["dataset"]["title_untrusted"].is_string())
    );

    // Shared between machines, a personal dataset is still only its shape through MCP.
    let found = client
        .call("suite_where", json!({"dataset": personal_id}))
        .unwrap();
    let only = &found["found"][0]["dataset"];
    assert_eq!(only["privacy"], "personal");
    for hidden in [
        "title_untrusted",
        "title",
        "location",
        "projects",
        "volume_id",
    ] {
        assert!(only.get(hidden).is_none(), "{hidden}: {only}");
    }
    assert!(
        client
            .call("suite_where", json!({}))
            .unwrap_err()
            .starts_with("invalid_params:")
    );
    drop(client);

    // Editing: refused without the person's yes, and done with it.
    let mut refused = Client::start(&m1.engines(), true, "no");
    let denied = refused
        .call(
            "suite_lease_take",
            json!({"app": app, "project_key": project}),
        )
        .unwrap_err();
    assert!(denied.starts_with("permission_denied:"), "{denied}");
    drop(refused);
    let folder1 = exchange.join("devices").join(&id1);
    assert!(!folder1.join("leases.json").exists());

    let mut client = Client::start(&m1.engines(), true, "yes");
    let taken = client
        .call(
            "suite_lease_take",
            json!({"app": app, "project_key": project}),
        )
        .unwrap();
    assert_eq!(taken["taken"], true);
    let leases = client.call("suite_leases", json!({})).unwrap();
    assert_eq!(leases["leases"][0]["here"], true);
    assert_eq!(leases["leases"][0]["conflict"], false);
    let sha = "ab".repeat(32);
    let pulled = client
        .call("suite_pull", json!({"from": id4, "object": sha}))
        .unwrap();
    assert_eq!(pulled["status"], "pending");
    let again = client
        .call("suite_pull", json!({"from": id4, "object": sha}))
        .unwrap();
    assert_eq!(again["request"], pulled["request"]);
    let requests: Value =
        serde_json::from_slice(&std::fs::read(folder1.join("requests.json")).unwrap()).unwrap();
    assert_eq!(requests["body"]["requests"][0]["to"], id4.as_str());
    assert!(
        client
            .call("suite_pull", json!({"from": id1, "object": sha}))
            .unwrap_err()
            .starts_with("invalid_params:"),
        "only from a paired machine"
    );
    let changes = std::fs::read_to_string(m1.root.join("agent/changes.ndjson")).unwrap();
    assert_eq!(
        changes.lines().count(),
        3,
        "every edit is logged with who asked"
    );
    assert!(changes.contains("\"agent\":\"Prueba\""));

    // A dataset its app exports: asked by MCP on the M1, exported by the engine on the M4, received.
    // Shared between machines, the personal one goes whole; MCP never asks an engine for bytes itself.
    assert!(!client.names().iter().any(|n| n.contains("export")));
    assert!(exports.asked().is_empty());
    let pulled = client
        .call("suite_pull", json!({"from": id4, "dataset": personal_id}))
        .unwrap();
    assert_eq!(pulled["status"], "pending");
    let answered = m4.ok(&["sync"]);
    assert_eq!(
        answered["trade"]["answered"].as_array().unwrap().len(),
        2,
        "the object it does not have, and the dataset: {answered}"
    );
    assert_eq!(exports.asked(), vec![local_id.to_owned()]);
    m1.ok(&["sync"]);
    let got = client
        .call("suite_pull", json!({"from": id4, "dataset": personal_id}))
        .unwrap();
    assert_eq!(got["status"], "received", "{got}");
    assert_eq!(got["media_type_untrusted"], "application/x-ndjson");
    assert_eq!(
        std::fs::read(got["path"].as_str().unwrap()).unwrap(),
        std::fs::read(&personal_file).unwrap()
    );
    drop(client);

    // engine_shutdown stops the agent in order: exit 0, record and socket gone.
    nu9ve_ipc::request(
        &record,
        "engine_shutdown",
        None,
        None,
        nu9ve_ipc::DEFAULT_TIMEOUT,
    )
    .unwrap();
    assert!(eventually(10, || serve.try_wait().unwrap().is_some()));
    assert!(serve.wait().unwrap().success());
    assert!(!m1.engines().join("suite.json").exists());
    assert!(!m1.root.join("agent/suite.sock").exists());
}
