use std::fs::File;
use std::io;
use std::path::Path;

/// Synchronization strength. Plain matches Unix Python's `os.fsync`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SyncMode {
    #[default]
    Full,
    Plain,
}

pub fn sync_file(file: &File, mode: SyncMode) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    if mode == SyncMode::Full {
        match file.sync_all() {
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(libc::ENOTSUP | libc::EINVAL | libc::ENOTTY)
                ) => {}
            result => return result,
        }
    }
    #[cfg(unix)]
    if mode == SyncMode::Plain || cfg!(target_os = "macos") {
        use std::os::fd::AsRawFd;
        loop {
            // SAFETY: the descriptor stays open for the call.
            if unsafe { libc::fsync(file.as_raw_fd()) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    #[cfg(windows)]
    let _ = mode;
    file.sync_all()
}

/// Synchronize a directory. Failures opening or syncing it are reported.
pub fn sync_dir(dir: &Path, mode: SyncMode) -> io::Result<()> {
    #[cfg(unix)]
    let handle = File::open(dir)?;
    #[cfg(windows)]
    let handle = {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
        File::options()
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)?
    };
    sync_file(&handle, mode)
}

pub(crate) fn sync_publication(dir: &Path, mode: SyncMode) -> io::Result<()> {
    #[cfg(unix)]
    {
        sync_dir(dir, mode)
    }
    #[cfg(windows)]
    {
        // Both Windows publication paths use a write-through rename. Windows
        // offers no equivalent POSIX directory flush for arbitrary filesystems.
        let _ = (dir, mode);
        Ok(())
    }
}
