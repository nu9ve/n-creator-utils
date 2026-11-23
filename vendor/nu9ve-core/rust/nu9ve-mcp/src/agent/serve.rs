//! `nu9ve agent serve`: the agent as a background service (nu9ve-agent-v1 §4 and §5). One per machine,
//! held by a lock on `serve.lock` in the agent folder. It publishes this machine's folder when the engine
//! registry or the mounted disks change, and reads the paired machines every few minutes. At rest it waits
//! in the kernel (kqueue on macOS): no timer wakes it between those reads.

use super::{Agent, publish, read};
use serde_json::{Value, json};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often the paired machines are read when nothing wakes the agent before.
const EVERY: Duration = Duration::from_secs(180);
/// A change is published once things settle for this long: an engine writes its record, a disk mounts.
const SETTLE: Duration = Duration::from_millis(800);

/// Set when the service has to stop: a signal, or `engine_shutdown` from the suite engine.
static STOP: AtomicBool = AtomicBool::new(false);

/// `NU9VE_AGENT_EVERY_SECS` shortens the reading interval for the tests; at least one second.
fn every() -> Duration {
    std::env::var("NU9VE_AGENT_EVERY_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map_or(EVERY, |s| Duration::from_secs(s.max(1)))
}

pub(super) fn log(message: &str) {
    let _ = writeln!(
        io::stderr(),
        "{} nu9ve agent: {message}",
        nu9ve_device::now_utc()
    );
}

/// The lock that makes this the only agent serving from its folder. Its text says who holds it.
pub(super) struct ServeLock {
    file: fs::File,
}

pub(super) fn lock_path(dir: &Path) -> PathBuf {
    dir.join("serve.lock")
}

impl ServeLock {
    pub(super) fn take(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path(dir))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                let holder = fs::read_to_string(lock_path(dir)).unwrap_or_default();
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "another agent is already serving here (pid {})",
                        holder.split_whitespace().next().unwrap_or("?")
                    ),
                ));
            }
            Err(fs::TryLockError::Error(e)) => return Err(e),
        }
        file.set_len(0)?;
        writeln!(file, "{}", std::process::id())?;
        Ok(Self { file })
    }
}

impl Drop for ServeLock {
    fn drop(&mut self) {
        let _ = self.file.set_len(0);
        let _ = self.file.unlock();
    }
}

/// The pid of the agent serving from `dir`, if one is.
pub(super) fn running_pid(dir: &Path) -> Option<u32> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path(dir))
        .ok()?;
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            None
        }
        Err(_) => fs::read_to_string(lock_path(dir))
            .ok()?
            .split_whitespace()
            .next()?
            .parse()
            .ok(),
    }
}

/// What woke the service.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Wake {
    Changed,
    Timeout,
    Stop,
}

/// Waits for a change in some folders, a stop or a timeout.
pub(super) use watch::Watcher;

#[cfg(target_os = "macos")]
mod watch {
    use super::{STOP, Wake};
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    const USER_IDENT: usize = 1;

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
        paths: Vec<(PathBuf, Option<i32>)>,
    }

    /// Wakes a waiting [`Watcher`] from another thread.
    #[derive(Clone, Copy)]
    pub struct Stopper(i32);

    impl Stopper {
        pub fn stop(self) {
            STOP.store(true, Ordering::SeqCst);
            let trigger = event(USER_IDENT, libc::EVFILT_USER, 0, libc::NOTE_TRIGGER);
            // SAFETY: one valid kevent in, none out, on a kqueue this process owns.
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
        pub fn new(paths: Vec<PathBuf>) -> io::Result<Self> {
            // SAFETY: kqueue takes no arguments; a negative result is an error.
            let kq = unsafe { libc::kqueue() };
            if kq < 0 {
                return Err(io::Error::last_os_error());
            }
            let mut watcher = Self {
                kq,
                paths: paths.into_iter().map(|p| (p, None)).collect(),
            };
            // SIGTERM (launchctl, an installer) and SIGINT stop the service through the queue.
            let mut changes = vec![event(
                USER_IDENT,
                libc::EVFILT_USER,
                libc::EV_ADD | libc::EV_CLEAR,
                0,
            )];
            for signal in [libc::SIGTERM, libc::SIGINT] {
                // SAFETY: ignoring a signal is always sound; kqueue still records it.
                unsafe { libc::signal(signal, libc::SIG_IGN) };
                changes.push(event(signal as usize, libc::EVFILT_SIGNAL, libc::EV_ADD, 0));
            }
            watcher.apply(&changes)?;
            watcher.rearm();
            Ok(watcher)
        }

        pub fn stopper(&self) -> Stopper {
            Stopper(self.kq)
        }

        fn apply(&self, changes: &[libc::kevent]) -> io::Result<()> {
            // SAFETY: `changes` is a valid slice of kevents; no events are returned.
            let rc = unsafe {
                libc::kevent(
                    self.kq,
                    changes.as_ptr(),
                    changes.len() as i32,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
            if rc < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        /// Watches the folders that exist now and are not watched yet (a QA mounts folder may come later).
        fn rearm(&mut self) {
            for i in 0..self.paths.len() {
                if self.paths[i].1.is_some() {
                    continue;
                }
                let Ok(c) = std::ffi::CString::new(self.paths[i].0.as_os_str().as_bytes()) else {
                    continue;
                };
                // SAFETY: a NUL-terminated path; O_EVTONLY opens it only for events.
                let fd = unsafe { libc::open(c.as_ptr(), libc::O_EVTONLY) };
                if fd < 0 {
                    continue;
                }
                let watch = event(
                    fd as usize,
                    libc::EVFILT_VNODE,
                    libc::EV_ADD | libc::EV_CLEAR,
                    libc::NOTE_WRITE
                        | libc::NOTE_DELETE
                        | libc::NOTE_RENAME
                        | libc::NOTE_EXTEND
                        | libc::NOTE_ATTRIB,
                );
                if self.apply(&[watch]).is_ok() {
                    self.paths[i].1 = Some(fd);
                } else {
                    // SAFETY: fd was opened above and is not used elsewhere.
                    unsafe { libc::close(fd) };
                }
            }
        }

        pub fn wait(&mut self, timeout: Duration) -> Wake {
            if STOP.load(Ordering::SeqCst) {
                return Wake::Stop;
            }
            self.rearm();
            let spec = libc::timespec {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_nsec: libc::c_long::from(timeout.subsec_nanos() as i32),
            };
            let mut out = [event(0, 0, 0, 0); 8];
            // SAFETY: `out` has room for 8 events; the timeout is a valid timespec.
            let n = unsafe {
                libc::kevent(
                    self.kq,
                    std::ptr::null(),
                    0,
                    out.as_mut_ptr(),
                    out.len() as i32,
                    &spec,
                )
            };
            if n <= 0 {
                return if STOP.load(Ordering::SeqCst) {
                    Wake::Stop
                } else {
                    Wake::Timeout
                };
            }
            let mut wake = Wake::Timeout;
            for ev in &out[..n as usize] {
                match ev.filter {
                    libc::EVFILT_SIGNAL | libc::EVFILT_USER => {
                        STOP.store(true, Ordering::SeqCst);
                        return Wake::Stop;
                    }
                    libc::EVFILT_VNODE => {
                        if ev.fflags & (libc::NOTE_DELETE | libc::NOTE_RENAME) != 0 {
                            // The folder went away: close it and watch it again when it comes back.
                            for (_, fd) in &mut self.paths {
                                if *fd == Some(ev.ident as i32) {
                                    // SAFETY: the fd is ours and is forgotten right after.
                                    unsafe { libc::close(ev.ident as i32) };
                                    *fd = None;
                                }
                            }
                        }
                        wake = Wake::Changed;
                    }
                    _ => {}
                }
            }
            wake
        }
    }

    impl Drop for Watcher {
        fn drop(&mut self) {
            for (_, fd) in &self.paths {
                if let Some(fd) = fd {
                    // SAFETY: our own descriptors, closed once.
                    unsafe { libc::close(*fd) };
                }
            }
            // SAFETY: our own kqueue, closed once.
            unsafe { libc::close(self.kq) };
        }
    }
}

/// Other systems compare a listing of the folders every two seconds. Not verified on Windows.
#[cfg(not(target_os = "macos"))]
mod watch {
    use super::{STOP, Wake};
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant, SystemTime};

    const POLL: Duration = Duration::from_secs(2);

    pub struct Watcher {
        paths: Vec<PathBuf>,
        last: Vec<(String, Option<SystemTime>)>,
    }

    #[derive(Clone, Copy)]
    pub struct Stopper;

    impl Stopper {
        #[cfg_attr(not(unix), allow(dead_code))]
        pub fn stop(self) {
            STOP.store(true, Ordering::SeqCst);
        }
    }

    #[cfg(unix)]
    extern "C" fn on_signal(_: libc::c_int) {
        STOP.store(true, Ordering::SeqCst);
    }

    impl Watcher {
        pub fn new(paths: Vec<PathBuf>) -> io::Result<Self> {
            #[cfg(unix)]
            for signal in [libc::SIGTERM, libc::SIGINT] {
                // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
                unsafe { libc::signal(signal, on_signal as *const () as libc::sighandler_t) };
            }
            let mut watcher = Self {
                paths,
                last: Vec::new(),
            };
            watcher.last = watcher.listing();
            Ok(watcher)
        }

        pub fn stopper(&self) -> Stopper {
            Stopper
        }

        fn listing(&self) -> Vec<(String, Option<SystemTime>)> {
            let mut out = Vec::new();
            for path in &self.paths {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for e in entries.flatten() {
                        let modified = e.metadata().ok().and_then(|m| m.modified().ok());
                        out.push((e.path().to_string_lossy().into_owned(), modified));
                    }
                }
            }
            out.sort();
            out
        }

        pub fn wait(&mut self, timeout: Duration) -> Wake {
            let deadline = Instant::now() + timeout;
            loop {
                if STOP.load(Ordering::SeqCst) {
                    return Wake::Stop;
                }
                let now = Instant::now();
                if now >= deadline {
                    return Wake::Timeout;
                }
                std::thread::sleep(POLL.min(deadline - now));
                let listing = self.listing();
                if listing != self.last {
                    self.last = listing;
                    return Wake::Changed;
                }
            }
        }
    }
}

pub(super) use watch::Stopper;

/// The folders whose changes make the agent publish: the engine registry and the mounted disks.
fn watched() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = nu9ve_ipc::default_dir() {
        paths.push(dir);
    }
    paths.push(PathBuf::from(
        std::env::var("NU9VE_VOLUMES_DIR").unwrap_or_else(|_| "/Volumes".into()),
    ));
    paths
}

/// One pass of the service: publish, and also read when it is time. Errors are logged, never fatal:
/// a missing exchange folder or an engine that does not answer must not stop the agent.
fn cycle(agent: &Mutex<Agent>, reading: bool) {
    let Ok(mut agent) = agent.lock() else {
        return;
    };
    agent.refresh();
    let agent = &*agent;
    if agent.exchange().is_err() {
        return;
    }
    match publish(agent) {
        Ok(out) if out["written"].as_array().is_some_and(|w| !w.is_empty()) => {
            log(&format!("published {}", out["written"]));
        }
        Ok(_) => {}
        Err(e) => log(&format!("publish: {e}")),
    }
    if reading {
        match read(agent) {
            Ok(_) => {
                let pass = super::trade::pass(agent);
                if pass.as_object().is_some_and(|m| {
                    m.values()
                        .any(|v| v.as_array().is_none_or(|l| !l.is_empty()))
                }) {
                    log(&format!("trade {pass}"));
                }
            }
            Err(e) => log(&format!("read: {e}")),
        }
    }
}

/// Runs until a signal or `engine_shutdown`. Returns why it stopped.
pub(super) fn serve(agent: Agent) -> io::Result<Value> {
    let _lock = ServeLock::take(&agent.dir)?;
    let mut watcher = Watcher::new(watched())?;
    log(&format!(
        "serving {} (pid {})",
        agent.device_id(),
        std::process::id()
    ));
    let agent = Arc::new(Mutex::new(agent));
    let _engine = super::suite::start(agent.clone(), watcher.stopper())?;
    let every = every();
    cycle(&agent, true);
    let mut next_read = Instant::now() + every;
    loop {
        let left = next_read.saturating_duration_since(Instant::now());
        match watcher.wait(left) {
            Wake::Stop => break,
            Wake::Changed => {
                // Let the change settle: an engine writes its record, a disk mounts with several events.
                loop {
                    match watcher.wait(SETTLE) {
                        Wake::Changed => continue,
                        Wake::Timeout => break,
                        Wake::Stop => {
                            log("stopping");
                            return Ok(json!({"stopped": true}));
                        }
                    }
                }
                cycle(&agent, false);
            }
            Wake::Timeout => {
                cycle(&agent, true);
                next_read = Instant::now() + every;
            }
        }
    }
    if STOP.load(Ordering::SeqCst) {
        log("stopping");
    }
    Ok(json!({"stopped": true}))
}
