//! Volumes for the suite's engines.
//!
//! rust-engine.md §6 · origin of the Windows half: curado `src-tauri/src/identity.rs` and `relink.rs`
//! (volume serial through `GetVolumeInformationW`, space available to the user through `GetDiskFreeSpaceExW`,
//! drive letters). The macOS half uses statfs, statvfs and diskutil.
//!
//! Windows identity (new: Python has no Windows backend; outside macOS it falls back to a `local` mode with no
//! physical identity). The same guarantees as the Mac one:
//! - a card keeps its identity under another letter: the volume GUID path and the filesystem serial, never the
//!   letter or the label;
//! - two volumes on the same physical disk are never two replicas: `physical_disks` comes from the volume's
//!   disk extents (`IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`), like `APFSPhysicalStores` on Mac;
//! - external, removable and disk image come from the device (`IOCTL_STORAGE_QUERY_PROPERTY`), not from a name.
//!
//! Device policy and writer exclusion are handled by the caller. Windows volume metadata needs no administrator.

pub mod fs;
#[cfg(target_os = "macos")]
pub mod macos;

use nu9ve_json::{Error, Result};
use std::path::{Path, PathBuf};

/// What kind of device a root is on (Windows `GetDriveTypeW`). Many USB disks report `Fixed`, so a role is never
/// inferred from this alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    Removable,
    Fixed,
    Network,
    Optical,
    Ram,
    Unknown,
}

impl DriveKind {
    pub fn id(self) -> &'static str {
        match self {
            DriveKind::Removable => "removable",
            DriveKind::Fixed => "fixed",
            DriveKind::Network => "network",
            DriveKind::Optical => "optical",
            DriveKind::Ram => "ram",
            DriveKind::Unknown => "unknown",
        }
    }
}

/// Bus of the device under a volume (`STORAGE_BUS_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    Usb,
    Sd,
    Mmc,
    Ieee1394,
    Sata,
    Nvme,
    Virtual,
    FileBackedVirtual,
    Other(u32),
    Unknown,
}

impl Bus {
    pub fn from_raw(value: u32) -> Self {
        match value {
            4 => Bus::Ieee1394,
            7 => Bus::Usb,
            11 => Bus::Sata,
            12 => Bus::Sd,
            13 => Bus::Mmc,
            14 => Bus::Virtual,
            15 => Bus::FileBackedVirtual,
            17 => Bus::Nvme,
            other => Bus::Other(other),
        }
    }

    pub fn id(self) -> String {
        match self {
            Bus::Usb => "usb".into(),
            Bus::Sd => "sd".into(),
            Bus::Mmc => "mmc".into(),
            Bus::Ieee1394 => "1394".into(),
            Bus::Sata => "sata".into(),
            Bus::Nvme => "nvme".into(),
            Bus::Virtual => "virtual".into(),
            Bus::FileBackedVirtual => "file_backed_virtual".into(),
            Bus::Other(n) => format!("other:{n}"),
            Bus::Unknown => "unknown".into(),
        }
    }

    /// A bus a person plugs a card or a disk into.
    pub fn is_external(self) -> bool {
        matches!(self, Bus::Usb | Bus::Sd | Bus::Mmc | Bus::Ieee1394)
    }

    /// A mounted image (VHD/VHDX/ISO): its bytes live on another disk.
    pub fn is_image(self) -> bool {
        matches!(self, Bus::Virtual | Bus::FileBackedVirtual)
    }
}

/// Identity and size of the volume that holds a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    /// Drive root, letter in uppercase (`E:\`).
    pub root: PathBuf,
    /// `\\?\Volume{GUID}\`: Windows' own volume identity, independent of the letter.
    pub volume_guid: String,
    /// Filesystem serial as 8 hex digits.
    pub serial: String,
    pub serial_number: u32,
    pub label: String,
    pub filesystem: String,
    pub kind: DriveKind,
    pub bus: Bus,
    /// The device reports removable media (an SD slot, a card reader).
    pub removable_media: bool,
    /// `PhysicalDriveN` for every disk that holds the volume; empty when Windows does not say.
    pub physical_disks: Vec<String>,
    /// File index of the root folder (`st_ino` on Mac): changes if another volume takes the letter.
    pub root_index: u64,
    /// Allocation unit in bytes.
    pub block_size: u64,
    /// Bytes available to this user (quotas included), total free bytes and total size.
    pub free: u64,
    pub total_free: u64,
    pub total: u64,
}

impl VolumeInfo {
    pub fn external(&self) -> bool {
        self.bus.is_external() || self.kind == DriveKind::Removable
    }

    pub fn removable(&self) -> bool {
        self.removable_media || self.kind == DriveKind::Removable
    }

    pub fn disk_image(&self) -> bool {
        self.bus.is_image()
    }
}

/// `C:\` → true; `C:`, UNC paths and relative paths → false (the same rule curado tests).
pub fn is_drive_root(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    bytes.len() == 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// The drive root that contains a path (`d:\DCIM\100MEDIA` → `D:\`), letter in uppercase.
pub fn drive_root_of(path: &Path) -> Option<PathBuf> {
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        .then(|| PathBuf::from(format!("{}:\\", text[..1].to_ascii_uppercase())))
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, GetDiskFreeSpaceExW, GetDiskFreeSpaceW,
        GetDriveTypeW, GetFileInformationByHandle, GetLogicalDrives, GetVolumeInformationW,
        GetVolumeNameForVolumeMountPointW,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x0056_0000;

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn text(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }

    struct Handle(HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateFileW and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    fn open(path: &str, flags: u32) -> Option<Handle> {
        let path = wide(path);
        // SAFETY: valid NUL-terminated path; no access rights requested, so no administrator is needed.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                0,
                FILE_SHARE_ALL,
                std::ptr::null(),
                OPEN_EXISTING,
                flags,
                std::ptr::null_mut(),
            )
        };
        (handle != INVALID_HANDLE_VALUE).then_some(Handle(handle))
    }

    fn ioctl(handle: &Handle, code: u32, input: &[u8], output: &mut [u8]) -> Option<usize> {
        let mut returned = 0u32;
        // SAFETY: both buffers are valid for the lengths passed; synchronous call (no OVERLAPPED).
        let ok = unsafe {
            DeviceIoControl(
                handle.0,
                code,
                input.as_ptr() as *const c_void,
                input.len() as u32,
                output.as_mut_ptr() as *mut c_void,
                output.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(returned as usize)
    }

    fn u32_at(buffer: &[u8], offset: usize) -> u32 {
        buffer
            .get(offset..offset + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0)
    }

    /// STORAGE_DEVICE_DESCRIPTOR: RemovableMedia at byte 10, BusType at byte 28.
    fn device(letter: char) -> (bool, Bus) {
        let Some(handle) = open(&format!("\\\\.\\{letter}:"), 0) else {
            return (false, Bus::Unknown);
        };
        // STORAGE_PROPERTY_QUERY { StorageDeviceProperty = 0, PropertyStandardQuery = 0, [0] }.
        let query = [0u8; 12];
        let mut out = [0u8; 1024];
        match ioctl(&handle, IOCTL_STORAGE_QUERY_PROPERTY, &query, &mut out) {
            Some(n) if n >= 32 => (out[10] != 0, Bus::from_raw(u32_at(&out, 28))),
            _ => (false, Bus::Unknown),
        }
    }

    /// VOLUME_DISK_EXTENTS: count at 0, then DISK_EXTENT { DiskNumber u32, pad, i64, i64 } from byte 8, 24 each.
    fn physical_disks(letter: char) -> Vec<String> {
        let Some(handle) = open(&format!("\\\\.\\{letter}:"), 0) else {
            return Vec::new();
        };
        let mut out = [0u8; 8 + 24 * 16];
        let Some(n) = ioctl(&handle, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, &[], &mut out) else {
            return Vec::new();
        };
        let count = (u32_at(&out, 0) as usize).min(16);
        let mut disks: Vec<String> = (0..count)
            .filter(|i| 8 + i * 24 + 4 <= n)
            .map(|i| format!("PhysicalDrive{}", u32_at(&out, 8 + i * 24)))
            .collect();
        disks.sort();
        disks.dedup();
        disks
    }

    fn root_index(root: &str) -> u64 {
        let Some(handle) = open(root, FILE_FLAG_BACKUP_SEMANTICS) else {
            return 0;
        };
        // SAFETY: zeroed plain-data struct filled by the call.
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: valid handle and out pointer.
        if unsafe { GetFileInformationByHandle(handle.0, &mut info) } == 0 {
            return 0;
        }
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)
    }

    pub fn describe(path: &Path) -> Result<VolumeInfo> {
        let root = drive_root_of(path).ok_or_else(|| {
            Error::new(
                "unsupported_path",
                format!("No drive letter in {}", path.display()),
            )
        })?;
        let root_text = root.to_string_lossy().into_owned();
        let letter = root_text.chars().next().unwrap_or('C');
        let root_w = wide(&root_text);
        let offline = |what: &str| {
            Error::new(
                "volume_offline",
                format!("{what} {root_text}: {}", std::io::Error::last_os_error()),
            )
        };
        let mut label = [0u16; 261];
        let mut filesystem = [0u16; 261];
        let mut serial = 0u32;
        // SAFETY: every pointer is valid for the call and the buffers' lengths are passed with them.
        let ok = unsafe {
            GetVolumeInformationW(
                root_w.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                filesystem.as_mut_ptr(),
                filesystem.len() as u32,
            )
        };
        if ok == 0 {
            return Err(offline("No volume at"));
        }
        let mut guid = [0u16; 64];
        // SAFETY: valid NUL-terminated mount point (with trailing backslash) and buffer length.
        let ok = unsafe {
            GetVolumeNameForVolumeMountPointW(root_w.as_ptr(), guid.as_mut_ptr(), guid.len() as u32)
        };
        if ok == 0 {
            return Err(Error::new(
                "volume_identity_unavailable",
                format!("Windows gave no volume GUID for {root_text}"),
            ));
        }
        let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
        // SAFETY: valid root and out pointers.
        if unsafe { GetDiskFreeSpaceExW(root_w.as_ptr(), &mut free, &mut total, &mut total_free) }
            == 0
        {
            return Err(offline("No free space for"));
        }
        let (mut sectors, mut bytes, mut free_clusters, mut clusters) = (0u32, 0u32, 0u32, 0u32);
        // SAFETY: valid root and out pointers.
        let block_size = if unsafe {
            GetDiskFreeSpaceW(
                root_w.as_ptr(),
                &mut sectors,
                &mut bytes,
                &mut free_clusters,
                &mut clusters,
            )
        } != 0
        {
            u64::from(sectors) * u64::from(bytes)
        } else {
            4096
        };
        // SAFETY: valid NUL-terminated root.
        let kind = match unsafe { GetDriveTypeW(root_w.as_ptr()) } {
            2 => DriveKind::Removable,
            3 => DriveKind::Fixed,
            4 => DriveKind::Network,
            5 => DriveKind::Optical,
            6 => DriveKind::Ram,
            _ => DriveKind::Unknown,
        };
        let (removable_media, bus) = device(letter);
        Ok(VolumeInfo {
            root,
            volume_guid: text(&guid),
            serial: format!("{serial:08X}"),
            serial_number: serial,
            label: text(&label),
            filesystem: text(&filesystem),
            kind,
            bus,
            removable_media,
            physical_disks: physical_disks(letter),
            root_index: root_index(&root_text),
            block_size,
            free,
            total_free,
            total,
        })
    }

    pub fn mounted_roots() -> Vec<PathBuf> {
        // SAFETY: no arguments; returns a bit mask of drive letters.
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| PathBuf::from(format!("{}:\\", (b'A' + i) as char)))
            .collect()
    }

    pub fn drive_kind(root: &Path) -> DriveKind {
        let root_w = wide(&root.to_string_lossy());
        // SAFETY: valid NUL-terminated root.
        match unsafe { GetDriveTypeW(root_w.as_ptr()) } {
            2 => DriveKind::Removable,
            3 => DriveKind::Fixed,
            4 => DriveKind::Network,
            5 => DriveKind::Optical,
            6 => DriveKind::Ram,
            _ => DriveKind::Unknown,
        }
    }
}

/// Identity, label, filesystem, bus, physical disks and space of the volume holding `path`.
pub fn describe(path: &Path) -> Result<VolumeInfo> {
    #[cfg(windows)]
    {
        windows::describe(path)
    }
    #[cfg(not(windows))]
    {
        Err(Error::new(
            "not_ported",
            format!(
                "Volume identity for {} is still provided by media/volumes.py on this platform",
                path.display()
            ),
        ))
    }
}

/// Mounted drive roots (every letter). Callers skip network and optical drives before describing them, and never
/// treat a registered destination as a card (media/volumes.py `reject_destination`).
pub fn mounted_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        windows::mounted_roots()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Drive type without opening the volume (cheap: no I/O to a slow network share or an empty optical drive).
pub fn drive_kind(root: &Path) -> DriveKind {
    #[cfg(windows)]
    {
        windows::drive_kind(root)
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        DriveKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_roots_use_uppercase_letters_and_refuse_other_shapes() {
        assert!(is_drive_root(Path::new("D:\\")));
        assert!(is_drive_root(Path::new("e:/")));
        assert!(!is_drive_root(Path::new("C:")));
        assert!(!is_drive_root(Path::new("\\\\server\\share\\")));
        assert!(!is_drive_root(Path::new("DCIM")));
        assert_eq!(
            drive_root_of(Path::new("f:\\DCIM\\100MEDIA")),
            Some(PathBuf::from("F:\\"))
        );
        assert_eq!(drive_root_of(Path::new("relative\\path")), None);
    }

    #[test]
    fn buses_say_what_is_external_and_what_is_an_image() {
        assert!(Bus::from_raw(7).is_external() && Bus::from_raw(12).is_external());
        assert!(!Bus::from_raw(17).is_external() && !Bus::from_raw(11).is_external());
        assert!(Bus::from_raw(15).is_image() && Bus::from_raw(14).is_image());
        assert_eq!(Bus::from_raw(99).id(), "other:99");
    }

    // Read-only and limited to the temporary folder's own volume (the system disk): no personal card is touched.
    #[cfg(windows)]
    #[test]
    fn the_temporary_folder_has_a_full_identity() {
        let folder = tempfile::tempdir().unwrap();
        let info = describe(folder.path()).unwrap();
        assert!(
            info.volume_guid.starts_with("\\\\?\\Volume{"),
            "{}",
            info.volume_guid
        );
        assert_eq!(info.serial.len(), 8);
        assert!(info.total > 0 && info.free <= info.total && info.total_free <= info.total);
        assert!(info.block_size >= 512);
        assert!(
            !info.physical_disks.is_empty(),
            "the system volume sits on a physical disk"
        );
        assert!(!info.disk_image());
        assert!(mounted_roots().contains(&info.root));
        assert_eq!(drive_kind(&info.root), info.kind);
        // Same answer twice: the identity is stable within a session.
        assert_eq!(describe(&info.root).unwrap().volume_guid, info.volume_guid);
    }
}

/// What a directory entry is, without following it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    /// A symlink, junction or any other reparse point: never followed, never ingested.
    Link,
    Other,
}

/// Python's `content_stamp(os.stat(..., follow_symlinks=False))`: size, modification and change times in
/// nanoseconds since 1970, and the inode (on Windows, the file index). On Windows `ctime_ns` is the NTFS/exFAT
/// change time, the closest to POSIX ctime (Python's `st_ctime` on Windows is the creation time, but Python's
/// planner does not run on Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub kind: EntryKind,
    pub size: u64,
    pub mtime_ns: i64,
    pub ctime_ns: i64,
    pub inode: u64,
}

/// A NUL-terminated wide path for raw Win32 calls, in the verbatim form (`\\?\C:\…`, `\\?\UNC\…`) so paths
/// longer than 260 characters work as they do through `std::fs`. Relative paths are left as they are.
#[cfg(windows)]
pub fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let text = path.to_string_lossy().replace('/', r"\");
    let verbatim = if text.starts_with(r"\\?\") || text.starts_with(r"\\.\") {
        text
    } else if let Some(share) = text.strip_prefix(r"\\") {
        format!(r"\\?\UNC\{share}")
    } else if text.len() >= 3 && text.as_bytes()[1] == b':' && text.as_bytes()[2] == b'\\' {
        format!(r"\\?\{text}")
    } else {
        return path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
    };
    std::ffi::OsStr::new(&verbatim)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
pub fn stamp(path: &Path) -> Result<Stamp> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_BASIC_INFO, FileBasicInfo,
        GetFileInformationByHandle, GetFileInformationByHandleEx,
    };
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const SHARE_ALL: u32 = 0x7;
    const OPEN_EXISTING: u32 = 3;
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const EPOCH_DIFF: i64 = 116_444_736_000_000_000; // 1601 → 1970 in 100 ns
    let wide = wide_path(path);
    // SAFETY: valid NUL-terminated path; only attributes are read and the handle never follows a reparse point.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            SHARE_ALL,
            std::ptr::null(),
            OPEN_EXISTING,
            BACKUP_SEMANTICS | OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::new(
            "operation_failed",
            format!("{}: {}", path.display(), std::io::Error::last_os_error()),
        ));
    }
    // SAFETY: zeroed plain-data structs filled by the calls; the handle is closed once below.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let mut basic: FILE_BASIC_INFO = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetFileInformationByHandle(handle, &mut info) } != 0
        && unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileBasicInfo,
                &mut basic as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<FILE_BASIC_INFO>() as u32,
            )
        } != 0;
    let error = std::io::Error::last_os_error();
    // SAFETY: handle from CreateFileW, closed once.
    unsafe { CloseHandle(handle) };
    if !ok {
        return Err(Error::new(
            "operation_failed",
            format!("{}: {error}", path.display()),
        ));
    }
    let attributes = info.dwFileAttributes;
    let kind = if attributes & 0x400 != 0 {
        EntryKind::Link
    } else if attributes & 0x10 != 0 {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let filetime = |high: u32, low: u32| (i64::from(high) << 32) | i64::from(low);
    let ns = |ticks: i64| (ticks - EPOCH_DIFF) * 100;
    Ok(Stamp {
        kind,
        size: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
        mtime_ns: ns(filetime(
            info.ftLastWriteTime.dwHighDateTime,
            info.ftLastWriteTime.dwLowDateTime,
        )),
        ctime_ns: ns(basic.ChangeTime),
        inode: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

#[cfg(unix)]
pub fn stamp(path: &Path) -> Result<Stamp> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| Error::new("operation_failed", format!("{}: {e}", path.display())))?;
    let file_type = meta.file_type();
    let kind = if file_type.is_symlink() {
        EntryKind::Link
    } else if file_type.is_dir() {
        EntryKind::Dir
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    Ok(Stamp {
        kind,
        size: meta.size(),
        mtime_ns: meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
        ctime_ns: meta.ctime() * 1_000_000_000 + meta.ctime_nsec(),
        inode: meta.ino(),
    })
}

#[cfg(test)]
mod stamp_tests {
    use super::*;

    #[test]
    fn stamps_files_and_folders_without_following_anything() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("C0001.MP4");
        std::fs::write(&file, b"twelve bytes").unwrap();
        let s = stamp(&file).unwrap();
        assert_eq!((s.kind, s.size), (EntryKind::File, 12));
        assert!(s.inode != 0);
        // Within a minute of now, in nanoseconds since 1970.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        assert!(
            (now - s.mtime_ns).abs() < 60_000_000_000,
            "{} vs {now}",
            s.mtime_ns
        );
        assert!(s.ctime_ns > 0);
        assert_eq!(stamp(folder.path()).unwrap().kind, EntryKind::Dir);
        assert_eq!(
            stamp(&file).unwrap(),
            s,
            "a stamp does not change by reading it"
        );
    }
}
