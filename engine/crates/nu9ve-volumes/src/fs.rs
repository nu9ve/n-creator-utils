//! Durable, no-follow file operations for writers that move media (Python `media/common.py`: `full_sync`,
//! `uncached`, the `Tree` writes; `os.rename` without replacing).
//!
//! - `full_sync`: macOS `fsync` does not flush the drive cache, `F_FULLFSYNC` does (as Python); elsewhere `fsync`.
//! - `sync_dir`: the directory entry of a create/rename/unlink is durable too. On Windows a directory handle with
//!   backup semantics is flushed (NTFS honours it; exFAT/FAT answer an error, ignored as Python ignores an
//!   unsupported `F_FULLFSYNC` and falls back).
//! - `rename_new`: a rename that never replaces: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows,
//!   `renamex_np(RENAME_EXCL)` on macOS; other Unix check first (under the writer lock, as Python).
//! - `open_read`/`create_new`: never through a link (`O_NOFOLLOW`; `FILE_FLAG_OPEN_REPARSE_POINT`), regular files
//!   only. `uncached` (macOS `F_NOCACHE`) keeps a verification read from being served by pages just written.

use crate::{EntryKind, Stamp, stamp};
use nu9ve_json::{Error, Result};
use std::fs::File;
use std::path::Path;

/// I/O errors carry Python's exception name so a job records what Python would (`FileNotFoundError`, …).
pub fn io_error(error: std::io::Error, context: &Path) -> Error {
    let name = match error.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::AlreadyExists => "FileExistsError",
        _ => "OSError",
    };
    Error::new(
        "operation_failed",
        format!("[{name}] {}: {error}", context.display()),
    )
}

/// The Python exception name inside an I/O error message, if any (see `io_error`).
pub fn python_exception(error: &Error) -> Option<&str> {
    let rest = error.message.strip_prefix('[')?;
    rest.split_once(']').map(|(name, _)| name)
}

/// `stamp(path)`, or `None` when nothing is there (`FileNotFoundError`/`NotADirectoryError`).
pub fn stamp_opt(path: &Path) -> Result<Option<Stamp>> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => stamp(path).map(Some),
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::NotADirectory =>
        {
            Ok(None)
        }
        Err(e) => Err(io_error(e, path)),
    }
}

/// `fsync`, with `F_FULLFSYNC` on macOS where the filesystem supports it.
pub fn full_sync(file: &File) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid open descriptor; F_FULLFSYNC takes no argument.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } != -1 {
            return Ok(());
        }
    }
    file.sync_all()
        .map_err(|e| Error::new("operation_failed", format!("[OSError] fsync: {e}")))
}

/// Best effort: keep reads of this file out of the OS cache (macOS `F_NOCACHE`; nothing elsewhere, as Python).
pub fn uncached(file: File) -> File {
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid open descriptor.
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) };
    }
    file
}

/// Make the entries of `dir` durable.
pub fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let handle = File::open(dir).map_err(|e| io_error(e, dir))?;
        full_sync(&handle)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FlushFileBuffers};
        let wide = crate::wide_path(dir);
        // SAFETY: NUL-terminated path; a directory handle (backup semantics) opened only to flush it.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_WRITE,
                0x7,
                std::ptr::null(),
                3,
                0x0200_0000,
                std::ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            // SAFETY: handle from CreateFileW, flushed then closed once. Unsupported filesystems fail; ignored.
            unsafe {
                FlushFileBuffers(handle);
                CloseHandle(handle);
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn no_follow(options: &mut std::fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
}

#[cfg(windows)]
fn no_follow(options: &mut std::fs::OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT: open a link itself, never its target
}

fn regular(file: File, path: &Path) -> Result<File> {
    let meta = file.metadata().map_err(|e| io_error(e, path))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(Error::new(
                "unsafe_file",
                format!("Not a regular file: {}", path.display()),
            ));
        }
    }
    if !meta.is_file() {
        return Err(Error::new(
            "unsafe_file",
            format!("Not a regular file: {}", path.display()),
        ));
    }
    Ok(file)
}

/// Open a regular file for reading without following a link; `None` when absent.
pub fn open_read(path: &Path) -> Result<Option<File>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    no_follow(&mut options);
    match options.open(path) {
        Ok(file) => regular(file, path).map(Some),
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::NotADirectory =>
        {
            Ok(None)
        }
        // ELOOP: the last component is a symlink.
        Err(e) if e.raw_os_error() == Some(40) || e.raw_os_error() == Some(62) => Err(Error::new(
            "unsafe_file",
            format!("Not a regular file: {}", path.display()),
        )),
        // Windows refuses to open a directory as a file: Python opens it and then refuses it as not regular.
        Err(e)
            if stamp_opt(path)
                .ok()
                .flatten()
                .is_some_and(|s| s.kind != EntryKind::File) =>
        {
            let _ = e;
            Err(Error::new(
                "unsafe_file",
                format!("Not a regular file: {}", path.display()),
            ))
        }
        Err(e) => Err(io_error(e, path)),
    }
}

/// Create a new file (`O_EXCL`), never through a link.
pub fn create_new(path: &Path) -> Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    no_follow(&mut options);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|e| io_error(e, path))
}

/// Rename `from` to `to` in the same directory, failing with `destination_conflict` when `to` exists.
pub fn rename_new(from: &Path, to: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
        let (a, b) = (crate::wide_path(from), crate::wide_path(to));
        // SAFETY: two NUL-terminated paths; without MOVEFILE_REPLACE_EXISTING an existing target fails the call.
        if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(80 | 183)) {
                return Err(Error::new(
                    "destination_conflict",
                    format!("A file appeared at {}", to.display()),
                ));
            }
            return Err(io_error(error, to));
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let c = |p: &Path| {
            CString::new(p.as_os_str().as_bytes())
                .map_err(|_| Error::new("unsafe_path", "NUL in path"))
        };
        let (a, b) = (c(from)?, c(to)?);
        // SAFETY: two NUL-terminated paths; RENAME_EXCL fails when the target exists.
        if unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_EXCL) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(Error::new(
                    "destination_conflict",
                    format!("A file appeared at {}", to.display()),
                ));
            }
            return Err(io_error(error, to));
        }
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if stamp_opt(to)?.is_some() {
            return Err(Error::new(
                "destination_conflict",
                format!("A file appeared at {}", to.display()),
            ));
        }
        std::fs::rename(from, to).map_err(|e| io_error(e, to))
    }
}

/// Replace `to` with `from` (owned metadata such as receipts): Python `os.replace`.
pub fn replace(from: &Path, to: &Path) -> Result<()> {
    std::fs::rename(from, to).map_err(|e| io_error(e, to))
}

/// Whether `path` is a directory (not a link to one).
pub fn is_dir(path: &Path) -> Result<Option<bool>> {
    Ok(stamp_opt(path)?.map(|s| {
        if s.kind == EntryKind::Link {
            false
        } else {
            s.kind == EntryKind::Dir
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn renames_never_replace_and_reads_never_follow() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        create_new(&a).unwrap().write_all(b"one").unwrap();
        assert!(
            create_new(&a)
                .unwrap_err()
                .message
                .starts_with("[FileExistsError]")
        );
        std::fs::write(&b, b"two").unwrap();
        assert_eq!(rename_new(&a, &b).unwrap_err().code, "destination_conflict");
        assert_eq!(std::fs::read(&b).unwrap(), b"two");
        let c = dir.path().join("c");
        rename_new(&a, &c).unwrap();
        assert!(open_read(&a).unwrap().is_none());
        assert!(open_read(&c).unwrap().is_some());
        assert_eq!(open_read(dir.path()).unwrap_err().code, "unsafe_file");
        sync_dir(dir.path()).unwrap();
        // Flushing needs write access on Windows; written files are always flushed through their writer.
        full_sync(&File::options().write(true).open(&c).unwrap()).unwrap();
        assert!(stamp_opt(&dir.path().join("x/y")).unwrap().is_none());
        assert_eq!(
            python_exception(&io_error(std::io::ErrorKind::NotFound.into(), &a)),
            Some("FileNotFoundError")
        );
    }
}

#[cfg(all(test, windows))]
mod long_paths {
    use super::*;

    #[test]
    fn paths_longer_than_max_path_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut deep = dir.path().to_path_buf();
        for _ in 0..12 {
            deep.push("carpeta-con-un-nombre-largo");
        }
        std::fs::create_dir_all(&deep).unwrap();
        assert!(deep.to_string_lossy().len() > 300);
        let (a, b) = (
            deep.join("a.ncpartial"),
            deep.join("26NOV04-163212_MAVICAIR2_MX-INTRO[].JPG"),
        );
        create_new(&a).unwrap();
        full_sync(&std::fs::File::options().write(true).open(&a).unwrap()).unwrap();
        rename_new(&a, &b).unwrap();
        sync_dir(&deep).unwrap();
        assert_eq!(stamp(&b).unwrap().kind, EntryKind::File);
        assert!(open_read(&b).unwrap().is_some());
    }
}
