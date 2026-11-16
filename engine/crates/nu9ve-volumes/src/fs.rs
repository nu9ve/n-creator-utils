//! Volume stamps and regular-file validation, with durable I/O from nu9ve-fs.

use crate::{EntryKind, Stamp, stamp};
use nu9ve_json::{Error, Result};
use std::fs::File;
use std::io;
use std::path::Path;

use nu9ve_fs::SyncMode;
pub use nu9ve_fs::{Cache, VerifyFile, uncached, verify_read};

/// I/O errors carry Python's exception name so a job records what Python would (`FileNotFoundError`, …).
pub fn io_error(error: std::io::Error, context: &Path) -> Error {
    let name = match error.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::AlreadyExists => "FileExistsError",
        _ => "OSError",
    };
    Error::new(
        "internal_error",
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

/// Full synchronization for media and metadata.
pub fn full_sync(file: &File) -> Result<()> {
    nu9ve_fs::sync_file(file, SyncMode::Full)
        .map_err(|e| Error::new("internal_error", format!("[OSError] fsync: {e}")))
}

/// Make the entries of `dir` durable.
pub fn sync_dir(dir: &Path) -> Result<()> {
    nu9ve_fs::sync_dir(dir, SyncMode::Full).map_err(|e| io_error(e, dir))
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
    nu9ve_fs::create_new(path).map_err(|e| io_error(e, path))
}

/// Rename without replacement, under the writer lock for filesystems such as exFAT.
pub fn rename_new(from: &Path, to: &Path) -> Result<()> {
    nu9ve_fs::rename_new_locked(from, to).map_err(|error| match error.kind() {
        io::ErrorKind::AlreadyExists => Error::new(
            "destination_conflict",
            format!("A file appeared at {}", to.display()),
        ),
        #[cfg(unix)]
        io::ErrorKind::InvalidInput if error.raw_os_error().is_none() => {
            Error::new("unsafe_path", "NUL in path")
        }
        _ => io_error(error, to),
    })
}

/// Replace `to` with `from` (owned metadata such as receipts): Python `os.replace`.
pub fn replace(from: &Path, to: &Path) -> Result<()> {
    nu9ve_fs::replace(from, to).map_err(|e| io_error(e, to))
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
    use std::io::{Read, Seek, SeekFrom};

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
