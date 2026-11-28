//! nu9ve-surface-v1: the agent as the only owner of the controller. Apps connect to
//! `<agent folder>/runtime/surface` (frames of 4 bytes big-endian plus JSON, at most 64 KiB), declare
//! their catalog and states, and receive the actions of the controls they own. The agent reads every
//! app's map from the controllers folder and the recording signal, both by kqueue: nothing polls.
//! MIDI opens only while a connected app has a map turned on (§8). macOS only, like CoreMIDI.

mod route;

pub use route::{CONTRACT, Conn, Out, SuiteFile, Surface};

use nu9ve_midi::{MapFile, MidiSystem, PortEvent, Sink};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// §2: frames are at most 64 KiB.
pub const MAX_FRAME: usize = 64 * 1024;
/// The software blink: half a period.
const BLINK: Duration = Duration::from_millis(300);
/// Several file events of one write become one reload.
const SETTLE: Duration = Duration::from_millis(30);
/// A map or a recording file is never this big.
const MAX_FILE: u64 = 1024 * 1024;

/// Opens the MIDI system. Called when an app with a map connects; dropped when none is left.
pub type MidiFactory = Box<dyn FnMut(Sink) -> io::Result<Box<dyn MidiSystem>> + Send>;

pub struct Config {
    /// `<agent folder>/runtime/surface`.
    pub socket: PathBuf,
    /// §6: `controllers/<profile>/<app>.json` and `controllers/suite.json`.
    pub controllers: PathBuf,
    /// nu9ve-recording-v1 §1, or `None` to never read it.
    pub recording: Option<PathBuf>,
    pub midi: MidiFactory,
}

/// §2 and §6: the socket in the agent folder, and the controllers and recording folders as its siblings
/// (`…/nu9ve/agent`, `…/nu9ve/controllers`, `…/nu9ve/recording`), so a QA agent folder brings its own.
/// `NU9VE_CONTROLLERS_DIR` and `NU9VE_RECORDING_DIR` (absolute) replace them.
pub fn paths(agent_dir: &Path) -> (PathBuf, PathBuf, Option<PathBuf>) {
    let base = agent_dir.parent().unwrap_or(agent_dir);
    let over = |var: &str| {
        std::env::var(var)
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let recording = match std::env::var_os("NU9VE_RECORDING_DIR") {
        Some(_) => over("NU9VE_RECORDING_DIR"),
        None => Some(base.join("recording")),
    };
    (
        agent_dir.join("runtime/surface"),
        over("NU9VE_CONTROLLERS_DIR").unwrap_or_else(|| base.join("controllers")),
        recording,
    )
}

fn read_small(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// Every app's map, by (profile folder, app). Invalid files are skipped: an app's configurator says why.
pub fn read_maps(dir: &Path) -> (BTreeMap<(String, String), MapFile>, SuiteFile) {
    let mut maps = BTreeMap::new();
    for folder in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = folder.file_name().to_string_lossy().into_owned();
        if !folder.file_type().is_ok_and(|t| t.is_dir())
            || !(name == "_" || nu9ve_ipc::valid_profile_id(&name))
        {
            continue;
        }
        for file in fs::read_dir(folder.path()).into_iter().flatten().flatten() {
            let path = file.path();
            let Some(app) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".json"))
                .filter(|a| super::suite::valid_app(a))
            else {
                continue;
            };
            if let Some(map) = read_small(&path).and_then(|t| MapFile::from_json(&t).ok()) {
                maps.insert((name.clone(), app.to_owned()), map);
            }
        }
    }
    let suite = read_small(&dir.join("suite.json"))
        .and_then(|t| SuiteFile::from_json(&t))
        .unwrap_or_default();
    (maps, suite)
}

/// nu9ve-recording-v1 §4, with `nu9ve-recording`: the apps with a live file in `recording`, and whether
/// any is capturing.
pub fn read_recording(dir: Option<&Path>) -> (BTreeSet<String>, bool) {
    let Some(dir) = dir else {
        return (BTreeSet::new(), false);
    };
    let derived = nu9ve_recording::read_dir(dir);
    let apps = derived
        .recording_apps()
        .into_iter()
        .map(str::to_owned)
        .collect();
    (apps, derived.capturing)
}

enum Event {
    Accepted(Conn, Arc<Outbox>),
    Frame(Conn, Value),
    Closed(Conn),
    Port(PortEvent),
    Files,
    Blink,
    Stop,
}

/// One app's messages, written by its own thread. A continuous value not yet written is replaced by a
/// newer one of the same control (§4); a trigger is never dropped or merged.
/// Frames waiting, each with its coalescing key, and whether the outbox is finished.
type Queue = (VecDeque<(Option<String>, Vec<u8>)>, bool);

struct Outbox {
    stream: UnixStream,
    queue: Mutex<Queue>,
    ready: Condvar,
}

impl Outbox {
    fn push(&self, message: &Value, coalesce: Option<String>) {
        let Ok(bytes) = serde_json::to_vec(message) else {
            return;
        };
        if bytes.len() > MAX_FRAME {
            return;
        }
        let Ok(mut q) = self.queue.lock() else { return };
        if q.1 {
            return;
        }
        if let Some(key) = &coalesce
            && let Some(slot) = q.0.iter_mut().find(|(k, _)| k.as_ref() == Some(key))
        {
            slot.1 = bytes;
        } else {
            q.0.push_back((coalesce, bytes));
        }
        self.ready.notify_one();
    }

    /// No more messages: the writer sends what is queued and closes the connection.
    fn finish(&self) {
        if let Ok(mut q) = self.queue.lock() {
            q.1 = true;
        }
        self.ready.notify_one();
    }

    fn write_all(&self) {
        let mut stream = &self.stream;
        loop {
            let next = {
                let Ok(mut q) = self.queue.lock() else { break };
                while q.0.is_empty() && !q.1 {
                    q = match self.ready.wait(q) {
                        Ok(q) => q,
                        Err(_) => return,
                    };
                }
                q.0.pop_front()
            };
            let Some((_, bytes)) = next else { break };
            let header = (bytes.len() as u32).to_be_bytes();
            if stream
                .write_all(&header)
                .and_then(|_| stream.write_all(&bytes))
                .is_err()
            {
                break;
            }
        }
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

fn read_frame(stream: &mut UnixStream) -> Option<Value> {
    let mut header = [0; 4];
    stream.read_exact(&mut header).ok()?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > MAX_FRAME {
        return None;
    }
    let mut payload = vec![0; size];
    stream.read_exact(&mut payload).ok()?;
    serde_json::from_slice(&payload).ok()
}

/// §2: only this user may connect.
fn same_user(stream: &UnixStream) -> bool {
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: a connected socket of ours and two out pointers.
    let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    // SAFETY: no arguments.
    rc == 0 && uid == unsafe { libc::geteuid() }
}

/// The surface server. `stop` (or dropping it) darkens the controller and sends `released` (§7).
pub struct Server {
    tx: Sender<Event>,
    threads: Vec<JoinHandle<()>>,
    socket: PathBuf,
    closing: Arc<AtomicBool>,
    /// The write end of the pipe that wakes the accepting thread.
    wake: i32,
    watch: Arc<Mutex<Option<watch::Stopper>>>,
}

fn wake(fd: i32) {
    // SAFETY: one byte to our own pipe.
    unsafe { libc::write(fd, [1u8].as_ptr().cast(), 1) };
}

impl Server {
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn start(config: Config) -> io::Result<Self> {
        let socket = config.socket.clone();
        let runtime = socket
            .parent()
            .ok_or_else(|| io::Error::other("no folder for the socket"))?;
        fs::create_dir_all(runtime)?;
        fs::set_permissions(runtime, fs::Permissions::from_mode(0o700))?;
        if fs::symlink_metadata(&config.socket).is_ok() {
            // The serve lock is held: a socket left here is from an agent that did not stop in order.
            fs::remove_file(&config.socket)?;
        }
        let listener = UnixListener::bind(&config.socket)?;
        fs::set_permissions(&config.socket, fs::Permissions::from_mode(0o600))?;

        let (tx, rx) = mpsc::channel();
        let closing = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();

        let mut pipe = [0; 2];
        // SAFETY: room for the two ends.
        if unsafe { libc::pipe(pipe.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let accept_tx = tx.clone();
        let accept_closing = closing.clone();
        let stop_fd = pipe[0];
        threads.push(std::thread::spawn(move || {
            accept(listener, stop_fd, accept_tx, accept_closing);
            // SAFETY: the read end, ours, closed once.
            unsafe { libc::close(stop_fd) };
        }));

        let mut folders = vec![config.controllers.clone()];
        folders.extend(config.recording.clone());
        let watch = Arc::new(Mutex::new(None));
        let watch_tx = tx.clone();
        let stopper = watch.clone();
        threads.push(std::thread::spawn(move || {
            if let Ok(mut w) = watch::Watcher::new(folders) {
                if let Ok(mut s) = stopper.lock() {
                    *s = Some(w.stopper());
                }
                while w.wait() {
                    if watch_tx.send(Event::Files).is_err() {
                        break;
                    }
                }
            }
        }));

        let central_tx = tx.clone();
        threads.push(std::thread::spawn(move || central(config, rx, central_tx)));
        let _ = tx.send(Event::Files);
        Ok(Self {
            tx,
            threads,
            socket,
            closing,
            wake: pipe[1],
            watch,
        })
    }

    /// §7: every light off, `released` to every app, then the socket goes away.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.threads.is_empty() {
            return;
        }
        let _ = self.tx.send(Event::Stop);
        self.closing.store(true, Ordering::SeqCst);
        // Wakes the accepting thread, even if the socket file is gone.
        wake(self.wake);
        if let Ok(Some(s)) = self.watch.lock().map(|w| *w) {
            s.stop();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        // SAFETY: the write end, ours, closed once.
        unsafe { libc::close(self.wake) };
        let _ = fs::remove_file(&self.socket);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Waits on the socket and the stop pipe at once: it sleeps in the kernel until one is ready.
fn accept(listener: UnixListener, stop: i32, tx: Sender<Event>, closing: Arc<AtomicBool>) {
    let next = AtomicU64::new(1);
    loop {
        let mut fds = [
            libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stop,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: two valid pollfds; no timeout.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if n < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        if closing.load(Ordering::SeqCst) || fds[1].revents != 0 {
            break;
        }
        if fds[0].revents == 0 {
            continue;
        }
        let Ok((stream, _)) = listener.accept() else {
            continue;
        };
        if !same_user(&stream) {
            continue;
        }
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        let (Ok(mut reader), Ok(writer)) = (stream.try_clone(), stream.try_clone()) else {
            continue;
        };
        let conn = next.fetch_add(1, Ordering::SeqCst);
        let outbox = Arc::new(Outbox {
            stream: writer,
            queue: Mutex::new((VecDeque::new(), false)),
            ready: Condvar::new(),
        });
        let out = outbox.clone();
        std::thread::spawn(move || out.write_all());
        if tx.send(Event::Accepted(conn, outbox)).is_err() {
            break;
        }
        let tx = tx.clone();
        std::thread::spawn(move || {
            while let Some(v) = read_frame(&mut reader) {
                if tx.send(Event::Frame(conn, v)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Event::Closed(conn));
        });
    }
}

/// Owns the surface, the MIDI system and the apps' outboxes. Every event ends here, one at a time.
fn central(mut config: Config, rx: Receiver<Event>, tx: Sender<Event>) {
    let mut surface = Surface::new(nu9ve_midi::bundled_profiles());
    let mut midi: Option<Box<dyn MidiSystem>> = None;
    let mut apps: BTreeMap<Conn, Arc<Outbox>> = BTreeMap::new();
    let mut hello: BTreeSet<Conn> = BTreeSet::new();
    let blinking = Arc::new(AtomicBool::new(false));
    let mut phase = true;
    // Events read ahead while a file change settled, in order.
    let mut pending: VecDeque<Event> = VecDeque::new();
    while let Some(event) = pending.pop_front().or_else(|| rx.recv().ok()) {
        let outs = match event {
            Event::Accepted(conn, outbox) => {
                apps.insert(conn, outbox);
                Vec::new()
            }
            Event::Frame(conn, v) if !hello.contains(&conn) => {
                let outs = surface.hello(conn, &v);
                if !outs.contains(&Out::Drop(conn)) {
                    hello.insert(conn);
                }
                outs
            }
            Event::Frame(conn, v) => {
                if v.get("type").and_then(Value::as_str) == Some("hello") {
                    Vec::new()
                } else {
                    surface.message(conn, &v)
                }
            }
            Event::Closed(conn) => {
                hello.remove(&conn);
                if let Some(o) = apps.remove(&conn) {
                    o.finish();
                }
                surface.disconnect(conn)
            }
            Event::Port(PortEvent::Connected { name }) => surface.port_connected(&name),
            Event::Port(PortEvent::Disconnected { name }) => surface.port_gone(&name),
            Event::Port(PortEvent::Message {
                name,
                bytes,
                host_ns,
            }) => surface.midi(&name, bytes, host_ns),
            Event::Files => {
                // Let a write settle: a temporary file and its rename come as several events.
                std::thread::sleep(SETTLE);
                while let Ok(e) = rx.try_recv() {
                    if !matches!(e, Event::Files) {
                        pending.push_back(e);
                    }
                }
                let (maps, suite) = read_maps(&config.controllers);
                let mut outs = surface.set_maps(maps, suite);
                let (recording, capturing) = read_recording(config.recording.as_deref());
                outs.extend(surface.set_recording(recording, capturing));
                outs
            }
            Event::Blink => {
                phase = !phase;
                surface.blink(phase)
            }
            Event::Stop => {
                let outs = surface.release();
                deliver(outs, &mut apps, &mut midi);
                drop(midi);
                blinking.store(false, Ordering::SeqCst);
                for (_, o) in std::mem::take(&mut apps) {
                    o.finish();
                }
                return;
            }
        };
        deliver(outs, &mut apps, &mut midi);
        // §8: MIDI open only while an app with a map is connected.
        if surface.wants_midi() && midi.is_none() {
            let port_tx = Mutex::new(tx.clone());
            let sink: Sink = Arc::new(move |e: PortEvent| {
                if let Ok(t) = port_tx.lock() {
                    let _ = t.send(Event::Port(e));
                }
            });
            match (config.midi)(sink) {
                Ok(system) => midi = Some(system),
                Err(e) => super::serve::log(&format!("surface: MIDI: {e}")),
            }
        } else if !surface.wants_midi() && midi.is_some() {
            midi = None;
            surface.midi_closed();
        }
        // The software blink runs only while something blinks.
        let needs = surface.needs_blink();
        if needs && !blinking.swap(true, Ordering::SeqCst) {
            let (flag, tick) = (blinking.clone(), Mutex::new(tx.clone()));
            std::thread::spawn(move || {
                while flag.load(Ordering::SeqCst) {
                    std::thread::sleep(BLINK);
                    if !flag.load(Ordering::SeqCst) {
                        break;
                    }
                    if tick.lock().map(|t| t.send(Event::Blink)).is_err() {
                        break;
                    }
                }
            });
        } else if !needs {
            blinking.store(false, Ordering::SeqCst);
        }
    }
}

fn deliver(
    outs: Vec<Out>,
    apps: &mut BTreeMap<Conn, Arc<Outbox>>,
    midi: &mut Option<Box<dyn MidiSystem>>,
) {
    for out in outs {
        match out {
            Out::Send {
                conn,
                message,
                coalesce,
            } => {
                if let Some(o) = apps.get(&conn) {
                    o.push(&message, coalesce);
                }
            }
            Out::Drop(conn) => {
                if let Some(o) = apps.remove(&conn) {
                    o.finish();
                }
            }
            Out::Open(port) => {
                if let Some(m) = midi
                    && let Err(e) = m.open(&port)
                {
                    super::serve::log(&format!("surface: open {port}: {e}"));
                }
            }
            Out::Close(port) => {
                if let Some(m) = midi {
                    m.close(&port);
                }
            }
            Out::Midi(port, messages) => {
                if let Some(m) = midi
                    && let Err(e) = m.send(&port, &messages)
                {
                    super::serve::log(&format!("surface: lights {port}: {e}"));
                }
            }
        }
    }
}

/// Folder changes by kqueue: the given folders and their subfolders (one per profile).
mod watch {
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    const USER: usize = 1;

    fn event(ident: usize, filter: i16, flags: u16, fflags: u32) -> libc::kevent {
        libc::kevent {
            ident,
            filter,
            flags,
            fflags,
            data: 0,
            udata: std::ptr::null_mut(),
        }
    }

    pub struct Watcher {
        kq: i32,
        roots: Vec<PathBuf>,
        open: Vec<(PathBuf, i32)>,
    }

    #[derive(Clone, Copy)]
    pub struct Stopper(i32);

    impl Stopper {
        pub fn stop(self) {
            let trigger = event(USER, libc::EVFILT_USER, 0, libc::NOTE_TRIGGER);
            // SAFETY: one kevent in, none out, on our kqueue.
            unsafe {
                libc::kevent(
                    self.0,
                    &trigger,
                    1,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
        }
    }

    impl Watcher {
        pub fn new(roots: Vec<PathBuf>) -> io::Result<Self> {
            // SAFETY: no arguments; negative is an error.
            let kq = unsafe { libc::kqueue() };
            if kq < 0 {
                return Err(io::Error::last_os_error());
            }
            let user = event(USER, libc::EVFILT_USER, libc::EV_ADD | libc::EV_CLEAR, 0);
            // SAFETY: one valid kevent in.
            unsafe { libc::kevent(kq, &user, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
            let mut w = Self {
                kq,
                roots,
                open: Vec::new(),
            };
            w.rearm();
            Ok(w)
        }

        pub fn stopper(&self) -> Stopper {
            Stopper(self.kq)
        }

        /// Watches every root and its subfolders that exist and are not watched yet.
        fn rearm(&mut self) {
            let mut folders = Vec::new();
            for root in &self.roots {
                folders.push(root.clone());
                for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
                    if e.file_type().is_ok_and(|t| t.is_dir()) {
                        folders.push(e.path());
                    }
                }
            }
            for folder in folders {
                if self.open.iter().any(|(p, _)| *p == folder) {
                    continue;
                }
                let Ok(c) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
                    continue;
                };
                // SAFETY: a NUL-terminated path, opened only for events.
                let fd = unsafe { libc::open(c.as_ptr(), libc::O_EVTONLY) };
                if fd < 0 {
                    continue;
                }
                let watch = event(
                    fd as usize,
                    libc::EVFILT_VNODE,
                    libc::EV_ADD | libc::EV_CLEAR,
                    libc::NOTE_WRITE | libc::NOTE_DELETE | libc::NOTE_RENAME | libc::NOTE_EXTEND,
                );
                // SAFETY: one valid kevent in.
                let rc = unsafe {
                    libc::kevent(
                        self.kq,
                        &watch,
                        1,
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null(),
                    )
                };
                if rc < 0 {
                    // SAFETY: just opened.
                    unsafe { libc::close(fd) };
                } else {
                    self.open.push((folder, fd));
                }
            }
        }

        /// Waits for a change; false once stopped.
        pub fn wait(&mut self) -> bool {
            let mut out = [event(0, 0, 0, 0); 8];
            loop {
                // SAFETY: room for 8 events, no timeout: it sleeps until something happens.
                let n = unsafe {
                    libc::kevent(
                        self.kq,
                        std::ptr::null(),
                        0,
                        out.as_mut_ptr(),
                        out.len() as i32,
                        std::ptr::null(),
                    )
                };
                if n < 0 {
                    if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return false;
                }
                let mut changed = false;
                for ev in &out[..n as usize] {
                    if ev.filter == libc::EVFILT_USER {
                        return false;
                    }
                    if ev.fflags & (libc::NOTE_DELETE | libc::NOTE_RENAME) != 0 {
                        let fd = ev.ident as i32;
                        self.open.retain(|(_, f)| *f != fd);
                        // SAFETY: ours, forgotten above.
                        unsafe { libc::close(fd) };
                    }
                    changed = true;
                }
                self.rearm();
                if changed {
                    return true;
                }
            }
        }
    }

    impl Drop for Watcher {
        fn drop(&mut self) {
            for (_, fd) in &self.open {
                // SAFETY: our descriptors, closed once.
                unsafe { libc::close(*fd) };
            }
            // SAFETY: our kqueue.
            unsafe { libc::close(self.kq) };
        }
    }
}
