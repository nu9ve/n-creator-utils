//! Durable file operations for the suite (nu9ve-fs-v1).

mod lock;
mod publish;
mod sync;
mod verify;

pub use lock::{LockMode, lock, lock_held};
pub use publish::{Placement, publish_new_locked, rename_new_locked, replace};
pub use sync::{SyncMode, sync_dir, sync_file};
pub use verify::{Cache, VerifyFile, uncached, verify_read};

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Create a new file with mode 0600 on Unix. An existing name, including a link, fails.
pub fn create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Whether an atomic write failed before replacement or during its directory sync.
#[derive(Debug)]
pub enum AtomicWriteError {
    BeforeRename(io::Error),
    AfterRename(io::Error),
}

impl From<AtomicWriteError> for io::Error {
    fn from(error: AtomicWriteError) -> Self {
        match error {
            AtomicWriteError::BeforeRename(error) | AtomicWriteError::AfterRename(error) => error,
        }
    }
}

impl std::fmt::Display for AtomicWriteError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeRename(error) => write!(fmt, "file replacement failed: {error}"),
            Self::AfterRename(error) => {
                write!(fmt, "directory sync failed after replacement: {error}")
            }
        }
    }
}

impl std::error::Error for AtomicWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BeforeRename(error) | Self::AfterRename(error) => Some(error),
        }
    }
}

/// Replace a file with the default, full synchronization.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), AtomicWriteError> {
    atomic_write_with(path, data, SyncMode::default())
}

/// Create parents, write and sync a sibling temporary, then replace and sync its directory.
pub fn atomic_write_with(path: &Path, data: &[u8], mode: SyncMode) -> Result<(), AtomicWriteError> {
    atomic_write_using(path, data, mode, sync::sync_publication)
}

fn atomic_write_using(
    path: &Path,
    data: &[u8],
    mode: SyncMode,
    sync_parent: impl FnOnce(&Path, SyncMode) -> io::Result<()>,
) -> Result<(), AtomicWriteError> {
    let parent = parent(path);
    let (temporary, mut file) = (|| {
        fs::create_dir_all(parent)?;
        temporary_sibling(path)
    })()
    .map_err(AtomicWriteError::BeforeRename)?;
    let written = file.write_all(data).and_then(|()| sync_file(&file, mode));
    drop(file);
    if let Err(error) = written.and_then(|()| replace(&temporary, path)) {
        let _ = fs::remove_file(temporary);
        return Err(AtomicWriteError::BeforeRename(error));
    }
    sync_parent(parent, mode).map_err(AtomicWriteError::AfterRename)
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn temporary_sibling(target: &Path) -> io::Result<(PathBuf, File)> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing file name"))?;
    let mut prefix = OsString::from(".");
    prefix.push(name);
    prefix.push(".");
    for _ in 0..100 {
        let random =
            nu9ve_json::random_bytes::<8>().map_err(|error| io::Error::other(error.message))?;
        let suffix: String = random
            .iter()
            .map(|byte| char::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
            .collect();
        let mut temporary = prefix.clone();
        temporary.push(suffix);
        temporary.push(".tmp");
        let candidate = parent(target).join(temporary);
        match create_new(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

#[cfg(windows)]
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    // Resolve relative paths and components before adding a verbatim prefix.
    let mut wide: Vec<u16> = std::path::absolute(path)?
        .as_os_str()
        .encode_wide()
        .collect();
    if wide.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"));
    }
    let units = |prefix: &str| prefix.encode_utf16().collect::<Vec<_>>();
    if !wide.starts_with(&units(r"\\?\")) && !wide.starts_with(&units(r"\\.\")) {
        for unit in &mut wide {
            if *unit == u16::from(b'/') {
                *unit = u16::from(b'\\');
            }
        }
        if wide.starts_with(&units(r"\\")) {
            wide.splice(..2, units(r"\\?\UNC\"));
        } else {
            wide.splice(..0, units(r"\\?\"));
        }
    }
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests;
