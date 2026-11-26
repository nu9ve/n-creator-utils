//! nüün on a transport the suite can find (nu9ve-registry-v1): while the app's `rpc-serve` runs, a Unix socket
//! serves suite methods in frames, one per connection, with the rules of nu9ve-engine-v1 §9,
//! and the engine registers as `nuun` once it accepts connections.
//!
//! Under QA (a `--state-dir`, the sandbox) it registers only in `NU9VE_ENGINES_DIR`, never in the person's folder.
use nu9ve_engine::{EngineInfo, Request, RpcError};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

pub const APP: &str = "nuun";
pub const PROTOCOL: &str = "nuun-suite/1";
pub const CAPABILITIES: &[&str] = &[
    "data_catalog_v1",
    "data_export_v1",
    "conversations_export_working_v1",
    #[cfg(target_os = "macos")]
    "missions_ingest_v1",
];
/// `conversations_export_working` (U1 of nove): the working copy the person already decided, started in the
/// background; `conversations_export_status` answers how that run went.
pub const METHODS: &[&str] = &[
    "data_catalog",
    "data_export",
    "conversations_export_working",
    "conversations_export_status",
    #[cfg(target_os = "macos")]
    "missions_ingest",
];

/// Export runs started from the suite while this engine serves: `run_id` → state.
static RUNS: std::sync::Mutex<Vec<(String, Value)>> = std::sync::Mutex::new(Vec::new());

fn info() -> EngineInfo<'static> {
    EngineInfo {
        protocol: PROTOCOL,
        version: env!("CARGO_PKG_VERSION"),
        capabilities: CAPABILITIES,
        ported: &[],
        native: METHODS,
        extra: Map::new(),
    }
}

/// Starts the decided working copy in a thread; only one at a time. The copy can take longer than a frame.
fn start_export(home: &Path, isolated: bool, params: &Value) -> nu9ve_json::Result<Value> {
    let mut runs = RUNS.lock().unwrap_or_else(|e| e.into_inner());
    nu9ve_json::require(
        !runs.iter().any(|(_, r)| r["status"] == "running"),
        "busy",
        "An export is already running",
    )?;
    let home = home.to_path_buf();
    let user_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| nu9ve_json::Error::new("internal_error", "No home folder"))?;
    // Checked before starting, so a refusal is the answer and not a failed run.
    let backend = nuun_media::volumes::platform_backend(&home)
        .filter(|b| b.writes())
        .ok_or_else(|| {
            nu9ve_json::Error::new("not_ported", "This platform cannot write disks yet")
        })?;
    drop(backend);
    nuun_media::conversations::export_decided_check(&home, params)?;
    let run_id = nu9ve_json::uid()?;
    runs.push((
        run_id.clone(),
        json!({"status": "running", "started_at": nu9ve_time::now_utc()}),
    ));
    if runs.len() > 16 {
        runs.remove(0);
    }
    drop(runs);
    let id = run_id.clone();
    let params = params.clone();
    std::thread::spawn(move || {
        let result = nuun_media::volumes::platform_backend(&home)
            .ok_or_else(|| nu9ve_json::Error::new("not_ported", "No volume backend"))
            .and_then(|b| {
                nuun_media::conversations::export_decided(
                    &home,
                    b.as_ref(),
                    isolated,
                    &user_home,
                    &params,
                )
            });
        let state = match result {
            Ok(data) => {
                json!({"status": "done", "result": data, "finished_at": nu9ve_time::now_utc()})
            }
            Err(e) => json!({"status": "failed", "error": {"code": e.code, "message": e.message},
                             "finished_at": nu9ve_time::now_utc()}),
        };
        let mut runs = RUNS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = runs.iter_mut().find(|(r, _)| *r == id) {
            entry.1 = state;
        }
    });
    Ok(json!({"status": "started", "run_id": run_id}))
}

fn export_status(params: &Value) -> nu9ve_json::Result<Value> {
    let id = params["run_id"].as_str().unwrap_or_default();
    let runs = RUNS.lock().unwrap_or_else(|e| e.into_inner());
    Ok(runs
        .iter()
        .find(|(r, _)| r == id)
        .map(|(r, state)| {
            let mut state = state.clone();
            state["run_id"] = json!(r);
            state
        })
        .unwrap_or_else(|| json!({"run_id": id, "status": "not_found"})))
}

#[cfg(target_os = "macos")]
fn missions_ingest(home: &Path, isolated: bool, params: &Value) -> nu9ve_json::Result<Value> {
    let path = params["path"]
        .as_str()
        .filter(|p| Path::new(p).is_absolute());
    let path = path
        .filter(|_| params["source"] == "nove" && params.as_object().is_some_and(|p| p.len() == 2))
        .ok_or_else(|| {
            nu9ve_json::Error::new(
                "invalid_params",
                "Expected an absolute path and source nove",
            )
        })?;
    let Some(dir) = registry(isolated) else {
        return Ok(json!({"status":"deferred","reason":"nove_unavailable"}));
    };
    let Some(nu9ve_ipc::Entry {
        state: nu9ve_ipc::State::Live(record),
        ..
    }) = nu9ve_ipc::read_entry(&dir, "nove")
        .map_err(|e| nu9ve_json::Error::new("internal_error", e.to_string()))?
    else {
        return Ok(json!({"status":"deferred","reason":"nove_unavailable"}));
    };
    let library = record.library.map(PathBuf::from).or_else(|| {
        let profile_id = record.profile_id?;
        let profiles = nuun_media::suite_profiles::file(isolated);
        let (document, _) = nuun_media::suite_profiles::load(profiles.as_deref());
        nuun_media::suite_profiles::libraries(document.as_ref())
            .into_iter()
            .find(|(id, _, app, _)| id == &profile_id && app == "nove")
            .map(|(_, _, _, library)| library)
    });
    let Some(library) = library else {
        return Ok(json!({"status":"deferred","reason":"nove_library_unavailable"}));
    };
    let backend = nuun_media::volumes::platform_backend(home)
        .ok_or_else(|| nu9ve_json::Error::new("not_ported", "No volume backend"))?;
    nuun_media::mission_review::ingest_nove(home, backend.as_ref(), &library, Path::new(path))
}

/// One request of the suite transport on the catalog of `home`.
pub fn dispatch(home: &Path, isolated: bool, request: Request<'_>) -> nu9ve_engine::Result {
    let params = Value::Object(request.params.clone());
    let result = match request.method {
        "data_catalog" => nuun_media::suite_volume::data_catalog(home, &params),
        "data_export" => nuun_media::suite_volume::data_export(home, &params),
        "conversations_export_working" => start_export(home, isolated, &params),
        "conversations_export_status" => export_status(&params),
        #[cfg(target_os = "macos")]
        "missions_ingest" => missions_ingest(home, isolated, &params),
        other => {
            return Err(RpcError::new(
                "unknown_method",
                format!("Unknown method: {other}"),
            ));
        }
    };
    result
        .map(Into::into)
        .map_err(|e| RpcError::new(&e.code, e.message))
}

/// The same methods in frames on stdin and stdout, for the suite's example checks (`--server`).
pub fn serve_stdio(home: &Path, isolated: bool) -> std::io::Result<()> {
    nu9ve_engine::serve_frames(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        info(),
        |r| dispatch(home, isolated, r),
    )
}

/// The registry folder this engine may write: `NU9VE_ENGINES_DIR` always wins; under QA nothing else.
fn registry(isolated: bool) -> Option<PathBuf> {
    if isolated && std::env::var_os(nu9ve_ipc::ENV_DIR).is_none() {
        return None;
    }
    nu9ve_ipc::default_dir()
}

/// A short private folder for the socket (a Unix socket path has at most 104 bytes on macOS), named after the
/// catalog it serves. Only a folder of this user with no access for others is used.
#[cfg(unix)]
fn socket_path(home: &Path) -> std::io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let tag = &nu9ve_json::sha256_hex(home.as_os_str().as_bytes())[..12];
    let dir = std::env::temp_dir().join(format!("nuun-{tag}"));
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(&dir) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(e),
        _ => {}
    }
    let meta = std::fs::symlink_metadata(&dir)?;
    if !meta.is_dir() || Some(meta.uid()) != nu9ve_ipc::current_uid() || meta.mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("{} is not a private folder of this user", dir.display()),
        ));
    }
    Ok(dir.join("engine.sock"))
}

/// The uid of the process at the other end (engine §9): only this user is served.
#[cfg(unix)]
fn peer_uid(stream: &std::os::unix::net::UnixStream) -> Option<u32> {
    use std::os::unix::io::AsRawFd;
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: getpeereid only writes the two ids of a connected socket we own.
    (unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } == 0).then_some(uid)
}

/// The served socket and its record. Dropping it (or `stop`) removes the record if it is still this engine's.
pub struct Served {
    registration: nu9ve_ipc::Registration,
    socket: PathBuf,
}

impl Served {
    pub fn stop(self) {
        let _ = self.registration.unregister();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Serves the socket and registers. Without a registry folder, or with another live `nuun` engine, nothing is
/// served: the app works the same, only the suite does not see it (agent §5).
#[cfg(unix)]
pub fn start(home: PathBuf, isolated: bool) -> Option<Served> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    let log = |m: &str| eprintln!("nuun-engine suite: {m}");
    let dir = registry(isolated)?;
    if let Ok(Some(nu9ve_ipc::Entry {
        state: nu9ve_ipc::State::Live(other),
        ..
    })) = nu9ve_ipc::read_entry(&dir, APP)
        && other.pid != std::process::id()
    {
        log(&format!(
            "another nüün engine is registered (pid {}); not served",
            other.pid
        ));
        return None;
    }
    let socket = match socket_path(&home) {
        Ok(path) => path,
        Err(e) => {
            log(&format!("no private socket folder: {e}"));
            return None;
        }
    };
    // A socket left by an engine that did not stop in order; anything else at that path is not ours to remove.
    if let Ok(meta) = std::fs::symlink_metadata(&socket)
        && meta.file_type().is_socket()
        && Some(meta.uid()) == nu9ve_ipc::current_uid()
    {
        let _ = std::fs::remove_file(&socket);
    }
    let listener = match UnixListener::bind(&socket) {
        Ok(listener) => listener,
        Err(e) => {
            log(&format!("cannot serve {}: {e}", socket.display()));
            return None;
        }
    };
    if std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).is_err() {
        let _ = std::fs::remove_file(&socket);
        return None;
    }
    let me = nu9ve_ipc::current_uid();
    let served = home.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            if me.is_none() || peer_uid(&stream) != me {
                continue;
            }
            let timeout = Some(std::time::Duration::from_secs(2));
            let _ = stream.set_read_timeout(timeout);
            let _ = stream.set_write_timeout(timeout);
            let Ok(reader) = stream.try_clone() else {
                continue;
            };
            let _ =
                nu9ve_engine::serve_frames(std::io::BufReader::new(reader), &stream, info(), |r| {
                    dispatch(&served, isolated, r)
                });
        }
    });
    let version = std::env::var("NUUN_APP_VERSION")
        .ok()
        .filter(|v| !v.is_empty() && v.len() <= 64)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    let record = nu9ve_ipc::Record::for_current_process(
        APP,
        &version,
        nu9ve_ipc::Transport::Unix {
            path: socket.clone(),
        },
        nu9ve_ipc::EngineRef {
            contract: nu9ve_engine::CONTRACT.into(),
            protocol: PROTOCOL.into(),
            engine: "rust".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        CAPABILITIES.iter().map(|c| (*c).to_owned()).collect(),
    );
    // The library it serves and, once nüün is in profiles.json, its default profile (registry §2, 2026-11-25).
    let record = record.map(|mut r| {
        r.library = home.to_str().map(str::to_owned);
        r.profile_id = nuun_media::suite_profiles::registered_default(&home, isolated);
        r
    });
    // Registered only once it accepts connections (registry §5).
    match record.and_then(|r| nu9ve_ipc::Registration::register(&dir, &r)) {
        Ok(registration) => Some(Served {
            registration,
            socket,
        }),
        Err(e) => {
            log(&format!("not registered: {e}"));
            let _ = std::fs::remove_file(&socket);
            None
        }
    }
}

/// Windows needs a named pipe with the rules of engine §9; not written yet, so nothing is registered.
#[cfg(not(unix))]
pub fn start(_: PathBuf, _: bool) -> Option<Served> {
    None
}

/// The live `suite` engine (the agent, agent §5) with the exchange methods, or why there is none.
fn agent(isolated: bool) -> std::result::Result<nu9ve_ipc::Record, &'static str> {
    let dir = registry(isolated).ok_or("absent")?;
    match nu9ve_ipc::read_entry(&dir, "suite") {
        Ok(Some(nu9ve_ipc::Entry {
            state: nu9ve_ipc::State::Live(record),
            ..
        })) if record.has_capability("exchange_v1") => Ok(record),
        Ok(Some(nu9ve_ipc::Entry {
            state: nu9ve_ipc::State::Live(_),
            ..
        })) => Err("unsupported"),
        _ => Err("absent"),
    }
}

fn agent_error(e: nu9ve_json::Error) -> nu9ve_json::Error {
    nu9ve_json::Error::new(
        if e.code.is_empty() {
            "agent_failed"
        } else {
            &e.code
        },
        format!("The suite agent: {}", e.message),
    )
}

/// «Trabajar aquí» (`take`), «Dejar de trabajar aquí» (`release`) or who works on it now (`state`) for one
/// nüün project, through the agent's `lease_take`, `lease_release` and `leases` (exchange §7.1). Without the
/// agent nothing changes: the answer says so and nüün works the same (agent §5).
pub fn lease(isolated: bool, params: &Value) -> nu9ve_json::Result<Value> {
    use nu9ve_json::{Error, require};
    use serde_json::json;
    let p = params.as_object().cloned().unwrap_or_default();
    require(
        p.keys().all(|k| k == "action" || k == "project_key"),
        "invalid_params",
        "Only action and project_key are accepted",
    )?;
    let action = p.get("action").and_then(Value::as_str).unwrap_or_default();
    require(
        ["take", "release", "state"].contains(&action),
        "invalid_params",
        "action is take, release or state",
    )?;
    // `state` without a key: every nüün project that some machine works on (one call for the projects list).
    let key = match p.get("project_key") {
        None if action == "state" => None,
        given => Some(
            given
                .and_then(Value::as_str)
                .filter(|k| {
                    (1..=64).contains(&k.len())
                        && k.bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                })
                .ok_or_else(|| {
                    Error::new("invalid_params", "project_key must match ^[a-z0-9-]{1,64}$")
                })?,
        ),
    };
    let record = match agent(isolated) {
        Ok(record) => record,
        Err(why) => return Ok(json!({"agent": why, "project_key": key, "holders": []})),
    };
    let timeout = nu9ve_ipc::DEFAULT_TIMEOUT;
    // The record says how to reach it; the engine says what it is (registry §6).
    let info =
        nu9ve_ipc::request(&record, "engine_info", None, None, timeout).map_err(agent_error)?;
    require(
        info["protocol"] == "nu9ve-suite/1",
        "agent_unsupported",
        "The registered suite engine speaks another protocol",
    )?;
    let args = json!({"app": APP, "project_key": key});
    let changed = match action {
        "take" => Some(
            nu9ve_ipc::request(&record, "lease_take", Some(args), None, timeout)
                .map_err(agent_error)?,
        ),
        "release" => Some(
            nu9ve_ipc::request(&record, "lease_release", Some(args), None, timeout)
                .map_err(agent_error)?,
        ),
        _ => None,
    };
    let leases = nu9ve_ipc::request(&record, "leases", Some(json!({})), None, timeout)
        .map_err(agent_error)?;
    // Other machines' names are their data, shown as such (`_untrusted`, agent §5).
    let holders: Vec<Value> = leases["leases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["app"] == APP && key.is_none_or(|k| l["project_key"] == k))
        .map(|l| {
            json!({"project_key": l["project_key"], "here": l["here"], "device_id": l["device_id"], "device_name": l["device_name"],
                "device_name_untrusted": l["device_name_untrusted"], "since": l["since"],
                "conflict": l["conflict"], "handing_over": l["handing_over"]})
        })
        .collect();
    let mut answer = json!({"agent": "running", "project_key": key,
        "here": holders.iter().any(|h| h["here"] == true), "holders": holders});
    if let Some(changed) = changed {
        answer["changed"] = changed;
    }
    Ok(answer)
}
