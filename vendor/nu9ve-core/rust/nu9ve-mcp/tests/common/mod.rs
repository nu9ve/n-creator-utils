//! A fake engine for the agent tests: it serves `data_catalog` and, if given exports, `data_export`
//! (nu9ve-export-v1), and is registered in a QA engines folder. It names no app: the tests pass one.
#![allow(dead_code)]

use nu9ve_engine::{EngineInfo, Reply, Request, RpcError, serve_frames};
use nu9ve_ipc::{EngineRef, Record, Registration, Transport};
use serde_json::{Map, Value};
use std::io::BufReader;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// What `data_export` answers, by dataset id, and the ids it was asked for. The test changes it while
/// the engine serves.
#[derive(Clone, Default)]
pub struct Exports {
    pub replies: Arc<Mutex<Map<String, Value>>>,
    pub asked: Arc<Mutex<Vec<String>>>,
}

impl Exports {
    pub fn set(&self, id: &str, reply: Value) {
        self.replies.lock().unwrap().insert(id.to_owned(), reply);
    }

    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

/// Registers in `dir` an engine for `app` that serves `catalog` and, with `exports`, `data_export`.
pub fn fake_engine(
    dir: &Path,
    sockets: &Path,
    app: &str,
    catalog: Value,
    exports: Option<Exports>,
) -> Registration {
    std::fs::create_dir_all(sockets).unwrap();
    std::fs::set_permissions(sockets, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = sockets.join(format!("{app}.sock"));
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    let capabilities: &'static [&'static str] = if exports.is_some() {
        &["data_catalog_v1", "data_export_v1"]
    } else {
        &["data_catalog_v1"]
    };
    let served = exports.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let reader = BufReader::new(stream.try_clone().unwrap());
            let info = EngineInfo {
                protocol: "test/1",
                version: "0.1.0",
                capabilities,
                ported: &[],
                native: &["data_catalog", "data_export"],
                extra: Map::new(),
            };
            let _ = serve_frames(reader, &stream, info, |r: Request<'_>| {
                match (r.method, &served) {
                    ("data_catalog", _) => Ok(Reply::Data(catalog.clone())),
                    ("data_export", Some(exports)) => {
                        let id = r
                            .params
                            .get("dataset")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        exports.asked.lock().unwrap().push(id.clone());
                        let reply = exports.replies.lock().unwrap().get(&id).cloned();
                        Ok(Reply::Data(reply.unwrap_or_else(|| {
                        serde_json::json!({"contract": "nu9ve-export/1", "dataset": id, "status": "not_found"})
                    })))
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
        capabilities.iter().map(|c| (*c).to_owned()).collect(),
    )
    .unwrap();
    Registration::register(dir, &record).unwrap()
}

/// A `ready` reply of nu9ve-export-v1 §2 for a file the test wrote.
pub fn ready(id: &str, path: &Path, media_type: &str) -> Value {
    let bytes = std::fs::read(path).unwrap();
    serde_json::json!({"contract": "nu9ve-export/1", "dataset": id, "status": "ready", "path": path,
        "bytes": bytes.len(), "media_type": media_type, "sha256": nu9ve_json::sha256_hex(&bytes)})
}
