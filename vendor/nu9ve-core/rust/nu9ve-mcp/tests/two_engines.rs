//! `nu9ve mcp` end to end: the real binary, two fake engines serving nu9ve-engine-v1 frames
//! on private sockets, and the registry in a temporary NU9VE_ENGINES_DIR.
//! The fake apps take their names from data/data-examples-v1.json, so this file names none.
#![cfg(any(target_os = "macos", target_os = "linux"))]

use nu9ve_engine::{EngineInfo, Reply, Request, RpcError, serve_frames};
use nu9ve_ipc::{EngineRef, Record, Registration, Transport};
use serde_json::{Map, Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Log = Arc<Mutex<Vec<Value>>>;

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/data-examples-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The two valid catalogs of the examples: (app, catalog).
fn catalogs() -> Vec<(String, Value)> {
    examples()["valid"].as_array().unwrap()[..2]
        .iter()
        .map(|c| {
            (
                c["app"].as_str().unwrap().to_owned(),
                serde_json::from_str(c["text"].as_str().unwrap()).unwrap(),
            )
        })
        .collect()
}

struct FakeEngine {
    log: Log,
    registration: Option<Registration>,
}

/// Tools: `status` (read), `show_item` (show), `item_save` (edit), `refresh_now` (act),
/// one already prefixed with the app, and two the server must omit.
fn tools(app: &str) -> Value {
    json!({"instructions": "Motor de prueba.", "tools": [
        {"name": "status", "title": "Estado", "description": "Estado.", "level": "read",
         "input_schema": {"type": "object", "properties": {}, "additionalProperties": false}},
        {"name": "show_item", "title": "Mostrar", "description": "Muestra.", "level": "show"},
        {"name": "item_save", "title": "Guardar", "description": "Guarda.", "level": "edit"},
        {"name": "refresh_now", "title": "Consultar ya", "description": "Consulta.", "level": "act"},
        {"name": format!("{}_summary", app.replace('-', "_")), "title": "Resumen", "description": "Resumen.", "level": "read"},
        {"name": "Bad Name", "level": "read"},
        {"name": "admin_all", "level": "admin"},
    ]})
}

/// The catalog in two pages when it has more than one dataset, to exercise the cursor.
fn page(catalog: &Value, cursor: Option<&str>) -> Value {
    let mut page = catalog.clone();
    let datasets = catalog["datasets"].as_array().unwrap();
    if cursor.is_none() && datasets.len() > 1 {
        page["datasets"] = json!(datasets[..1]);
        page["next_cursor"] = json!("p2");
    } else if cursor == Some("p2") {
        page["datasets"] = json!(datasets[1..]);
    }
    page
}

fn start_engine(dir: &Path, sockets: &Path, app: &str, catalog: Value) -> FakeEngine {
    let socket = sockets.join(format!("{app}.sock"));
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    let log: Log = Arc::default();
    let (thread_log, thread_app) = (log.clone(), app.to_owned());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let reader = BufReader::new(stream.try_clone().unwrap());
            let info = EngineInfo {
                protocol: "test/1",
                version: "0.1.0",
                capabilities: &["data_catalog_v1", "mcp_tools_v1"],
                ported: &[],
                native: &["data_catalog", "mcp_tools", "mcp_call"],
                extra: Map::new(),
            };
            let _ = serve_frames(reader, &stream, info, |request: Request<'_>| {
                let params = Value::Object(request.params.clone());
                match request.method {
                    "mcp_tools" => Ok(Reply::Data(tools(&thread_app))),
                    "data_catalog" => Ok(Reply::Data(page(&catalog, params["cursor"].as_str()))),
                    "mcp_call" => {
                        let mut entry = params.clone();
                        entry["idempotency_key"] = json!(request.idempotency_key);
                        thread_log.lock().unwrap().push(entry);
                        Ok(Reply::Data(match params["tool"].as_str() {
                            Some("status") => {
                                json!({"version": "0.1.0", "note_untrusted": "hola\u{202e} mundo\n"})
                            }
                            _ => json!({"done": params["tool"]}),
                        }))
                    }
                    _ => Err(RpcError::new("unknown_method", "No such method.")),
                }
            });
        }
    });
    let record = Record::for_current_process(
        app,
        "1.0.0",
        Transport::Unix { path: socket },
        EngineRef {
            contract: "nu9ve-engine/1".into(),
            protocol: "test/1".into(),
            engine: "rust".into(),
            version: "0.1.0".into(),
        },
        vec!["data_catalog_v1".into(), "mcp_tools_v1".into()],
    )
    .unwrap();
    FakeEngine {
        log,
        registration: Some(Registration::register(dir, &record).unwrap()),
    }
}

struct Suite {
    _root: tempfile::TempDir,
    dir: PathBuf,
    engines: Vec<(String, FakeEngine)>,
}

fn suite() -> Suite {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("engines");
    let sockets = root.path().join("sockets");
    std::fs::create_dir(&sockets).unwrap();
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).unwrap();
    let engines = catalogs()
        .into_iter()
        .map(|(app, catalog)| {
            let engine = start_engine(&dir, &sockets, &app, catalog);
            (app, engine)
        })
        .collect();
    // A third app whose pid was reused: same pid, another start. Never live.
    let stale = json!({"contract": "nu9ve-registry/1", "app": "zeta", "app_version": "1.0.0",
        "pid": std::process::id(), "process_start_us": 1, "transport": {"kind": "unix", "path": "/nowhere"},
        "engine": {"contract": "nu9ve-engine/1", "protocol": "test/1", "engine": "rust", "version": "0.1.0"},
        "capabilities": ["mcp_tools_v1"], "written_at": "2026-11-19T00:00:00Z"});
    let path = dir.join("zeta.json");
    std::fs::write(&path, stale.to_string()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    Suite {
        _root: root,
        dir,
        engines,
    }
}

struct Client {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Value>,
    notes: Vec<Value>,
    next: u64,
}

impl Client {
    fn start(dir: &Path, actions: bool, confirm: &str) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nu9ve"));
        command.arg("mcp");
        if actions {
            command.arg("--allow-actions");
        }
        let mut child = command
            .env("NU9VE_ENGINES_DIR", dir)
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
            notes: Vec::new(),
            next: 1,
        };
        let reply = client.rpc(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "clientInfo": {"name": "Prueba\u{7}"}}),
        );
        assert_eq!(reply["result"]["serverInfo"]["name"], "nu9ve");
        assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(
            reply["result"]["capabilities"]["tools"]["listChanged"],
            true
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
            self.notes.push(message);
        }
    }

    fn tool_names(&mut self) -> Vec<String> {
        let reply = self.rpc("tools/list", json!({}));
        reply["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.rpc("tools/call", json!({"name": name, "arguments": arguments}))
    }

    fn wait_note(&mut self, method: &str, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if self.notes.iter().any(|n| n["method"] == method) {
                return true;
            }
            if let Ok(message) = self.lines.recv_timeout(Duration::from_millis(200)) {
                self.notes.push(message);
            }
        }
        false
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text(result: &Value) -> &str {
    result["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn read_only_by_default_with_prefixes_and_levels() {
    let suite = suite();
    let (a, b) = (suite.engines[0].0.clone(), suite.engines[1].0.clone());
    let mut client = Client::start(&suite.dir, false, "yes");
    let reply = client.rpc("tools/list", json!({}));
    let tools = reply["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for app in [&a, &b] {
        assert!(
            names.contains(&format!("{app}_status").as_str()),
            "{names:?}"
        );
        assert!(names.contains(&format!("{app}_show_item").as_str()));
        // Already prefixed: not doubled.
        assert!(names.contains(&format!("{app}_summary").as_str()));
        assert!(
            !names
                .iter()
                .any(|n| n.contains("item_save") || n.contains("refresh_now"))
        );
    }
    assert!(names.contains(&"suite_engines") && names.contains(&"suite_data_catalog"));
    assert!(
        !names.iter().any(|n| n.starts_with("zeta_")),
        "a stale engine is not listed"
    );
    assert!(!names.iter().any(|n| n.contains("admin") || n.contains(' ')));
    assert!(
        tools
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true)
    );

    // Protocol errors.
    assert_eq!(
        client.rpc("resources/list", json!({}))["error"]["code"],
        -32601
    );
    assert_eq!(
        client.call("nothing_here", json!({}))["error"]["code"],
        -32602
    );
    assert_eq!(
        client.call(&format!("{a}_item_save"), json!({}))["error"]["code"],
        -32602
    );

    // A read reaches the engine with the agent's name; observed text arrives cleaned.
    let result = client.call(&format!("{a}_status"), json!({}));
    assert_eq!(result["result"]["isError"], false);
    assert_eq!(
        result["result"]["structuredContent"]["note_untrusted"],
        "hola mundo"
    );
    let log = suite.engines[0].1.log.lock().unwrap().clone();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0]["agent"], "Prueba");
    assert!(log[0].get("grant").is_none() && log[0]["idempotency_key"].is_null());
}

#[test]
fn a_refused_edit_changes_nothing() {
    let suite = suite();
    let a = suite.engines[0].0.clone();
    let mut client = Client::start(&suite.dir, true, "no");
    assert!(client.tool_names().contains(&format!("{a}_item_save")));
    for tool in ["item_save", "refresh_now"] {
        let result = client.call(&format!("{a}_{tool}"), json!({"x": 1}));
        assert_eq!(result["result"]["isError"], true);
        assert!(
            text(&result).starts_with("permission_denied:"),
            "{}",
            text(&result)
        );
    }
    assert!(suite.engines[0].1.log.lock().unwrap().is_empty());
}

#[test]
fn an_accepted_edit_carries_the_grant_once_per_app() {
    let suite = suite();
    let (a, b) = (suite.engines[0].0.clone(), suite.engines[1].0.clone());
    let mut client = Client::start(&suite.dir, true, "yes");
    let first = client.call(&format!("{a}_item_save"), json!({"x": 1}));
    assert_eq!(
        first["result"]["structuredContent"]["edit_permission_minutes_left"],
        60
    );
    client.call(&format!("{a}_item_save"), json!({"x": 2}));
    client.call(&format!("{a}_refresh_now"), json!({}));
    client.call(&format!("{b}_item_save"), json!({}));
    let log = suite.engines[0].1.log.lock().unwrap().clone();
    let grants: Vec<&str> = log.iter().map(|e| e["grant"].as_str().unwrap()).collect();
    assert_eq!(grants, ["edit", "edit", "act"]);
    assert_eq!(log[1]["arguments"], json!({"x": 2}));
    let keys: Vec<&str> = log
        .iter()
        .map(|e| e["idempotency_key"].as_str().unwrap())
        .collect();
    assert!(keys[0] != keys[1], "each write has its own key");
    assert_eq!(suite.engines[1].1.log.lock().unwrap()[0]["grant"], "edit");
}

#[test]
fn suite_tools_aggregate_and_hide_what_is_personal() {
    let suite = suite();
    let (a, b) = (suite.engines[0].0.clone(), suite.engines[1].0.clone());
    let mut client = Client::start(&suite.dir, false, "no");

    let engines = client.call("suite_engines", json!({}));
    let engines = engines["result"]["structuredContent"]["engines"]
        .as_array()
        .unwrap()
        .clone();
    let state = |app: &str| engines.iter().find(|e| e["app"] == app).unwrap().clone();
    assert_eq!(state(&a)["state"], "live");
    assert_eq!(state(&a)["tools"], 5);
    assert_eq!(state(&a)["omitted_tools"], 2);
    assert_eq!(state("zeta")["state"], "stale");
    assert!(engines.iter().all(|e| e.get("pid").is_none()));

    let catalog = client.call("suite_data_catalog", json!({}));
    let data = &catalog["result"]["structuredContent"];
    let refs: Vec<&str> = data["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["ref"].as_str().unwrap())
        .collect();
    assert_eq!(refs.len(), 4, "both pages of each app: {refs:?}");
    for dataset in data["datasets"].as_array().unwrap() {
        assert!(dataset.get("title").is_none());
        if dataset["privacy"] == "personal" {
            for hidden in [
                "title_untrusted",
                "location",
                "hash",
                "projects",
                "origin_key",
            ] {
                assert!(
                    dataset.get(hidden).is_none(),
                    "{hidden} of a personal dataset"
                );
            }
        } else {
            assert!(dataset["title_untrusted"].is_string());
        }
    }
    // The examples share one project_key between the two apps.
    let shared: Vec<&Value> = data["projects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["apps"].get(&a).is_some() && p["apps"].get(&b).is_some())
        .collect();
    assert_eq!(shared.len(), 1);
    assert!(shared[0]["apps"][&a]["title_untrusted"].is_string());

    let only = client.call("suite_data_catalog", json!({"app": b}));
    let apps = only["result"]["structuredContent"]["apps"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["app"], json!(b));
    let bad = client.call("suite_data_catalog", json!({"project_key": "Con Espacios"}));
    assert!(text(&bad).starts_with("invalid_params:"));
}

#[test]
fn closing_an_engine_changes_the_tool_list() {
    let mut suite = suite();
    let b = suite.engines[1].0.clone();
    let mut client = Client::start(&suite.dir, false, "no");
    assert!(client.tool_names().contains(&format!("{b}_status")));
    drop(suite.engines[1].1.registration.take());
    assert!(client.wait_note("notifications/tools/list_changed", Duration::from_secs(10)));
    assert!(
        !client
            .tool_names()
            .iter()
            .any(|n| n.starts_with(&format!("{b}_")))
    );
    // Still registered as stale? No: the file is gone, so the name is simply unknown.
    assert_eq!(
        client.call(&format!("{b}_status"), json!({}))["error"]["code"],
        -32602
    );
}

#[test]
fn a_stale_engine_is_not_running() {
    let suite = suite();
    let mut client = Client::start(&suite.dir, false, "no");
    let result = client.call("zeta_status", json!({}));
    assert!(text(&result).starts_with("not_running:"), "{result}");
}
