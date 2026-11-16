use crate::{SyncMode, parent, sync, sync_file, temporary_sibling};
use std::fs::{self, File};
use std::io;
use std::path::Path;

/// How exclusive publication affected the source name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    Linked,
    Moved,
    Copied,
}

/// Rename without replacing. Hold the writer lock shared by all publishers:
/// filesystems without exclusive rename (including exFAT on macOS) need a
/// check followed by rename, which protects only cooperating writers.
pub fn rename_new_locked(from: &Path, to: &Path) -> io::Result<()> {
    match rename_exclusive(from, to) {
        Err(error) if exclusive_unsupported(&error) => {
            match fs::symlink_metadata(to) {
                Ok(_) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            fs::rename(from, to)
        }
        result => result,
    }
}

/// Publish an already synced regular file and sync the target's directory.
/// Hold the writer lock for the exFAT fallback in `rename_new_locked`.
/// On a cross-volume move, stage and sync a copy next to the target. The
/// original remains in place for the caller to remove after a successful publish.
pub fn publish_new_locked(from: &Path, to: &Path, mode: SyncMode) -> io::Result<Placement> {
    // An exclusive rename provides the write-through option on Windows.
    #[cfg(unix)]
    match fs::hard_link(from, to) {
        Ok(()) => {
            sync::sync_publication(parent(to), mode)?;
            return Ok(Placement::Linked);
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => {} // Try exclusive rename on filesystems without hard links.
    }
    match rename_new_locked(from, to) {
        Ok(()) => {
            sync::sync_publication(parent(to), mode)?;
            if parent(from) != parent(to) {
                sync::sync_publication(parent(from), mode)?;
            }
            Ok(Placement::Moved)
        }
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            copy_new_locked(from, to, mode)
        }
        Err(error) => Err(error),
    }
}

fn copy_new_locked(from: &Path, to: &Path, mode: SyncMode) -> io::Result<Placement> {
    let mut source = File::open(from)?;
    let (temporary, mut staged) = temporary_sibling(to)?;
    let copied = io::copy(&mut source, &mut staged).and_then(|_| sync_file(&staged, mode));
    drop(staged);
    if let Err(error) = copied.and_then(|()| rename_new_locked(&temporary, to)) {
        let _ = fs::remove_file(temporary);
        return Err(error);
    }
    sync::sync_publication(parent(to), mode)?;
    Ok(Placement::Copied)
}

/// Rename and replace an existing file. Windows requests a write-through rename.
pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::rename(from, to)
    }
    #[cfg(windows)]
    {
        move_file(from, to, true)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = |path: &Path| {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))
    };
    let (from, to) = (c_path(from)?, c_path(to)?);
    // SAFETY: both paths are valid C strings for the duration of the call.
    #[cfg(target_os = "macos")]
    let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    // SAFETY: both paths are valid C strings for the duration of the call.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn rename_exclusive(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(windows)]
fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    move_file(from, to, false)
}

fn exclusive_unsupported(error: &io::Error) -> bool {
    #[cfg(target_os = "macos")]
    {
        error.raw_os_error() == Some(libc::ENOTSUP)
    }
    #[cfg(target_os = "linux")]
    {
        matches!(
            error.raw_os_error(),
            Some(libc::ENOSYS | libc::EOPNOTSUPP | libc::EINVAL)
        )
    }
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
    {
        error.kind() == io::ErrorKind::Unsupported
    }
    #[cfg(windows)]
    {
        let _ = error;
        false
    }
}

#[cfg(windows)]
fn move_file(from: &Path, to: &Path, replace: bool) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let (from, to) = (crate::wide_path(from)?, crate::wide_path(to)?);
    let mut flags = MOVEFILE_WRITE_THROUGH;
    if replace {
        flags |= MOVEFILE_REPLACE_EXISTING;
    }
    // SAFETY: both paths are valid NUL-terminated UTF-16 for the call.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atomic_write;

    #[test]
    fn staged_copy_keeps_the_original_and_cleans_up_on_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("original");
        let to = dir.path().join("target");
        atomic_write(&from, b"original").unwrap();
        atomic_write(&to, b"previous").unwrap();
        assert_eq!(
            copy_new_locked(&from, &to, SyncMode::Plain)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&to).unwrap(), b"previous");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);

        fs::remove_file(&to).unwrap();
        assert_eq!(
            copy_new_locked(&from, &to, SyncMode::Plain).unwrap(),
            Placement::Copied
        );
        assert_eq!(fs::read(&to).unwrap(), b"original");
        assert_eq!(fs::read(&from).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
