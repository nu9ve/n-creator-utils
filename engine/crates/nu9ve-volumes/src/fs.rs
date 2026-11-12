//! Durable, no-follow file operations for writers that move media (Python `media/common.py`: `full_sync`,
//! `uncached`, the `Tree` writes; `os.rename` without replacing).
//!
//! - `full_sync`: macOS `fsync` does not flush the drive cache, `F_FULLFSYNC` does (as Python); elsewhere `fsync`.
//! - `sync_dir`: the directory entry of a create/rename/unlink is durable too. On Windows a directory handle with
//!   backup semantics is flushed (NTFS honours it; exFAT/FAT answer an error, ignored as Python ignores an
//!   unsupported `F_FULLFSYNC` and falls back).
//! - `rename_new`: a rename that never replaces: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows,
//!   `renamex_np(RENAME_EXCL)` on macOS (exFAT/FAT refuse it with ENOTSUP: then check first, as Python); other
//!   Unix check first (under the writer lock, as Python).
//! - `open_read`/`create_new`: never through a link (`O_NOFOLLOW`; `FILE_FLAG_OPEN_REPARSE_POINT`), regular files
//!   only. `uncached` (macOS `F_NOCACHE`) keeps a copy's reads and writes out of the OS cache.
//! - `verify_read`: the read that proves a copy (a partial before its rename, an adopted destination, the original
//!   before cleanup) must come from the device, not from pages just written. macOS: `F_NOCACHE`, as Python.
//!   Windows: the same validated handle reopened with `FILE_FLAG_NO_BUFFERING` (`ReOpenFile`), read in
//!   sector-aligned blocks. Linux: `posix_fadvise(DONTNEED)` drops the clean pages first. When the platform or
//!   the filesystem refuses, the read goes through the cache and says so (`Cache::Os`); it never pretends.

use crate::{EntryKind, Stamp, stamp};
use nu9ve_json::{Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
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

/// How a verification read reached its bytes (recorded only when it is not `Bypassed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cache {
    /// The OS cache was skipped: `FILE_FLAG_NO_BUFFERING` on Windows, `F_NOCACHE` on macOS.
    Bypassed,
    /// The file's clean pages were dropped before reading (Linux `posix_fadvise(DONTNEED)`).
    Dropped,
    /// The read may have been served by the OS cache: the platform or the filesystem refused the others.
    Os,
}

impl Cache {
    pub fn as_str(self) -> &'static str {
        match self {
            Cache::Bypassed => "bypassed",
            Cache::Dropped => "dropped",
            Cache::Os => "os",
        }
    }
}

/// Unbuffered reads need a buffer, offset and length aligned to the volume's sector size. 4 KiB covers 512-byte
/// and 4K-native disks; the block is a multiple of it.
#[cfg(windows)]
const SECTOR_ALIGN: usize = 4096;
#[cfg(windows)]
const VERIFY_BLOCK: usize = 4 * 1024 * 1024;

/// A read-only view of an already opened (and validated) regular file for verification. `Read` + `Seek` to the
/// start; the bytes are the file's, only the path to them changes.
pub struct VerifyFile {
    file: File,
    cache: Cache,
    #[cfg(windows)]
    direct: Option<Direct>,
}

#[cfg(windows)]
struct Direct {
    file: File,
    storage: Vec<u8>,
    offset: usize,
    start: usize,
    end: usize,
    eof: bool,
    read: u64,
    first: bool,
}

/// Wrap `file` (from `open_read`) for a verification read; see the module notes.
pub fn verify_read(file: File) -> VerifyFile {
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid open descriptor.
        let ok = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) } != -1;
        let cache = if ok { Cache::Bypassed } else { Cache::Os };
        VerifyFile { file, cache }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: a valid open descriptor; advice only, the file is unchanged.
            let ok =
                unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) }
                    == 0;
            let cache = if ok { Cache::Dropped } else { Cache::Os };
            VerifyFile { file, cache }
        }
        #[cfg(not(target_os = "linux"))]
        {
            VerifyFile {
                file,
                cache: Cache::Os,
            }
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        use windows_sys::Win32::Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_NO_BUFFERING, FILE_FLAG_SEQUENTIAL_SCAN, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, ReOpenFile,
        };
        // SAFETY: a valid handle; the reopened handle names the same file object, so no link is followed.
        let handle = unsafe {
            ReOpenFile(
                file.as_raw_handle(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN,
            )
        };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return VerifyFile {
                file,
                cache: Cache::Os,
                direct: None,
            };
        }
        // SAFETY: a fresh handle owned by the new File and closed by it.
        let direct = unsafe { File::from_raw_handle(handle) };
        let storage = vec![0u8; VERIFY_BLOCK + SECTOR_ALIGN];
        let offset = storage.as_ptr().align_offset(SECTOR_ALIGN);
        VerifyFile {
            file,
            cache: Cache::Bypassed,
            direct: Some(Direct {
                file: direct,
                storage,
                offset,
                start: 0,
                end: 0,
                eof: false,
                read: 0,
                first: true,
            }),
        }
    }
}

impl VerifyFile {
    /// How the bytes were read so far (`Os` once an unbuffered read was refused).
    pub fn cache(&self) -> Cache {
        self.cache
    }

    pub fn metadata(&self) -> std::io::Result<std::fs::Metadata> {
        self.file.metadata()
    }
}

impl Read for VerifyFile {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        #[cfg(windows)]
        if let Some(direct) = self.direct.as_mut() {
            if direct.start == direct.end {
                if direct.eof {
                    return Ok(0);
                }
                let offset = direct.offset;
                match direct
                    .file
                    .read(&mut direct.storage[offset..offset + VERIFY_BLOCK])
                {
                    Ok(n) => {
                        direct.first = false;
                        direct.read += n as u64;
                        // Unbuffered reads end short only at the end of the file; a later read would be unaligned.
                        if n < VERIFY_BLOCK {
                            direct.eof = true;
                            let size = self.file.metadata()?.len();
                            if direct.read != size {
                                return Err(std::io::Error::other(format!(
                                    "Unbuffered read ended at {} of {size} bytes",
                                    direct.read
                                )));
                            }
                        }
                        direct.start = direct.offset;
                        direct.end = direct.offset + n;
                        if n == 0 {
                            return Ok(0);
                        }
                    }
                    // ERROR_INVALID_PARAMETER on the first read: this filesystem refuses unbuffered reads.
                    Err(e) if direct.first && e.raw_os_error() == Some(87) => {
                        self.direct = None;
                        self.cache = Cache::Os;
                        self.file.seek(SeekFrom::Start(0))?;
                        return self.file.read(out);
                    }
                    Err(e) => return Err(e),
                }
            }
            let n = out.len().min(direct.end - direct.start);
            out[..n].copy_from_slice(&direct.storage[direct.start..direct.start + n]);
            direct.start += n;
            return Ok(n);
        }
        self.file.read(out)
    }
}

impl Seek for VerifyFile {
    /// Only back to the start: a verification always hashes the whole file.
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        if pos != SeekFrom::Start(0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "A verification read only rewinds",
            ));
        }
        #[cfg(windows)]
        if let Some(direct) = self.direct.as_mut() {
            direct.file.seek(pos)?;
            direct.start = 0;
            direct.end = 0;
            direct.eof = false;
            direct.read = 0;
        }
        self.file.seek(pos)
    }
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
            // exFAT and FAT do not implement RENAME_EXCL (ENOTSUP, found with a test exFAT disk): check, then
            // rename, as Python does on every filesystem, under the same writer lock.
            if error.raw_os_error() == Some(libc::ENOTSUP) {
                if stamp_opt(to)?.is_some() {
                    return Err(Error::new(
                        "destination_conflict",
                        format!("A file appeared at {}", to.display()),
                    ));
                }
                return std::fs::rename(from, to).map_err(|e| io_error(e, to));
            }
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

#[cfg(test)]
mod verify {
    use super::*;

    /// Deterministic, position-dependent bytes so a misplaced block changes the content.
    fn bytes(size: usize) -> Vec<u8> {
        (0..size)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect()
    }

    fn read_all(file: &mut VerifyFile) -> Vec<u8> {
        let mut out = Vec::new();
        // Odd-sized reads cross the aligned blocks inside.
        let mut chunk = vec![0u8; 1_000_003];
        loop {
            let n = file.read(&mut chunk).unwrap();
            if n == 0 {
                return out;
            }
            out.extend_from_slice(&chunk[..n]);
        }
    }

    #[test]
    fn verification_reads_return_the_files_bytes_at_every_size() {
        let dir = tempfile::tempdir().unwrap();
        let block = 4 * 1024 * 1024;
        let sizes = [
            0,
            1,
            511,
            512,
            513,
            4095,
            4096,
            4097,
            block - 1,
            block,
            block + 1,
            9 * 1024 * 1024 + 7,
        ];
        for size in sizes {
            let path = dir.path().join(format!("f{size}"));
            let data = bytes(size);
            std::fs::write(&path, &data).unwrap();
            let mut file = verify_read(open_read(&path).unwrap().unwrap());
            assert_eq!(read_all(&mut file), data, "size {size}");
            // Rewinding reads it again from the start (hash_file seeks first).
            file.seek(SeekFrom::Start(0)).unwrap();
            assert_eq!(read_all(&mut file), data, "size {size} after rewind");
            assert!(file.seek(SeekFrom::Start(1)).is_err());
            assert_eq!(file.metadata().unwrap().len(), size as u64);
            // A local NTFS or APFS volume accepts it; the cache itself cannot be observed from a test.
            #[cfg(any(windows, target_os = "macos"))]
            assert_eq!(file.cache(), Cache::Bypassed, "size {size}");
        }
    }

    #[test]
    fn a_same_size_change_is_seen_after_the_first_verification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial");
        let data = bytes(5 * 1024 * 1024 + 3);
        std::fs::write(&path, &data).unwrap();
        let first = read_all(&mut verify_read(open_read(&path).unwrap().unwrap()));
        let mut changed = data.clone();
        changed[4 * 1024 * 1024 + 1] ^= 0xff;
        std::fs::write(&path, &changed).unwrap();
        let second = read_all(&mut verify_read(open_read(&path).unwrap().unwrap()));
        assert_eq!(first, data);
        assert_eq!(second, changed);
        assert_eq!(Cache::Os.as_str(), "os");
        assert!(Cache::Bypassed < Cache::Dropped && Cache::Dropped < Cache::Os);
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
        let verify = verify_read(open_read(&b).unwrap().unwrap());
        assert_eq!(verify.cache(), Cache::Bypassed);
    }
}
