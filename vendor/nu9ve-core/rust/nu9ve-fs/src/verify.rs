use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

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

/// Wrap an already validated regular file for a verification read.
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
