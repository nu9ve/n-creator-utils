//! Cooperative cancellation for one-shot workers: the job stops at a recoverable checkpoint and is recorded
//! `paused`, so `resume` continues it. macOS: SIGINT. Windows: a named event the client creates and passes in
//! `NUUN_CANCEL_EVENT` (`Local\nuun-cancel-…`); the client signals it to cancel, as nove signals its host with
//! named events. Without the variable a Windows worker is not cancellable, only stoppable (journal + `resume`).

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
pub(crate) struct Cancel(libc::sigaction);

#[cfg(target_os = "macos")]
impl Cancel {
    // Only install from a dedicated one-shot worker.
    pub(crate) fn install() -> Result<Self> {
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
impl Drop for Cancel {
    fn drop(&mut self) {
        unsafe {
            libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut());
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn cancelled() -> bool {
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
pub(crate) struct Cancel(());

#[cfg(windows)]
impl Cancel {
    // Only install from a dedicated one-shot worker.
    pub(crate) fn install() -> Result<Self> {
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
impl Drop for Cancel {
    fn drop(&mut self) {
        let handle = EVENT.swap(0, Ordering::Relaxed);
        if handle != 0 {
            // SAFETY: the handle came from OpenEventW in `install` and is closed once.
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle as _) };
        }
    }
}

#[cfg(windows)]
pub(crate) fn cancelled() -> bool {
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    let handle = EVENT.load(Ordering::Relaxed);
    // SAFETY: a live event handle with SYNCHRONIZE, polled without waiting.
    handle != 0 && unsafe { WaitForSingleObject(handle as _, 0) } == WAIT_OBJECT_0
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
            let _cancel = Cancel::open(Some(name.clone())).unwrap();
            assert!(!cancelled());
            // SAFETY: live event handle.
            unsafe { SetEvent(event) };
            assert!(cancelled());
        }
        assert!(!cancelled(), "dropping the guard forgets the event");
        // SAFETY: live event handle, closed once.
        unsafe { CloseHandle(event) };
        let missing = format!("{CANCEL_EVENT_PREFIX}missing-{}", std::process::id());
        assert_eq!(
            Cancel::open(Some(missing)).err().unwrap().code,
            "internal_error"
        );
        let other = Cancel::open(Some("Global\\other".into()));
        assert_eq!(other.err().unwrap().code, "invalid_params");
        let none = Cancel::open(None).unwrap();
        assert!(!cancelled(), "without the variable nothing cancels");
        drop(none);
    }
}
