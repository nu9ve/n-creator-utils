//! nu9ve-surface-v1 end to end with fake clients: apps talk to the agent's surface socket with frames, and a
//! fake MIDI system stands for the controller (what it opens, what lights it is sent, and the presses the
//! test plays). Nothing touches CoreMIDI or the person's folders.
#![cfg(target_os = "macos")]

use nu9ve_mcp::agent::surface::{Config, Server};
use nu9ve_midi::{MidiSystem, PortEvent, Sink};
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PORT: &str = "Launchpad Pro Standalone Port";

fn eventually(mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// What the fake controller saw, and the sink to play presses with.
#[derive(Default)]
struct Wire {
    sink: Option<Sink>,
    open: Vec<String>,
    sent: Vec<String>,
    systems: usize,
}

#[derive(Clone, Default)]
struct FakeMidi(Arc<Mutex<Wire>>);

struct FakeSystem(Arc<Mutex<Wire>>);

impl MidiSystem for FakeSystem {
    fn sources(&self) -> Vec<String> {
        vec![PORT.into()]
    }
    fn open(&mut self, name: &str) -> io::Result<()> {
        self.0.lock().unwrap().open.push(name.into());
        Ok(())
    }
    fn close(&mut self, name: &str) {
        self.0.lock().unwrap().open.retain(|n| n != name);
    }
    fn send(&mut self, _: &str, messages: &[Vec<u8>]) -> io::Result<()> {
        let mut wire = self.0.lock().unwrap();
        for m in messages {
            let hex: Vec<String> = m.iter().map(|b| format!("{b:02X}")).collect();
            wire.sent.push(hex.join(" "));
        }
        Ok(())
    }
}

impl FakeMidi {
    fn factory(&self) -> nu9ve_mcp::agent::surface::MidiFactory {
        let wire = self.0.clone();
        Box::new(move |sink: Sink| {
            let mut w = wire.lock().unwrap();
            w.systems += 1;
            w.sink = Some(sink.clone());
            drop(w);
            sink(PortEvent::Connected { name: PORT.into() });
            Ok(Box::new(FakeSystem(wire.clone())) as Box<dyn MidiSystem>)
        })
    }
    fn play(&self, bytes: [u8; 3], host_ns: u64) {
        let sink = self.0.lock().unwrap().sink.clone().expect("MIDI is open");
        sink(PortEvent::Message {
            name: PORT.into(),
            bytes,
            host_ns,
        });
    }
    fn sent(&self) -> Vec<String> {
        self.0.lock().unwrap().sent.clone()
    }
    fn take_sent(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap().sent)
    }
    fn is_open(&self) -> bool {
        self.0.lock().unwrap().open.iter().any(|n| n == PORT)
    }
    fn systems(&self) -> usize {
        self.0.lock().unwrap().systems
    }
}

struct App {
    stream: UnixStream,
}

impl App {
    fn connect(socket: &Path) -> Self {
        let stream = UnixStream::connect(socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Self { stream }
    }
    fn send(&mut self, message: Value) {
        let bytes = serde_json::to_vec(&message).unwrap();
        self.stream
            .write_all(&(bytes.len() as u32).to_be_bytes())
            .unwrap();
        self.stream.write_all(&bytes).unwrap();
    }
    fn read(&mut self) -> Option<Value> {
        let mut header = [0; 4];
        self.stream.read_exact(&mut header).ok()?;
        let mut payload = vec![0; u32::from_be_bytes(header) as usize];
        self.stream.read_exact(&mut payload).ok()?;
        serde_json::from_slice(&payload).ok()
    }
    /// The next message of a type, skipping the others.
    fn next(&mut self, kind: &str) -> Value {
        loop {
            let m = self.read().unwrap_or_else(|| panic!("no {kind} message"));
            if m["type"] == kind {
                return m;
            }
        }
    }
    fn hello(&mut self, app: &str, catalog: Value, states: Value) -> Value {
        self.send(
            json!({"type":"hello","contract":"nu9ve-surface/1","app":app,"pid":std::process::id(),
            "process_start_us":1,"profile_id":null,"catalog":catalog,"states":states}),
        );
        self.next("welcome")
    }
}

/// The server is dropped before the folder.
struct Setup {
    server: Option<Server>,
    root: PathBuf,
    midi: FakeMidi,
    _dir: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("controllers/_")).unwrap();
        std::fs::create_dir_all(root.join("recording")).unwrap();
        Self {
            _dir: dir,
            root,
            midi: FakeMidi::default(),
            server: None,
        }
    }
    fn map(&self, app: &str, map: Value) {
        let path = self.root.join(format!("controllers/_/{app}.json"));
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(&map).unwrap()).unwrap();
        std::fs::rename(tmp, path).unwrap();
    }
    fn start(&mut self) -> PathBuf {
        let socket = self.root.join("agent/runtime/surface");
        let server = Server::start(Config {
            socket: socket.clone(),
            controllers: self.root.join("controllers"),
            recording: Some(self.root.join("recording")),
            midi: self.midi.factory(),
        })
        .unwrap();
        self.server = Some(server);
        socket
    }
    /// A live file of nu9ve-recording-v1 for `app`, as this process.
    fn recording(&self, app: &str, state: &str) {
        let me = nu9ve_ipc::process(std::process::id()).unwrap();
        let session = if state == "idle" {
            Value::Null
        } else {
            json!("ded6fcd1-7019-42e1-835e-c25896147a7f")
        };
        let epoch = if state == "recording" {
            json!({"clock":"coremedia-host-clock","value":1,"timescale":1_000_000_000})
        } else {
            Value::Null
        };
        let file = json!({"contract":"nu9ve-recording/1","app":app,"app_version":"1.0","pid":std::process::id(),
            "process_start_us":me.start_us,"seq":1,"state":state,"session_id":session,"epoch":epoch,
            "epoch_wall_clock": if state == "recording" { json!("2026-11-27T18:00:00.123Z") } else { Value::Null },
            "captures": if state == "idle" { json!([]) } else { json!(["screen"]) },
            "ended_at":null,"written_at":"2026-11-27T18:00:00.125Z"});
        let path = self.root.join(format!("recording/{app}.json"));
        let tmp = self.root.join(format!("recording/.{app}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec(&file).unwrap()).unwrap();
        std::fs::rename(tmp, path).unwrap();
    }
}

fn launchpad(mappings: Value) -> Value {
    json!({"schemaVersion":1,"enabled":true,"devices":[{"match":"Launchpad Pro","profile":"novation-launchpad-pro","mappings":mappings}]})
}

fn catalog() -> Value {
    json!([
        {"id":"recording.toggle","title":"Grabar","group":"Grabación","kind":"trigger"},
        {"id":"preset.select","title":"Preset","group":"Presets","kind":"trigger","arguments":[{"value":"a","title":"A"}]},
        {"id":"scene.crossfade","title":"Mezcla","group":"Escena","kind":"continuous","color":"purple"}
    ])
}

#[test]
fn an_app_gets_its_actions_and_its_lights() {
    let mut s = Setup::new();
    s.map(
        "recorder",
        launchpad(json!([
            {"control":"pad-1-1","action":"recording.toggle","color":"red"},
            {"control":"top-1","action":"preset.select","argument":"a","color":"cyan"},
            {"control":"pad-2-1","action":"scene.crossfade"},
            {"control":"pad-3-1","action":"unknown.action"}
        ])),
    );
    let socket = s.start();
    assert_eq!(
        s.midi.systems(),
        0,
        "nothing opens MIDI before an app with a map connects"
    );
    let mut app = App::connect(&socket);
    let welcome = app.hello(
        "recorder",
        catalog(),
        json!([{"action":"recording.toggle","state":"available"},
               {"action":"preset.select","argument":"a","state":"active"}]),
    );
    assert_eq!(welcome["contract"], "nu9ve-surface/1");
    let (mut page, mut devices) = (Value::Null, Value::Null);
    while page.is_null() || devices.is_null() {
        let m = app.read().expect("page and devices");
        match m["type"].as_str() {
            Some("page") => page = m,
            Some("devices") => devices = m,
            _ => {}
        }
    }
    assert_eq!(page["page"], true);
    assert_eq!(devices["devices"][0]["profile"], "novation-launchpad-pro");
    assert_eq!(devices["devices"][0]["port"], PORT);
    let device = devices["devices"][0]["device"].as_str().unwrap().to_owned();

    // Connected: init, every light off, then the states: red dim (available), cyan bright (active), the
    // catalog's purple, dim, for an action with no state yet, and Session (the page control) dim.
    assert!(eventually(
        || s.midi.is_open() && s.midi.sent().len() >= 1 + 96 + 4
    ));
    let sent = s.midi.take_sent();
    assert_eq!(sent[0], "F0 00 20 29 02 10 2C 03 F7");
    assert_eq!(
        sent[97..],
        ["B0 5B 25", "B0 5F 01", "90 51 07", "90 47 33"],
        "{sent:?}"
    );
    assert!(
        !sent[97..].iter().any(|m| m.starts_with("90 3D")),
        "an unknown action stays dark"
    );

    // A press of any velocity is the action, once, with the host clock of the message.
    s.midi.play([0x90, 81, 1], 123_000_000);
    s.midi.play([0x90, 81, 0], 124_000_000);
    let action = app.next("action");
    assert_eq!(
        (
            &action["action"],
            &action["via"],
            &action["control"],
            &action["host_ns"]
        ),
        (
            &json!("recording.toggle"),
            &json!("page"),
            &json!("pad-1-1"),
            &json!(123_000_000)
        )
    );
    assert_eq!(action["device"], device);
    s.midi.play([0xb0, 91, 127], 125_000_000);
    let action = app.next("action");
    assert_eq!(
        (&action["action"], &action["argument"]),
        (&json!("preset.select"), &json!("a"))
    );

    // A button on a continuous action toggles it between 1 and 0.
    s.midi.play([0x90, 71, 127], 126_000_000);
    assert_eq!(app.next("action")["value"], 1.0);
    s.midi.play([0x90, 71, 0], 127_000_000);
    s.midi.play([0x90, 71, 127], 128_000_000);
    assert_eq!(app.next("action")["value"], 0.0);

    // A state with its own light: only what changed is sent.
    app.send(json!({"type":"states","states":[{"action":"recording.toggle","state":"active","light":{"color":"red","motion":"pulse"}}]}));
    assert!(eventually(|| s.midi.sent().len() >= 2));
    assert_eq!(
        s.midi.take_sent(),
        ["90 51 05", "F0 00 20 29 02 10 28 51 05 F7"]
    );

    // The map changes on disk: the agent reads it again by itself.
    s.map(
        "recorder",
        launchpad(json!([{"control":"pad-1-2","action":"recording.toggle","color":"green"}])),
    );
    assert!(
        eventually(|| s.midi.sent().len() >= 3),
        "{:?}",
        s.midi.sent()
    );
    let sent = s.midi.take_sent();
    assert!(sent.contains(&"90 51 00".to_owned()) && sent.contains(&"B0 5B 00".to_owned()));
    assert!(sent.iter().any(|m| m.starts_with("90 52 15")), "{sent:?}");

    // Bye: the lights go off and, with no app left, MIDI closes.
    app.send(json!({"type":"bye"}));
    assert!(eventually(|| !s.midi.is_open()));
    assert!(s.midi.sent().contains(&"90 52 00".to_owned()));
}

#[test]
fn the_page_follows_focus_and_a_recording_zone_stays() {
    let mut s = Setup::new();
    let mut recorder_map = launchpad(json!([
        {"control":"pad-1-1","action":"preset.select","argument":"a"},
        {"control":"left-8","action":"recording.toggle","color":"red"}
    ]));
    recorder_map["devices"][0]["zones"] = json!([{"when":"recording","controls":["left-8"]}]);
    s.map("recorder", recorder_map);
    s.map(
        "player",
        launchpad(json!([{"control":"pad-1-1","action":"scene.crossfade"}])),
    );
    let socket = s.start();
    let mut recorder = App::connect(&socket);
    recorder.hello("recorder", catalog(), json!([]));
    assert_eq!(recorder.next("page")["page"], true);
    let mut player = App::connect(&socket);
    player.hello("player", catalog(), json!([]));

    // The player comes to the front: the page is its.
    player.send(json!({"type":"focus","active":true}));
    assert_eq!(player.next("page")["page"], true);
    assert_eq!(recorder.next("page")["page"], false);
    s.midi.play([0x90, 81, 127], 1_000_000);
    let action = player.next("action");
    assert_eq!(
        (&action["action"], &action["via"]),
        (&json!("scene.crossfade"), &json!("page"))
    );

    // Record is the recorder's only while it records.
    s.midi.play([0xb0, 10, 127], 2_000_000);
    s.midi.play([0xb0, 10, 0], 2_100_000);
    s.recording("recorder", "recording");
    // The agent sees the signal by itself and lights the zone (red, available: dim).
    assert!(
        eventually(|| s.midi.sent().iter().any(|m| m == "B0 0A 07")),
        "{:?}",
        s.midi.sent()
    );
    s.midi.play([0xb0, 10, 127], 3_000_000);
    let action = recorder.next("action");
    assert_eq!(
        (&action["action"], &action["via"], &action["host_ns"]),
        (
            &json!("recording.toggle"),
            &json!("zone"),
            &json!(3_000_000)
        )
    );

    // The recorder comes to the front.
    recorder.send(json!({"type":"focus","active":true}));
    assert_eq!(recorder.next("page")["page"], true);
    assert_eq!(player.next("page")["page"], false);

    // The agent stops: lights off, `released` to every app.
    s.server.take().unwrap().stop();
    assert_eq!(recorder.next("released")["type"], "released");
    assert_eq!(player.next("released")["type"], "released");
    assert!(!s.midi.is_open());
    assert!(s.midi.sent().iter().any(|m| m == "B0 0A 00"));
}

#[test]
fn a_bad_hello_is_closed_and_an_app_without_a_map_opens_nothing() {
    let mut s = Setup::new();
    let socket = s.start();
    let mut app = App::connect(&socket);
    app.send(json!({"type":"hello","contract":"nu9ve-surface/2","app":"recorder","pid":1,"process_start_us":1,"profile_id":null,"catalog":[],"states":[]}));
    assert!(app.read().is_none(), "another major version is closed");
    let mut app = App::connect(&socket);
    app.send(json!({"type":"states","states":[]}));
    assert!(app.read().is_none(), "hello comes first");
    let mut app = App::connect(&socket);
    let welcome = app.hello("plain", catalog(), json!([]));
    assert_eq!(welcome["devices"], json!([]));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(s.midi.systems(), 0, "no map: MIDI stays closed");
}

#[test]
fn the_page_control_pins_each_app_then_follows_the_front_again() {
    let mut s = Setup::new();
    std::fs::write(
        s.root.join("controllers/suite.json"),
        r#"{"schemaVersion":1,"devices":[{"match":"Launchpad Pro","pageControl":"bottom-8"}]}"#,
    )
    .unwrap();
    s.map(
        "recorder",
        launchpad(json!([{"control":"pad-1-1","action":"recording.toggle"}])),
    );
    s.map(
        "player",
        launchpad(json!([{"control":"pad-1-1","action":"recording.toggle"}])),
    );
    let socket = s.start();
    let mut recorder = App::connect(&socket);
    recorder.hello("recorder", catalog(), json!([]));
    assert_eq!(recorder.next("page")["page"], true);
    let mut player = App::connect(&socket);
    player.hello("player", catalog(), json!([]));
    // The page control is lit, dim while the page follows the front (bottom-8 is CC 8).
    assert!(
        eventually(|| s.midi.sent().iter().any(|m| m == "B0 08 01")),
        "{:?}",
        s.midi.sent()
    );

    let press = |m: &FakeMidi, t: u64| {
        m.play([0xb0, 8, 127], t);
        m.play([0xb0, 8, 0], t + 1);
    };
    press(&s.midi, 1_000_000);
    assert_eq!(player.next("page")["page"], true, "pinned to the next app");
    assert_eq!(recorder.next("page")["page"], false);
    assert!(
        eventually(|| s.midi.sent().iter().any(|m| m == "B0 08 03")),
        "pinned: bright"
    );
    // Coming to the front does not take a pinned page.
    recorder.send(json!({"type":"focus","active":true}));
    s.midi.play([0x90, 81, 127], 2_000_000);
    assert_eq!(player.next("action")["action"], "recording.toggle");
    // Past the last app the page follows the front again: the recorder.
    press(&s.midi, 3_000_000);
    assert_eq!(recorder.next("page")["page"], true);
    assert_eq!(player.next("page")["page"], false);
}

#[test]
fn session_changes_page_by_default_and_suite_json_can_turn_it_off() {
    let mut s = Setup::new();
    s.map(
        "recorder",
        launchpad(json!([{"control":"pad-1-1","action":"recording.toggle"}])),
    );
    s.map(
        "player",
        launchpad(json!([{"control":"pad-1-1","action":"recording.toggle"},
                                     {"control":"top-5","action":"scene.crossfade"}])),
    );
    let socket = s.start();
    let mut recorder = App::connect(&socket);
    recorder.hello("recorder", catalog(), json!([]));
    assert_eq!(recorder.next("page")["page"], true);
    let mut player = App::connect(&socket);
    player.hello("player", catalog(), json!([]));

    // Session (top-5, CC 0x5F) is the profile's page control: reserved, even if a map uses it.
    s.midi.play([0xb0, 0x5f, 127], 1_000_000);
    s.midi.play([0xb0, 0x5f, 0], 1_100_000);
    assert_eq!(player.next("page")["page"], true);
    assert_eq!(recorder.next("page")["page"], false);

    // suite.json says no page control for this model: Session is the player's again.
    std::fs::write(
        s.root.join("controllers/suite.json"),
        r#"{"schemaVersion":1,"devices":[{"match":"Launchpad Pro","pageControl":null}]}"#,
    )
    .unwrap();
    assert!(
        eventually(|| s.midi.sent().iter().any(|m| m == "B0 5F 33")),
        "the player's purple, dim"
    );
    s.midi.play([0xb0, 0x5f, 127], 2_000_000);
    assert_eq!(player.next("action")["action"], "scene.crossfade");
}
