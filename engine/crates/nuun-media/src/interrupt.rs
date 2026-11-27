//! Cooperative cancellation for one-shot workers: the job stops at a recoverable checkpoint and is recorded
//! `paused`, so `resume` continues it.
//!
//! Every platform: a cancel file (nu9ve-engine-v1 §6.2, as Clipper's supervised jobs). The client passes an
//! absolute path in `NUUN_CANCEL_FILE` and creates that file to cancel; a worker whose file already exists stops
//! at its first checkpoint. Windows has no SIGINT, so this is its way (decided 2026-11-26); the client then gives
//! a grace period and cuts the worker's Job Object.
//! macOS also stops on SIGINT. Windows still accepts the named event in `NUUN_CANCEL_EVENT`
//! (`Local\nuun-cancel-…`) until its client moves to the file.

use nu9ve_json::{Error, Result};
use std::io;
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[cfg(target_os = "macos")]
static CANCELLED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
extern "C" fn interrupt(_: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
struct Platform(libc::sigaction);

#[cfg(target_os = "macos")]
impl Platform {
    fn install() -> Result<Self> {
        CANCELLED.store(false, Ordering::Relaxed);
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = interrupt as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut old = std::mem::MaybeUninit::uninit();
            if libc::sigaction(libc::SIGINT, &action, old.as_mut_ptr()) != 0 {
                return Err(Error::new(
                    "internal_error",
                    io::Error::last_os_error().to_string(),
                ));
            }
            Ok(Self(old.assume_init()))
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for Platform {
    fn drop(&mut self) {
        unsafe {
            libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut());
        }
    }
}

#[cfg(target_os = "macos")]
fn signalled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

/// The event handle of the running worker (0: none, the worker cannot be cancelled).
#[cfg(windows)]
static EVENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Environment variable with the name of the event the client signals to cancel.
#[cfg(windows)]
pub const CANCEL_EVENT_VAR: &str = "NUUN_CANCEL_EVENT";
#[cfg(windows)]
const CANCEL_EVENT_PREFIX: &str = "Local\\nuun-cancel-";

#[cfg(windows)]
struct Platform(());

#[cfg(windows)]
impl Platform {
    fn install() -> Result<Self> {
        Self::open(std::env::var_os(CANCEL_EVENT_VAR).map(|n| n.to_string_lossy().into_owned()))
    }

    fn open(name: Option<String>) -> Result<Self> {
        use windows_sys::Win32::System::Threading::{OpenEventW, SYNCHRONIZATION_SYNCHRONIZE};
        let Some(name) = name else {
            return Ok(Self(()));
        };
        if !name.starts_with(CANCEL_EVENT_PREFIX) || name.contains('\0') {
            return Err(Error::new(
                "invalid_params",
                format!("{CANCEL_EVENT_VAR} must start with {CANCEL_EVENT_PREFIX}"),
            ));
        }
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
        let handle = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide.as_ptr()) };
        if handle.is_null() {
            // The client creates the event before starting the worker: a missing one is a client error.
            return Err(Error::new(
                "internal_error",
                format!("{name}: {}", io::Error::last_os_error()),
            ));
        }
        EVENT.store(handle as usize, Ordering::Relaxed);
        Ok(Self(()))
    }
}

#[cfg(windows)]
impl Drop for Platform {
    fn drop(&mut self) {
        let handle = EVENT.swap(0, Ordering::Relaxed);
        if handle != 0 {
            // SAFETY: the handle came from OpenEventW in `install` and is closed once.
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle as _) };
        }
    }
}

#[cfg(windows)]
fn signalled() -> bool {
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    let handle = EVENT.load(Ordering::Relaxed);
    // SAFETY: a live event handle with SYNCHRONIZE, polled without waiting.
    handle != 0 && unsafe { WaitForSingleObject(handle as _, 0) } == WAIT_OBJECT_0
}

#[cfg(not(any(target_os = "macos", windows)))]
struct Platform(());
#[cfg(not(any(target_os = "macos", windows)))]
impl Platform {
    fn install() -> Result<Self> {
        Ok(Self(()))
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
fn signalled() -> bool {
    false
}

/// Environment variable with the absolute path of the file whose existence cancels the worker.
pub const CANCEL_FILE_VAR: &str = "NUUN_CANCEL_FILE";

/// The cancel file of the running worker. Only a guard that set one clears it, so a worker without the variable
/// never forgets another's.
static CANCEL_FILE: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Installed once by a dedicated one-shot worker for the length of its job.
pub(crate) struct Cancel {
    _platform: Platform,
    file: bool,
}

impl Cancel {
    pub(crate) fn install() -> Result<Self> {
        Self::with_file(std::env::var_os(CANCEL_FILE_VAR))
    }

    fn with_file(file: Option<std::ffi::OsString>) -> Result<Self> {
        let file = file.filter(|f| !f.is_empty()).map(std::path::PathBuf::from);
        if let Some(path) = &file
            && !path.is_absolute()
        {
            return Err(Error::new(
                "invalid_params",
                format!("{CANCEL_FILE_VAR} must be an absolute path"),
            ));
        }
        let platform = Platform::install()?;
        let set = file.is_some();
        if set {
            *CANCEL_FILE.lock().unwrap_or_else(|e| e.into_inner()) = file;
        }
        Ok(Self {
            _platform: platform,
            file: set,
        })
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        if self.file {
            *CANCEL_FILE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }
}

/// True once the client asked to stop: the cancel file exists, or the platform's signal arrived.
pub(crate) fn cancelled() -> bool {
    signalled()
        || CANCEL_FILE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            .is_some_and(|path| path.exists())
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn a_relative_cancel_file_is_refused() {
        assert_eq!(
            Cancel::with_file(Some("relative/cancel".into()))
                .err()
                .unwrap()
                .code,
            "invalid_params"
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};

    #[test]
    fn a_signalled_event_cancels_and_a_missing_one_is_refused() {
        let name = format!("{CANCEL_EVENT_PREFIX}test-{}", std::process::id());
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: manual-reset, initially unsignalled event with a NUL-terminated name.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        assert!(!event.is_null());
        {
            let _cancel = Platform::open(Some(name.clone())).unwrap();
            assert!(!signalled());
            // SAFETY: live event handle.
            unsafe { SetEvent(event) };
            assert!(signalled());
        }
        assert!(!signalled(), "dropping the guard forgets the event");
        // SAFETY: live event handle, closed once.
        unsafe { CloseHandle(event) };
        let missing = format!("{CANCEL_EVENT_PREFIX}missing-{}", std::process::id());
        assert_eq!(
            Platform::open(Some(missing)).err().unwrap().code,
            "internal_error"
        );
        let other = Platform::open(Some("Global\\other".into()));
        assert_eq!(other.err().unwrap().code, "invalid_params");
        let none = Platform::open(None).unwrap();
        assert!(!signalled(), "without the variable nothing cancels");
        drop(none);
    }
}
