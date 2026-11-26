//! Helpers to produce runs: the machine (§4), the subject (§3), inputs (§5) and the measurements.
//!
//! [`measure_fn`] times a function in this process; [`measure_command`] runs a process once per repetition and
//! reads its CPU time and peak RSS from `wait4`, which is exact for that process but does not see its
//! grandchildren (contract §14). Both give the `repeats`, `warmup`, `status`, `metrics` and `error` fields of a
//! run; [`new_run`] puts a run together.
//!
//! [`measure_soak`] repeats one operation many times while a thread samples the resident memory and CPU time
//! of a process ([`process_sample`]: `proc_pid_rusage` on macOS, `/proc` on Linux), and gives the `resources`
//! series and the memory slope of a soak (§5.1, §5.2).

use crate::{
    CONTRACT, MAX_RESOURCE_SAMPLES, SOAK_GROWTH, SOAK_SLOPE, distribution, memory_slope,
    new_run_id, run_problem, time,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const TOOL: &str = concat!("nu9ve-bench ", env!("CARGO_PKG_VERSION"));

// ------------------------------------------------------------------------------------------- machine

#[cfg(target_os = "macos")]
fn sysctl_bytes(name: &str) -> Option<Vec<u8>> {
    let name = std::ffi::CString::new(name).ok()?;
    let mut size: libc::size_t = 0;
    // SAFETY: a null buffer asks only for the size; `name` is a valid C string.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || size == 0 {
        return None;
    }
    let mut buffer = vec![0u8; size];
    // SAFETY: `buffer` has `size` writable bytes and sysctlbyname writes at most that many.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    buffer.truncate(size);
    Some(buffer)
}

#[cfg(target_os = "macos")]
fn sysctl_string(name: &str) -> Option<String> {
    let bytes = sysctl_bytes(name)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_end_matches('\0').trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(target_os = "macos")]
fn sysctl_u64(name: &str) -> Option<u64> {
    let bytes = sysctl_bytes(name)?;
    match bytes.len() {
        8 => Some(u64::from_ne_bytes(bytes.try_into().ok()?)),
        4 => Some(u64::from(u32::from_ne_bytes(bytes.try_into().ok()?))),
        _ => None,
    }
}

fn arch() -> Value {
    match std::env::consts::ARCH {
        "aarch64" => json!("aarch64"),
        "x86_64" => json!("x86_64"),
        _ => Value::Null,
    }
}

fn logical_cores() -> Value {
    std::thread::available_parallelism()
        .map(|n| json!(n.get()))
        .unwrap_or(Value::Null)
}

/// §4 for this computer: never the host name, the user or a serial number.
#[cfg(target_os = "macos")]
pub fn machine() -> Value {
    json!({
        "model": sysctl_string("hw.model"),
        "chip": sysctl_string("machdep.cpu.brand_string"),
        "cpu_cores": sysctl_u64("hw.logicalcpu").map_or_else(logical_cores, |n| json!(n)),
        "memory_bytes": sysctl_u64("hw.memsize"),
        "os": sysctl_string("kern.osproductversion").map(|v| format!("macOS {v}")),
        "arch": arch(),
    })
}

#[cfg(target_os = "linux")]
pub fn machine() -> Value {
    let memory = std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| {
        m.lines()
            .find_map(|l| l.strip_prefix("MemTotal:"))
            .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    });
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|r| format!("Linux {}", r.trim()));
    json!({
        "model": null, "chip": null, "cpu_cores": logical_cores(), "memory_bytes": memory,
        "os": release, "arch": arch(),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn machine() -> Value {
    json!({
        "model": null, "chip": null, "cpu_cores": logical_cores(), "memory_bytes": null,
        "os": if cfg!(windows) { Value::from("Windows") } else { Value::Null }, "arch": arch(),
    })
}

// ------------------------------------------------------------------------------------------- subject, inputs

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// §3, with the commit and whether the checkout is dirty read from `repo` when given.
pub fn subject(app: &str, engine: &str, version: Option<&str>, repo: Option<&Path>) -> Value {
    let commit = repo
        .and_then(|r| git(r, &["rev-parse", "--short=12", "HEAD"]))
        .filter(|c| !c.is_empty());
    let dirty = match (repo, &commit) {
        (Some(r), Some(_)) => git(r, &["status", "--porcelain", "--untracked-files=no"])
            .map(|s| Value::Bool(!s.is_empty()))
            .unwrap_or(Value::Null),
        _ => Value::Null,
    };
    json!({"app": app, "engine": engine, "version": version, "commit": commit, "dirty": dirty})
}

/// §5: an input by its generic name, with the hash and size of the file at `path`, read in blocks.
pub fn file_input(name: &str, path: &Path) -> io::Result<Value> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut size: u64 = 0;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok(json!({"name": name, "sha256": nu9ve_json::hex(&hasher.finalize()), "bytes": size}))
}

// ------------------------------------------------------------------------------------------- measuring

/// The measured part of a run: `repeats`, `warmup`, `status`, `metrics` and, when it failed, `error`.
#[derive(Debug, Clone)]
pub struct Measured {
    pub repeats: u64,
    pub warmup: u64,
    pub status: &'static str,
    pub metrics: Option<Value>,
    pub error: Option<Value>,
}

#[cfg(unix)]
fn timeval_ms(t: libc::timeval) -> f64 {
    t.tv_sec as f64 * 1000.0 + t.tv_usec as f64 / 1000.0
}

#[cfg(unix)]
fn self_cpu_ms() -> Option<f64> {
    // SAFETY: getrusage fills the zeroed struct it is given.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `usage` is a valid, writable rusage.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    (rc == 0).then(|| timeval_ms(usage.ru_utime) + timeval_ms(usage.ru_stime))
}

#[cfg(not(unix))]
fn self_cpu_ms() -> Option<f64> {
    None
}

fn metrics(wall: &[f64], cpu: &[f64], memory: Option<u64>) -> Value {
    let mut map = Map::new();
    map.insert("wall_ms".into(), distribution(wall));
    if cpu.len() == wall.len() {
        map.insert("cpu_ms".into(), distribution(cpu));
    }
    if let Some(bytes) = memory {
        map.insert(
            "peak_memory".into(),
            json!({"bytes": bytes, "measure": "rss", "scope": "process"}),
        );
    }
    Value::Object(map)
}

/// Times `function` in this process: wall time and the process's CPU time of each repetition.
pub fn measure_fn(repeats: u64, warmup: u64, mut function: impl FnMut()) -> Measured {
    for _ in 0..warmup {
        function();
    }
    let (mut wall, mut cpu) = (Vec::new(), Vec::new());
    for _ in 0..repeats {
        let cpu_start = self_cpu_ms();
        let start = Instant::now();
        function();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if let (Some(a), Some(b)) = (cpu_start, self_cpu_ms()) {
            cpu.push((b - a).max(0.0));
        }
        wall.push(elapsed);
    }
    Measured {
        repeats,
        warmup,
        status: "ok",
        metrics: (repeats > 0).then(|| metrics(&wall, &cpu, None)),
        error: None,
    }
}

struct Exit {
    code: Option<i32>,
    cpu_ms: Option<f64>,
    max_rss: Option<u64>,
}

#[cfg(unix)]
fn wait_child(child: std::process::Child) -> io::Result<Exit> {
    let pid = child.id() as libc::pid_t;
    let mut status: libc::c_int = 0;
    // SAFETY: wait4 fills the zeroed struct it is given.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: `pid` is our own child, not yet reaped; `status` and `usage` are valid and writable.
        let rc = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
        if rc == pid {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    drop(child); // already reaped: dropping a Child does not wait
    let code = if libc::WIFEXITED(status) {
        Some(libc::WEXITSTATUS(status))
    } else {
        None
    };
    // ru_maxrss is in bytes on macOS and in kilobytes on Linux.
    let unit: u64 = if cfg!(target_os = "macos") { 1 } else { 1024 };
    Ok(Exit {
        code,
        cpu_ms: Some(timeval_ms(usage.ru_utime) + timeval_ms(usage.ru_stime)),
        max_rss: u64::try_from(usage.ru_maxrss).ok().map(|r| r * unit),
    })
}

#[cfg(not(unix))]
fn wait_child(mut child: std::process::Child) -> io::Result<Exit> {
    let status = child.wait()?;
    Ok(Exit {
        code: status.code(),
        cpu_ms: None,
        max_rss: None,
    })
}

/// Runs the command `make()` builds once per repetition (stdin, stdout and stderr are null unless `make` sets
/// them) and measures each as a process. A non-zero exit stops and gives a failed run with what was measured.
pub fn measure_command(
    repeats: u64,
    warmup: u64,
    mut make: impl FnMut() -> Command,
) -> io::Result<Measured> {
    let (mut wall, mut cpu, mut peak) = (Vec::new(), Vec::new(), None::<u64>);
    for index in 0..warmup + repeats {
        let mut command = make();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let start = Instant::now();
        let exit = wait_child(command.spawn()?)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if exit.code != Some(0) {
            let message = match exit.code {
                Some(code) => format!("exit status {code}"),
                None => "terminated by a signal".to_owned(),
            };
            return Ok(Measured {
                repeats: wall.len() as u64,
                warmup,
                status: "failed",
                metrics: (!wall.is_empty()).then(|| metrics(&wall, &cpu, peak)),
                error: Some(json!({"code": "exit_status", "message": message})),
            });
        }
        if index < warmup {
            continue;
        }
        wall.push(elapsed);
        cpu.extend(exit.cpu_ms);
        if let Some(rss) = exit.max_rss {
            peak = Some(peak.map_or(rss, |p| p.max(rss)));
        }
    }
    Ok(Measured {
        repeats,
        warmup,
        status: "ok",
        metrics: (repeats > 0).then(|| metrics(&wall, &cpu, peak)),
        error: None,
    })
}

// ------------------------------------------------------------------------------------------- soak

/// Resident memory and accumulated CPU time of a process at one instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessSample {
    pub rss_bytes: u64,
    pub cpu_ms: u64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    // libc marks its binding deprecated in favour of the mach2 crate; this one call does not need a dependency.
    fn mach_timebase_info(info: *mut Timebase) -> libc::c_int;
}

/// `ri_user_time` and `ri_system_time` are in Mach absolute time units (not nanoseconds on Apple silicon).
#[cfg(target_os = "macos")]
fn mach_to_ms(ticks: u64) -> u64 {
    let mut base = Timebase { numer: 0, denom: 0 };
    // SAFETY: `base` is a valid, writable struct with the layout of mach_timebase_info_data_t.
    let rc = unsafe { mach_timebase_info(&mut base) };
    if rc != 0 || base.denom == 0 {
        return ticks / 1_000_000;
    }
    let nanos = u128::from(ticks) * u128::from(base.numer) / u128::from(base.denom);
    u64::try_from(nanos / 1_000_000).unwrap_or(u64::MAX)
}

/// The resident memory and CPU time of `pid` (this process when `None`), or `None` when it cannot be read
/// (the process ended, or another user's process, or a system without support).
#[cfg(target_os = "macos")]
pub fn process_sample(pid: Option<u32>) -> Option<ProcessSample> {
    let pid = match pid {
        Some(pid) => libc::c_int::try_from(pid).ok()?,
        None => std::process::id().try_into().ok()?,
    };
    // SAFETY: an all-zero rusage_info_v2 is a valid value of the struct.
    let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let buffer: *mut libc::rusage_info_v2 = &mut info;
    // SAFETY: flavor V2 writes exactly one rusage_info_v2 into the buffer, which points at `info`.
    let rc = unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, buffer.cast()) };
    (rc == 0).then(|| ProcessSample {
        rss_bytes: info.ri_resident_size,
        cpu_ms: mach_to_ms(info.ri_user_time.saturating_add(info.ri_system_time)),
    })
}

#[cfg(target_os = "linux")]
pub fn process_sample(pid: Option<u32>) -> Option<ProcessSample> {
    let dir = match pid {
        Some(pid) => format!("/proc/{pid}"),
        None => "/proc/self".to_owned(),
    };
    // SAFETY: sysconf only reads a configuration value.
    let (page, ticks) = unsafe {
        (
            libc::sysconf(libc::_SC_PAGESIZE),
            libc::sysconf(libc::_SC_CLK_TCK),
        )
    };
    let (page, ticks) = (u64::try_from(page).ok()?, u64::try_from(ticks).ok()?);
    let statm = std::fs::read_to_string(format!("{dir}/statm")).ok()?;
    let resident: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    let stat = std::fs::read_to_string(format!("{dir}/stat")).ok()?;
    // The command name may hold spaces and parentheses: the fields start after the last ')'. utime and stime
    // are fields 14 and 15, so the 12th and 13th after the name.
    let mut fields = stat
        .get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .skip(11);
    let user: u64 = fields.next()?.parse().ok()?;
    let system: u64 = fields.next()?.parse().ok()?;
    (ticks > 0).then(|| ProcessSample {
        rss_bytes: resident.saturating_mul(page),
        cpu_ms: (user + system).saturating_mul(1000) / ticks,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn process_sample(_pid: Option<u32>) -> Option<ProcessSample> {
    None
}

/// How to run a soak: `warmup` iterations first, then `iterations` measured ones, sampling the process every
/// `interval_ms` milliseconds. `slope_limit` is the most memory growth per iteration, in bytes, that passes.
#[derive(Debug, Clone)]
pub struct Soak {
    pub iterations: u64,
    pub warmup: u64,
    pub interval_ms: u64,
    /// The process to sample; `None` is this one (the operation runs in this process).
    pub pid: Option<u32>,
    pub slope_limit: f64,
    /// At most this many samples in the run (`resources`), so the line fits in 64 KiB; the slope uses all.
    pub max_samples: usize,
}

impl Soak {
    pub fn new(iterations: u64, warmup: u64, slope_limit: f64) -> Self {
        Soak {
            iterations,
            warmup,
            interval_ms: 50,
            pid: None,
            slope_limit,
            max_samples: 2000,
        }
    }
}

/// A measured soak: the run's measured fields, its `quality` (slope and growth) and its `params`
/// (`iterations`, `warmup_iterations`, `interval_ms`), which the producer may extend.
#[derive(Debug, Clone)]
pub struct Soaked {
    pub measured: Measured,
    pub quality: Value,
    pub params: Value,
}

/// One sample: how many iterations had finished, and the process then.
type Point = (u64, ProcessSample);

/// §5.1: halves the series (every other sample, keeping the last) until it fits, doubling the interval.
pub fn resources(interval_ms: u64, samples: &[ProcessSample], max_samples: usize) -> Value {
    let mut kept: Vec<ProcessSample> = samples.to_vec();
    let mut interval = interval_ms.max(1);
    let limit = max_samples.clamp(2, MAX_RESOURCE_SAMPLES);
    while kept.len() > limit {
        let last = kept.len() - 1;
        kept = kept
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0 || *i == last)
            .map(|(_, s)| *s)
            .collect();
        interval = interval.saturating_mul(2);
    }
    // CPU time never goes back in the series, even if a reading rounds down.
    let mut cpu = Vec::with_capacity(kept.len());
    let mut highest = 0;
    for sample in &kept {
        highest = highest.max(sample.cpu_ms);
        cpu.push(highest);
    }
    json!({
        "interval_ms": interval,
        "rss_bytes": kept.iter().map(|s| s.rss_bytes).collect::<Vec<_>>(),
        "cpu_ms": cpu,
    })
}

fn soak_failed(warmup: u64, code: &str, message: String) -> Measured {
    Measured {
        repeats: 0,
        warmup,
        status: "failed",
        metrics: None,
        error: Some(json!({"code": code, "message": message})),
    }
}

/// Runs `step` `warmup + iterations` times while a thread samples the process (§5.2). The run is one
/// repetition: `wall_ms` and `cpu_ms` of the measured iterations, `peak_memory`, `resources` over the whole
/// soak and `throughput` in iterations per second. `quality` has the slope of the resident memory on the
/// iteration count after the warmup, with `slope_limit` and `le`, and the growth from the end of the warmup to
/// the end. A step that fails, or a process that cannot be sampled, gives a failed run.
pub fn measure_soak(soak: &Soak, mut step: impl FnMut() -> io::Result<()>) -> io::Result<Soaked> {
    if soak.iterations == 0 || !soak.slope_limit.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a soak needs at least one iteration and a finite slope limit",
        ));
    }
    let params = json!({
        "iterations": soak.iterations,
        "warmup_iterations": soak.warmup,
        "interval_ms": soak.interval_ms.max(1),
    });
    let done = |measured| Soaked {
        measured,
        quality: json!([]),
        params: params.clone(),
    };
    let pid = soak.pid;
    let Some(first) = process_sample(pid) else {
        return Ok(done(soak_failed(
            0,
            "sample_failed",
            "the process could not be sampled".into(),
        )));
    };
    let finished = Arc::new(AtomicU64::new(0));
    let points: Arc<Mutex<Vec<Point>>> = Arc::new(Mutex::new(Vec::with_capacity(4096)));
    points
        .lock()
        .map_err(|_| io::Error::other("poisoned"))?
        .push((0, first));
    let (stop, stopped) = mpsc::channel::<()>();
    let interval = Duration::from_millis(soak.interval_ms.max(1));
    let sampler = {
        let (finished, points) = (Arc::clone(&finished), Arc::clone(&points));
        std::thread::spawn(move || {
            while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(interval) {
                if let Some(sample) = process_sample(pid)
                    && let Ok(mut points) = points.lock()
                {
                    points.push((finished.load(Ordering::SeqCst), sample));
                }
            }
        })
    };
    let mark = |count: u64| -> Option<ProcessSample> {
        let sample = process_sample(pid)?;
        points.lock().ok()?.push((count, sample));
        Some(sample)
    };

    let mut outcome: Result<(f64, ProcessSample, ProcessSample), Measured> =
        Ok((0.0, first, first));
    for index in 0..soak.warmup {
        if let Err(error) = step() {
            outcome = Err(soak_failed(
                soak.warmup,
                "step_failed",
                format!("warmup iteration {index} failed: {:?}", error.kind()),
            ));
            break;
        }
        finished.store(index + 1, Ordering::SeqCst);
    }
    if outcome.is_ok() {
        outcome = match mark(soak.warmup) {
            None => Err(soak_failed(
                soak.warmup,
                "sample_failed",
                "the process could not be sampled".into(),
            )),
            Some(start) => {
                let clock = Instant::now();
                let mut failed = None;
                for index in 0..soak.iterations {
                    if let Err(error) = step() {
                        failed = Some(format!("iteration {index} failed: {:?}", error.kind()));
                        break;
                    }
                    finished.store(soak.warmup + index + 1, Ordering::SeqCst);
                }
                let wall = clock.elapsed().as_secs_f64() * 1000.0;
                match (failed, mark(soak.warmup + soak.iterations)) {
                    (Some(message), _) => Err(soak_failed(soak.warmup, "step_failed", message)),
                    (None, None) => Err(soak_failed(
                        soak.warmup,
                        "sample_failed",
                        "the process could not be sampled".into(),
                    )),
                    (None, Some(end)) => Ok((wall, start, end)),
                }
            }
        };
    }
    // The receiver ends the loop when the sender is dropped or sends; either way the thread stops.
    let _ = stop.send(());
    sampler
        .join()
        .map_err(|_| io::Error::other("the sampler panicked"))?;
    let (wall, start, end) = match outcome {
        Err(measured) => return Ok(done(measured)),
        Ok(values) => values,
    };
    let points = points
        .lock()
        .map_err(|_| io::Error::other("poisoned"))?
        .clone();

    let after: Vec<&Point> = points.iter().filter(|(n, _)| *n >= soak.warmup).collect();
    let xs: Vec<f64> = after.iter().map(|(n, _)| *n as f64).collect();
    let ys: Vec<f64> = after.iter().map(|(_, s)| s.rss_bytes as f64).collect();
    // The end sample is at warmup + iterations and the start one at warmup, so there are two distinct x.
    let slope = memory_slope(&xs, &ys).unwrap_or(0.0);
    let growth = i128::from(end.rss_bytes) - i128::from(start.rss_bytes);
    let growth = i64::try_from(growth).unwrap_or(if growth < 0 { i64::MIN } else { i64::MAX });
    let samples: Vec<ProcessSample> = points.iter().map(|(_, s)| *s).collect();
    let peak = samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0);

    let mut metrics = Map::new();
    metrics.insert("wall_ms".into(), distribution(&[wall]));
    metrics.insert(
        "cpu_ms".into(),
        distribution(&[end.cpu_ms.saturating_sub(start.cpu_ms) as f64]),
    );
    metrics.insert(
        "peak_memory".into(),
        json!({"bytes": peak, "measure": "rss", "scope": "process"}),
    );
    if wall > 0.0 {
        metrics.insert(
            "throughput".into(),
            json!({"value": soak.iterations as f64 / (wall / 1000.0), "unit": "iter/s"}),
        );
    }
    metrics.insert(
        "resources".into(),
        resources(soak.interval_ms, &samples, soak.max_samples),
    );
    let quality = json!([
        {"metric": SOAK_SLOPE, "value": slope, "reference": null, "limit": soak.slope_limit, "op": "le",
         "pass": slope <= soak.slope_limit},
        {"metric": SOAK_GROWTH, "value": growth, "reference": null, "pass": null},
    ]);
    Ok(Soaked {
        measured: Measured {
            repeats: 1,
            warmup: 0,
            status: "ok",
            metrics: Some(Value::Object(metrics)),
            error: None,
        },
        quality,
        params,
    })
}

// ------------------------------------------------------------------------------------------- runs

/// What identifies a measured run; the rest comes from [`Measured`].
#[derive(Debug, Clone)]
pub struct RunInfo {
    pub suite: String,
    pub case: String,
    pub category: String,
    pub subject: Value,
    /// `None` detects this machine with [`machine`].
    pub machine: Option<Value>,
    pub inputs: Vec<Value>,
    pub params: Option<Value>,
    pub cache: &'static str,
    pub privacy: &'static str,
    pub quality: Option<Value>,
    pub note: Option<String>,
}

/// A measured run with a fresh id and the current time, in the contract's field order, already validated.
pub fn new_run(info: RunInfo, measured: Measured) -> io::Result<Value> {
    let id = new_run_id().map_err(io::Error::other)?;
    let mut run = Map::new();
    run.insert("contract".into(), json!(CONTRACT));
    run.insert("run_id".into(), json!(id));
    run.insert("at".into(), json!(time::now_utc()));
    run.insert("suite".into(), json!(info.suite));
    run.insert("case".into(), json!(info.case));
    run.insert("category".into(), json!(info.category));
    run.insert("subject".into(), info.subject);
    run.insert("machine".into(), info.machine.unwrap_or_else(machine));
    run.insert("inputs".into(), Value::Array(info.inputs));
    if let Some(params) = info.params {
        run.insert("params".into(), params);
    }
    run.insert("cache".into(), json!(info.cache));
    run.insert("repeats".into(), json!(measured.repeats));
    run.insert("warmup".into(), json!(measured.warmup));
    run.insert("status".into(), json!(measured.status));
    if let Some(metrics) = measured.metrics {
        run.insert("metrics".into(), metrics);
    }
    if let Some(quality) = info.quality {
        run.insert("quality".into(), quality);
    }
    if let Some(error) = measured.error {
        run.insert("error".into(), error);
    }
    run.insert("privacy".into(), json!(info.privacy));
    run.insert("source".into(), json!({"kind": "measured", "tool": TOOL}));
    if let Some(note) = info.note {
        run.insert("note".into(), json!(note));
    }
    let run = Value::Object(run);
    match run_problem(&run) {
        None => Ok(run),
        Some(problem) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a nu9ve-bench/1 run: {problem}"),
        )),
    }
}
