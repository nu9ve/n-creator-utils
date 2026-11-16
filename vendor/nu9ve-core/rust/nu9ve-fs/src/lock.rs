use std::fs::{File, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

/// Acquire a lock on an open file. Contention through the deadline returns
/// `WouldBlock`. A zero timeout performs one attempt. Close the file to release it.
pub fn lock(file: &File, mode: LockMode, timeout: Duration) -> io::Result<()> {
    let started = Instant::now();
    loop {
        let result = match mode {
            LockMode::Shared => file.try_lock_shared(),
            LockMode::Exclusive => file.try_lock(),
        };
        match result {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) => {
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return Err(io::Error::from(io::ErrorKind::WouldBlock));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}

/// Observe whether an existing lock file is held (shared or exclusive).
/// This opens a fresh handle and does not create a missing lock file.
pub fn lock_held(path: &Path) -> io::Result<bool> {
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    options.write(true);
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    match lock(&file, LockMode::Exclusive, Duration::ZERO) {
        Ok(()) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(true),
        Err(error) => Err(error),
    }
}
