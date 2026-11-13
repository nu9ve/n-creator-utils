//! Cooperative SIGINT for macOS one-shot workers.

use nu9ve_json::{Error, Result};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

extern "C" fn interrupt(_: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

pub(crate) struct Cancel(libc::sigaction);

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
                    "operation_failed",
                    io::Error::last_os_error().to_string(),
                ));
            }
            Ok(Self(old.assume_init()))
        }
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        unsafe {
            libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut());
        }
    }
}

pub(crate) fn cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}
