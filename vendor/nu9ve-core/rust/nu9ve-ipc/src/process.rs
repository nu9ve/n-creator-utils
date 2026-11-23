//! Who owns a process and when it started, as the system reports them (registry §4).
//!
//! The engine reads its own start time with the same call a client uses on its pid,
//! so the two numbers compare equal without a tolerance.

/// Owner and start of a running process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessInfo {
    pub uid: u32,
    /// Microseconds since 1970 UTC, truncated.
    pub start_us: u64,
}

/// The current user's id; `None` where the registry has no user model yet (Windows).
pub fn current_uid() -> Option<u32> {
    #[cfg(unix)]
    {
        // SAFETY: getuid has no preconditions and cannot fail.
        Some(unsafe { libc::getuid() })
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// The process with this pid, or `None` when it does not exist or cannot be read.
pub fn process(pid: u32) -> Option<ProcessInfo> {
    if pid == 0 {
        return None;
    }
    imp::process(pid)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::ProcessInfo;

    pub fn process(pid: u32) -> Option<ProcessInfo> {
        let pid = libc::c_int::try_from(pid).ok()?;
        // SAFETY: an all-zero proc_bsdinfo is a valid value of this plain C struct.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        // SAFETY: the buffer is a live proc_bsdinfo of exactly `size` bytes.
        let read = unsafe {
            libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size)
        };
        if read != size {
            return None;
        }
        Some(ProcessInfo {
            uid: info.pbi_uid,
            start_us: info
                .pbi_start_tvsec
                .checked_mul(1_000_000)?
                .checked_add(info.pbi_start_tvusec)?,
        })
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::ProcessInfo;
    use std::os::unix::fs::MetadataExt;

    /// Start in clock ticks since boot (field 22 of /proc/<pid>/stat) plus the boot time.
    /// Both are fixed for the life of the process, so writer and reader agree exactly.
    pub fn process(pid: u32) -> Option<ProcessInfo> {
        let dir = format!("/proc/{pid}");
        let uid = std::fs::metadata(&dir).ok()?.uid();
        let stat = std::fs::read_to_string(format!("{dir}/stat")).ok()?;
        let after_name = &stat[stat.rfind(')')? + 2..];
        let ticks: u64 = after_name.split(' ').nth(19)?.parse().ok()?;
        let boot: u64 = std::fs::read_to_string("/proc/stat")
            .ok()?
            .lines()
            .find_map(|line| line.strip_prefix("btime "))?
            .trim()
            .parse()
            .ok()?;
        // SAFETY: sysconf has no preconditions.
        let hz = u64::try_from(unsafe { libc::sysconf(libc::_SC_CLK_TCK) }).ok()?;
        if hz == 0 {
            return None;
        }
        Some(ProcessInfo {
            uid,
            start_us: boot
                .checked_mul(1_000_000)?
                .checked_add(ticks.checked_mul(1_000_000)? / hz)?,
        })
    }
}

/// Windows (GetProcessTimes) and the rest: not in this first cut, so no record is live.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
    use super::ProcessInfo;

    pub fn process(_: u32) -> Option<ProcessInfo> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn reads_this_process_twice_the_same() {
        let me = process(std::process::id()).expect("own process");
        assert_eq!(Some(me.uid), current_uid());
        assert_eq!(process(std::process::id()), Some(me));
        assert!(me.start_us > 1_600_000_000_000_000);
    }

    #[test]
    fn pid_zero_is_never_a_process() {
        assert_eq!(process(0), None);
    }
}
