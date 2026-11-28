//! The surface without threads or sockets (nu9ve-surface-v1 §3 to §6): who owns each control, which
//! app has the page, what each light shows and which action a message is. Every input returns what to
//! do ([`Out`]); the server does it. Tables are rebuilt when an app, a map, a page or the mode changes,
//! never per message.

use nu9ve_midi::{
    ActionState, Color, DeviceMap, Gesture, Key, Light, LightBoard, MapFile, Mapping, Message,
    Motion, Profile, Reader, ZoneWhen,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const CONTRACT: &str = "nu9ve-surface/1";

/// A connection to an app.
pub type Conn = u64;

/// What the server has to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// A message to an app. With a key, a newer value with the same key replaces it if not yet sent.
    Send {
        conn: Conn,
        message: Value,
        coalesce: Option<String>,
    },
    /// Close an app's connection.
    Drop(Conn),
    Open(String),
    Close(String),
    Midi(String, Vec<Vec<u8>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Trigger,
    Continuous,
}

#[derive(Clone, Debug)]
struct Item {
    kind: Kind,
    color: Option<Color>,
}

type ActionKey = (String, Option<String>);

#[derive(Clone, Debug)]
struct App {
    name: String,
    profile_id: Option<String>,
    order: u64,
    focus: u64,
    catalog: BTreeMap<String, Item>,
    states: BTreeMap<ActionKey, (ActionState, Option<Value>)>,
    toggles: BTreeMap<ActionKey, bool>,
    /// The devices whose page it was told it has.
    pages: BTreeSet<String>,
}

struct Device {
    id: String,
    profile: Option<Profile>,
    reader: Reader,
    board: Option<LightBoard>,
    /// The app (by connection) pinned with the page control, if any.
    pinned: Option<Conn>,
    page: Option<Conn>,
}

/// `suite.json` of the controllers folder (§6): what belongs to no app.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SuiteFile {
    /// (substring of the port name, control id that cycles the page, or `None` for no page control).
    pub page_controls: Vec<(String, Option<String>)>,
}

impl SuiteFile {
    pub fn from_json(text: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(text).ok()?;
        let devices = v.get("devices")?.as_array()?;
        Some(Self {
            page_controls: devices
                .iter()
                .filter_map(|d| {
                    let control = match d.get("pageControl")? {
                        Value::Null => None,
                        Value::String(c) => Some(c.clone()),
                        _ => return None,
                    };
                    Some((d.get("match")?.as_str()?.to_owned(), control))
                })
                .collect(),
        })
    }
}

#[derive(Default)]
pub struct Surface {
    apps: BTreeMap<Conn, App>,
    /// Ports open now, by port name.
    devices: BTreeMap<String, Device>,
    /// Sources present on the system.
    present: BTreeSet<String>,
    /// Maps by (profile folder, app).
    maps: BTreeMap<(String, String), MapFile>,
    suite: SuiteFile,
    /// Apps whose recording signal is live in `recording`.
    recording: BTreeSet<String>,
    capturing: bool,
    profiles: Vec<Profile>,
    order: u64,
    focus: u64,
    serial: BTreeMap<String, u32>,
    last_devices: Value,
}

fn send(conn: Conn, message: Value) -> Out {
    Out::Send {
        conn,
        message,
        coalesce: None,
    }
}

fn light_from(value: &Value) -> Option<Light> {
    let o = value.as_object()?;
    let color = match o.get("color") {
        Some(Value::Null) => None,
        Some(Value::String(c)) => Some(Color::from_name(c)?),
        _ => return None,
    };
    let motion = match o.get("motion").and_then(Value::as_str) {
        None | Some("solid") => Motion::Solid,
        Some("flash") => Motion::Flash,
        Some("pulse") => Motion::Pulse,
        _ => return None,
    };
    Some(Light::new(
        color,
        o.get("dim").and_then(Value::as_bool).unwrap_or(false),
        motion,
    ))
}

fn state_from(text: &str) -> Option<ActionState> {
    serde_json::from_value(json!(text)).ok()
}

fn argument_of(v: &Value) -> Option<String> {
    v.get("argument").and_then(Value::as_str).map(str::to_owned)
}

fn catalog_of(v: &Value) -> BTreeMap<String, Item> {
    let mut out = BTreeMap::new();
    for item in v.as_array().into_iter().flatten() {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        let kind = match item.get("kind").and_then(Value::as_str) {
            Some("continuous") => Kind::Continuous,
            _ => Kind::Trigger,
        };
        let color = item
            .get("color")
            .and_then(Value::as_str)
            .and_then(Color::from_name);
        out.insert(id.to_owned(), Item { kind, color });
    }
    out
}

/// §4: the first message, checked. Returns the app, or why it is refused.
fn hello_app(v: &Value) -> Result<App, &'static str> {
    if v.get("type").and_then(Value::as_str) != Some("hello") {
        return Err("hello comes first");
    }
    if v.get("contract").and_then(Value::as_str) != Some(CONTRACT) {
        return Err("unsupported contract");
    }
    let name = v
        .get("app")
        .and_then(Value::as_str)
        .filter(|a| super::super::suite::valid_app(a))
        .ok_or("invalid app")?;
    if v.get("pid").and_then(Value::as_u64).is_none()
        || v.get("process_start_us").and_then(Value::as_u64).is_none()
    {
        return Err("invalid pid");
    }
    let profile_id = match v.get("profile_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(p)) if nu9ve_ipc::valid_profile_id(p) => Some(p.clone()),
        _ => return Err("invalid profile_id"),
    };
    Ok(App {
        name: name.to_owned(),
        profile_id,
        order: 0,
        focus: 0,
        catalog: catalog_of(v.get("catalog").unwrap_or(&Value::Null)),
        states: BTreeMap::new(),
        toggles: BTreeMap::new(),
        pages: BTreeSet::new(),
    })
}

impl Surface {
    pub fn new(profiles: Vec<Profile>) -> Self {
        Self {
            profiles,
            last_devices: json!([]),
            ..Self::default()
        }
    }

    fn map_of(&self, app: &App) -> Option<&MapFile> {
        let folder = app.profile_id.as_deref().unwrap_or("_");
        self.maps
            .get(&(folder.to_owned(), app.name.clone()))
            .filter(|m| m.enabled)
    }

    fn device_map(&self, app: &App, port: &str) -> Option<&DeviceMap> {
        self.map_of(app)?.device(port)
    }

    /// §8: MIDI is open only while a connected app has a map turned on.
    pub fn wants_midi(&self) -> bool {
        self.apps.values().any(|a| self.map_of(a).is_some())
    }

    pub fn needs_blink(&self) -> bool {
        self.devices
            .values()
            .any(|d| d.board.as_ref().is_some_and(LightBoard::needs_blink))
    }

    fn devices_json(&self) -> Value {
        json!(
            self.devices
                .iter()
                .map(|(port, d)| json!({"device": d.id, "profile": d.profile.as_ref().map(|p| p.id.clone()), "port": port}))
                .collect::<Vec<_>>()
        )
    }

    pub fn hello(&mut self, conn: Conn, v: &Value) -> Vec<Out> {
        let mut app = match hello_app(v) {
            Ok(app) => app,
            Err(_) => return vec![Out::Drop(conn)],
        };
        let mut out = Vec::new();
        // One connection per app: a new hello replaces the old one.
        let old: Vec<Conn> = self
            .apps
            .iter()
            .filter(|(_, a)| a.name == app.name)
            .map(|(c, _)| *c)
            .collect();
        for c in old {
            out.push(Out::Drop(c));
            self.apps.remove(&c);
        }
        self.order += 1;
        app.order = self.order;
        self.apps.insert(conn, app);
        self.apply_states(conn, v.get("states").unwrap_or(&Value::Null));
        out.push(send(
            conn,
            json!({"type":"welcome","contract":CONTRACT,"agent":super::super::install::VERSION,"devices":self.devices_json()}),
        ));
        out.extend(self.recompute());
        out
    }

    fn apply_states(&mut self, conn: Conn, states: &Value) {
        let Some(app) = self.apps.get_mut(&conn) else {
            return;
        };
        for s in states.as_array().into_iter().flatten() {
            let (Some(action), Some(state)) = (
                s.get("action").and_then(Value::as_str),
                s.get("state").and_then(Value::as_str).and_then(state_from),
            ) else {
                continue;
            };
            let light = s.get("light").filter(|l| light_from(l).is_some()).cloned();
            app.states
                .insert((action.to_owned(), argument_of(s)), (state, light));
        }
    }

    /// A message after `hello`. Unknown types are ignored (§4).
    pub fn message(&mut self, conn: Conn, v: &Value) -> Vec<Out> {
        if !self.apps.contains_key(&conn) {
            return vec![Out::Drop(conn)];
        }
        match v.get("type").and_then(Value::as_str) {
            Some("states") => {
                self.apply_states(conn, v.get("states").unwrap_or(&Value::Null));
                self.recompute()
            }
            Some("catalog") => {
                if let Some(app) = self.apps.get_mut(&conn) {
                    app.catalog = catalog_of(v.get("catalog").unwrap_or(&Value::Null));
                }
                self.recompute()
            }
            Some("focus") => {
                if v.get("active").and_then(Value::as_bool) == Some(true) {
                    self.focus += 1;
                    let focus = self.focus;
                    if let Some(app) = self.apps.get_mut(&conn) {
                        app.focus = focus;
                    }
                }
                self.recompute()
            }
            Some("bye") => {
                let mut out = self.disconnect(conn);
                out.push(Out::Drop(conn));
                out
            }
            _ => Vec::new(),
        }
    }

    pub fn disconnect(&mut self, conn: Conn) -> Vec<Out> {
        if self.apps.remove(&conn).is_none() {
            return Vec::new();
        }
        for d in self.devices.values_mut() {
            if d.pinned == Some(conn) {
                d.pinned = None;
            }
            if d.page == Some(conn) {
                d.page = None;
            }
        }
        self.recompute()
    }

    pub fn set_maps(
        &mut self,
        maps: BTreeMap<(String, String), MapFile>,
        suite: SuiteFile,
    ) -> Vec<Out> {
        if maps == self.maps && suite == self.suite {
            return Vec::new();
        }
        self.maps = maps;
        self.suite = suite;
        self.recompute()
    }

    pub fn set_recording(&mut self, recording: BTreeSet<String>, capturing: bool) -> Vec<Out> {
        if recording == self.recording && capturing == self.capturing {
            return Vec::new();
        }
        self.recording = recording;
        self.capturing = capturing;
        self.recompute()
    }

    pub fn port_connected(&mut self, name: &str) -> Vec<Out> {
        self.present.insert(name.to_owned());
        self.recompute()
    }

    pub fn port_gone(&mut self, name: &str) -> Vec<Out> {
        self.present.remove(name);
        self.devices.remove(name);
        let mut out = self.recompute();
        // It is gone: nothing to darken, only to tell.
        out.retain(|o| !matches!(o, Out::Midi(p, _) | Out::Close(p) if p == name));
        out
    }

    /// The MIDI system closed (no app with a map left): every device is forgotten.
    pub fn midi_closed(&mut self) {
        self.present.clear();
        self.devices.clear();
    }

    /// Apps that have a map for `port`, in connection order.
    fn apps_for(&self, port: &str) -> Vec<Conn> {
        let mut apps: Vec<(u64, Conn)> = self
            .apps
            .iter()
            .filter(|(_, a)| self.device_map(a, port).is_some())
            .map(|(c, a)| (a.order, *c))
            .collect();
        apps.sort();
        apps.into_iter().map(|(_, c)| c).collect()
    }

    /// §5: the pinned app, else the app most recently in front, else the first connected.
    fn page_for(&self, port: &str, pinned: Option<Conn>) -> Option<Conn> {
        let apps = self.apps_for(port);
        if let Some(p) = pinned.filter(|p| apps.contains(p)) {
            return Some(p);
        }
        apps.iter()
            .copied()
            .max_by_key(|c| (self.apps[c].focus, std::cmp::Reverse(self.apps[c].order)))
    }

    /// §5: `suite.json` first (a control, or `null` for none), else the profile's `pageControl`.
    fn page_control(&self, port: &str) -> Option<&str> {
        let lower = port.to_lowercase();
        if let Some((_, control)) = self
            .suite
            .page_controls
            .iter()
            .find(|(m, _)| lower.contains(&m.to_lowercase()))
        {
            return control.as_deref();
        }
        self.devices
            .get(port)?
            .profile
            .as_ref()?
            .page_control
            .as_deref()
    }

    /// Who owns a control now, and how: an active zone first (the recording app wins, then the first
    /// connected), else the page.
    fn owner(&self, port: &str, control: Option<&str>) -> Option<(Conn, &'static str)> {
        let device = self.devices.get(port)?;
        if let Some(control) = control {
            let mut zones: Vec<(bool, u64, Conn)> = Vec::new();
            for (conn, app) in &self.apps {
                let Some(map) = self.device_map(app, port) else {
                    continue;
                };
                let recording = self.recording.contains(&app.name);
                let active = map.zones.iter().any(|z| {
                    (z.when == ZoneWhen::Always || (z.when == ZoneWhen::Recording && recording))
                        && z.controls.iter().any(|c| c == control)
                });
                if active {
                    zones.push((!recording, app.order, *conn));
                }
            }
            if let Some((_, _, conn)) = zones.into_iter().min() {
                return Some((conn, "zone"));
            }
        }
        device.page.map(|c| (c, "page"))
    }

    fn mapping_for(
        &self,
        conn: Conn,
        port: &str,
        control: Option<&str>,
        key: Key,
    ) -> Option<Mapping> {
        let map = self.device_map(self.apps.get(&conn)?, port)?;
        let mappings = map.effective(self.capturing);
        map.mapping(&mappings, control, key).cloned()
    }

    fn light_for(&self, port: &str, control: &nu9ve_midi::Control) -> Light {
        if self.page_control(port) == Some(control.id.as_str()) {
            let pinned = self.devices.get(port).is_some_and(|d| d.pinned.is_some());
            return Light::new(Some(Color::White), !pinned, Motion::Solid);
        }
        let Some((conn, _)) = self.owner(port, Some(&control.id)) else {
            return Light::OFF;
        };
        let Some(mapping) = self.mapping_for(conn, port, Some(&control.id), control.key()) else {
            return Light::OFF;
        };
        let app = &self.apps[&conn];
        let Some(item) = app.catalog.get(&mapping.action) else {
            // Unknown action: kept in the map, shown unavailable (map §3), dark here.
            return Light::OFF;
        };
        let key = (mapping.action.clone(), mapping.argument.clone());
        let (state, asked) = app
            .states
            .get(&key)
            .cloned()
            .unwrap_or((ActionState::Available, None));
        let color = mapping.color.or(item.color).unwrap_or(Color::White);
        // The app may ask for its own light; a mapping's color still wins (map §2).
        match asked.as_ref().and_then(light_from) {
            Some(light) if light.is_off() => Light::OFF,
            Some(light) => Light::new(mapping.color.or(light.color), light.dim, light.motion),
            None => Light::for_state(state, color),
        }
    }

    /// Rebuilds devices, pages and lights from the apps, maps, ports and mode.
    fn recompute(&mut self) -> Vec<Out> {
        let mut out = Vec::new();
        // Devices: the present ports some connected app maps.
        let wanted: Vec<String> = self
            .present
            .iter()
            .filter(|p| self.apps.values().any(|a| self.device_map(a, p).is_some()))
            .cloned()
            .collect();
        let gone: Vec<String> = self
            .devices
            .keys()
            .filter(|p| !wanted.contains(p))
            .cloned()
            .collect();
        for port in gone {
            if let Some(mut d) = self.devices.remove(&port) {
                if let Some(board) = &mut d.board {
                    let off = board.darken();
                    if !off.is_empty() {
                        out.push(Out::Midi(port.clone(), off));
                    }
                }
                out.push(Out::Close(port.clone()));
            }
            for app in self.apps.values_mut() {
                app.pages.remove(&port);
            }
        }
        for port in &wanted {
            if self.devices.contains_key(port) {
                continue;
            }
            let profile_id = self
                .apps_for(port)
                .first()
                .and_then(|c| self.device_map(&self.apps[c], port))
                .and_then(|m| m.profile.clone());
            let profile =
                profile_id.and_then(|id| self.profiles.iter().find(|p| p.id == id).cloned());
            let base = profile
                .as_ref()
                .map_or("midi", |p| p.id.as_str())
                .to_owned();
            let n = self.serial.entry(base.clone()).or_insert(0);
            *n += 1;
            let id = format!("{base}-{n}");
            let board = profile
                .clone()
                .filter(|p| p.feedback.is_some())
                .map(LightBoard::new);
            out.push(Out::Open(port.clone()));
            self.devices.insert(
                port.clone(),
                Device {
                    id,
                    reader: Reader::new(profile.clone()),
                    profile,
                    board,
                    pinned: None,
                    page: None,
                },
            );
            // Connect (map §1): init and every light off; the lights come below.
            if let Some(board) = self.devices.get_mut(port).and_then(|d| d.board.as_mut()) {
                let fresh = board.forget();
                if !fresh.is_empty() {
                    out.push(Out::Midi(port.clone(), fresh));
                }
            }
        }
        // Pages.
        let ports: Vec<String> = self.devices.keys().cloned().collect();
        for port in &ports {
            let pinned = self.devices[port].pinned;
            let page = self.page_for(port, pinned);
            let device = self.devices.get_mut(port).unwrap();
            if device.pinned.is_some() && device.pinned != page {
                device.pinned = None;
            }
            device.page = page;
            let id = device.id.clone();
            for (conn, app) in self.apps.iter_mut() {
                let has = page == Some(*conn);
                if has != app.pages.contains(port) {
                    if has {
                        app.pages.insert(port.clone());
                    } else {
                        app.pages.remove(port);
                    }
                    out.push(send(*conn, json!({"type":"page","page":has,"device":id})));
                }
            }
        }
        // Lights: only what changed.
        for port in &ports {
            let Some(profile) = self.devices[port]
                .board
                .as_ref()
                .map(|b| b.profile().clone())
            else {
                continue;
            };
            let lights: BTreeMap<String, Light> = profile
                .controls
                .iter()
                .filter(|c| profile.is_lit(c))
                .map(|c| (c.id.clone(), self.light_for(port, c)))
                .filter(|(_, l)| !l.is_off())
                .collect();
            let board = self.devices.get_mut(port).unwrap().board.as_mut().unwrap();
            let changed = board.update(&lights);
            if !changed.is_empty() {
                out.push(Out::Midi(port.clone(), changed));
            }
        }
        // Devices, to every app, when they change.
        let devices = self.devices_json();
        if devices != self.last_devices {
            self.last_devices = devices.clone();
            for conn in self.apps.keys() {
                out.push(send(*conn, json!({"type":"devices","devices":devices})));
            }
        }
        out
    }

    /// A channel message from an open port: the actions it makes (§4).
    pub fn midi(&mut self, port: &str, bytes: [u8; 3], host_ns: u64) -> Vec<Out> {
        let Some(message) = Message::parse(&bytes) else {
            return Vec::new();
        };
        let Some(device) = self.devices.get_mut(port) else {
            return Vec::new();
        };
        let readings = device.reader.read(message, host_ns / 1_000_000);
        let id = device.id.clone();
        let mut out = Vec::new();
        for reading in readings {
            let control = reading.control.as_deref();
            if control.is_some() && control == self.page_control(port) {
                if reading.gesture == Gesture::Press {
                    out.extend(self.cycle_page(port));
                }
                continue;
            }
            let Some((conn, via)) = self.owner(port, control) else {
                continue;
            };
            let Some(mapping) = self.mapping_for(conn, port, control, reading.key) else {
                continue;
            };
            let app = self.apps.get_mut(&conn).unwrap();
            let Some(item) = app.catalog.get(&mapping.action).cloned() else {
                continue;
            };
            let mut message = Map::new();
            message.insert("type".into(), json!("action"));
            message.insert("action".into(), json!(mapping.action));
            if let Some(argument) = &mapping.argument {
                message.insert("argument".into(), json!(argument));
            }
            let mut coalesce = None;
            match (item.kind, reading.gesture) {
                (Kind::Trigger, Gesture::Press) => {}
                (Kind::Continuous, Gesture::Press) => {
                    let key = (mapping.action.clone(), mapping.argument.clone());
                    let on = !app.toggles.get(&key).copied().unwrap_or(false);
                    app.toggles.insert(key, on);
                    message.insert("value".into(), json!(if on { 1.0 } else { 0.0 }));
                }
                (Kind::Continuous, Gesture::Value(x)) => {
                    message.insert("value".into(), json!(x));
                    coalesce = Some(format!("{}\u{0}{}", mapping.action, control.unwrap_or("")));
                }
                (Kind::Continuous, Gesture::Step(d)) => {
                    message.insert("delta".into(), json!(d));
                }
                _ => continue,
            }
            message.insert("device".into(), json!(id));
            message.insert(
                "control".into(),
                control.map_or_else(|| json!(reading.key.label()), |c| json!(c)),
            );
            message.insert("via".into(), json!(via));
            message.insert("host_ns".into(), json!(host_ns));
            out.push(Out::Send {
                conn,
                message: Value::Object(message),
                coalesce,
            });
        }
        out
    }

    /// The page control (§5): each press pins the next app with a map; past the last one, the page
    /// follows the front again.
    fn cycle_page(&mut self, port: &str) -> Vec<Out> {
        let apps = self.apps_for(port);
        let Some(device) = self.devices.get(port) else {
            return Vec::new();
        };
        let after = |c: Option<Conn>| {
            c.and_then(|c| apps.iter().position(|a| *a == c))
                .map_or(0, |i| i + 1)
        };
        let next = match device.pinned {
            // From the front: the app after the one with the page, or the first.
            None => apps.get(after(device.page)).or(apps.first()).copied(),
            Some(p) => apps.get(after(Some(p))).copied(),
        };
        self.devices.get_mut(port).unwrap().pinned = next;
        self.recompute()
    }

    /// The software blink of every device that needs it.
    pub fn blink(&mut self, phase: bool) -> Vec<Out> {
        self.devices
            .iter()
            .filter_map(|(port, d)| {
                let b = d.board.as_ref()?.blink(phase);
                (!b.is_empty()).then(|| Out::Midi(port.clone(), b))
            })
            .collect()
    }

    /// §7: before stopping or updating, every light off and `released` to every app.
    pub fn release(&mut self) -> Vec<Out> {
        let mut out = Vec::new();
        for (port, d) in &mut self.devices {
            if let Some(board) = &mut d.board {
                let off = board.darken();
                if !off.is_empty() {
                    out.push(Out::Midi(port.clone(), off));
                }
            }
            out.push(Out::Close(port.clone()));
        }
        self.devices.clear();
        for conn in self.apps.keys() {
            out.push(send(*conn, json!({"type":"released"})));
        }
        out
    }
}
