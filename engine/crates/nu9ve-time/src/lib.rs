//! Civil time with Python's `datetime` semantics, for answers that must match the Python engine byte for byte:
//! `fromisoformat`, `isoformat`, the `strftime` codes the suite uses, `fromtimestamp` and `astimezone()`.
//!
//! Local time follows CPython exactly: an instant becomes local with the OS `localtime` (on macOS the same
//! `localtime_r` Python calls, so `TZ` and `/etc/localtime` agree), and a naive wall time becomes an instant with
//! CPython's `local_to_seconds` search (fold 0: the earlier of two repeated times, and after a gap the later
//! offset), not with `mktime`. On Windows the zone comes from the system (chrono's `Local`).

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

pub use chrono;
pub use chrono::NaiveDateTime as Naive;

/// A `datetime`: wall time plus an optional UTC offset in seconds (`None` is a naive datetime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub wall: NaiveDateTime,
    pub offset: Option<i32>,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MAX_FOLD_SECONDS: i64 = 24 * 3600;

fn digits(text: &str) -> Option<u32> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok())?
}

/// `HH[:MM[:SS[.fff[fff]]]]` as Python's `_parse_isoformat_time` (3.7–3.10, also accepted by 3.11+).
fn parse_time(text: &str) -> Option<(u32, u32, u32, u32)> {
    let (clock, fraction) = match text.split_once('.') {
        Some((c, f)) => (c, Some(f)),
        None => (text, None),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.len() != 2) {
        return None;
    }
    let mut values = [0u32; 3];
    for (i, p) in parts.iter().enumerate() {
        values[i] = digits(p)?;
    }
    let micro = match fraction {
        None => 0,
        Some(f) if parts.len() == 3 && (f.len() == 3 || f.len() == 6) => {
            let value = digits(f)?;
            if f.len() == 3 { value * 1000 } else { value }
        }
        Some(_) => return None,
    };
    Some((values[0], values[1], values[2], micro))
}

/// `±HH:MM[:SS[.ffffff]]` → seconds (microseconds of an offset are dropped: no source in the suite has them).
fn parse_offset(sign: i32, text: &str) -> Option<i32> {
    let (h, m, s, _) = parse_time(text)?;
    if text.len() < 5 || h > 23 || m > 59 || s > 59 {
        return None;
    }
    Some(sign * (h * 3600 + m * 60 + s) as i32)
}

impl DateTime {
    /// Python `datetime.fromisoformat` for `YYYY-MM-DD[<sep>HH[:MM[:SS[.ffffff]]][±HH:MM[:SS]]]`, the separator
    /// being any single character. `Z` is not accepted (as in Python before 3.11; the suite replaces it first).
    pub fn fromisoformat(text: &str) -> Option<DateTime> {
        if !text.is_ascii() || text.len() < 10 {
            return None;
        }
        let (date, rest) = text.split_at(10);
        let b = date.as_bytes();
        if b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let day = NaiveDate::from_ymd_opt(
            digits(&date[0..4])? as i32,
            digits(&date[5..7])?,
            digits(&date[8..10])?,
        )?;
        if rest.is_empty() {
            return Some(DateTime {
                wall: day.and_hms_opt(0, 0, 0)?,
                offset: None,
            });
        }
        let time = &rest[1..];
        let (clock, offset) = match time.find(['+', '-']) {
            Some(i) => {
                let sign = if time.as_bytes()[i] == b'-' { -1 } else { 1 };
                (&time[..i], Some(parse_offset(sign, &time[i + 1..])?))
            }
            None => (time, None),
        };
        let (h, m, s, us) = parse_time(clock)?;
        let wall = day.and_time(NaiveTime::from_hms_micro_opt(h, m, s, us)?);
        Some(DateTime { wall, offset })
    }

    /// Python `isoformat()`: `T`, microseconds only when not zero, the offset as `+HH:MM` (`:SS` when needed).
    pub fn isoformat(&self) -> String {
        let w = self.wall;
        let mut out = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            w.year(),
            w.month(),
            w.day(),
            w.hour(),
            w.minute(),
            w.second()
        );
        let micro = w.nanosecond() / 1000;
        if micro != 0 {
            out.push_str(&format!(".{micro:06}"));
        }
        if let Some(offset) = self.offset {
            let sign = if offset < 0 { '-' } else { '+' };
            let a = offset.unsigned_abs();
            out.push_str(&format!("{sign}{:02}:{:02}", a / 3600, a % 3600 / 60));
            if a % 60 != 0 {
                out.push_str(&format!(":{:02}", a % 60));
            }
        }
        out
    }

    /// Python `strftime` for `%Y %y %m %b %d %H %M %S %%` in the C locale; other codes are kept as written.
    pub fn strftime(&self, pattern: &str) -> String {
        let w = self.wall;
        let mut out = String::new();
        let mut chars = pattern.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('Y') => out.push_str(&format!("{:04}", w.year())),
                Some('y') => out.push_str(&format!("{:02}", w.year().rem_euclid(100))),
                Some('m') => out.push_str(&format!("{:02}", w.month())),
                Some('b') => out.push_str(MONTHS[w.month0() as usize]),
                Some('d') => out.push_str(&format!("{:02}", w.day())),
                Some('H') => out.push_str(&format!("{:02}", w.hour())),
                Some('M') => out.push_str(&format!("{:02}", w.minute())),
                Some('S') => out.push_str(&format!("{:02}", w.second())),
                Some('%') => out.push('%'),
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
                None => out.push('%'),
            }
        }
        out
    }

    /// Seconds since the epoch of an aware datetime (`None` for a naive one).
    pub fn timestamp(&self) -> Option<i64> {
        self.offset
            .map(|o| self.wall.and_utc().timestamp() - i64::from(o))
    }

    /// Python `astimezone(timezone.utc)`. A naive value is first taken as local time, as Python does.
    pub fn to_utc(&self) -> DateTime {
        let local = self.astimezone();
        let wall = local.wall - chrono::Duration::seconds(i64::from(local.offset.unwrap_or(0)));
        DateTime {
            wall,
            offset: Some(0),
        }
    }

    /// Python `astimezone()` with no argument: the same instant in the host's zone (a naive value is local).
    pub fn astimezone(&self) -> DateTime {
        self.astimezone_with(&host_local)
    }

    pub fn astimezone_with(&self, local: &dyn Fn(i64) -> i64) -> DateTime {
        let micro = self.wall.nanosecond() / 1000;
        let seconds = match self.timestamp() {
            Some(t) => t,
            None => local_to_seconds(local, self.wall.with_nanosecond(0).expect("valid")),
        };
        let offset = (local(seconds) - seconds) as i32;
        let wall = chrono::DateTime::from_timestamp(seconds + i64::from(offset), micro * 1000)
            .expect("in range")
            .naive_utc();
        DateTime {
            wall,
            offset: Some(offset),
        }
    }

    /// The naive wall time (Python `replace(tzinfo=None)`).
    pub fn naive(&self) -> DateTime {
        DateTime {
            wall: self.wall,
            offset: None,
        }
    }

    /// Python `fromtimestamp(t)` (naive local time, microseconds rounded half to even).
    pub fn fromtimestamp(t: f64) -> DateTime {
        Self::fromtimestamp_with(t, &host_local)
    }

    pub fn fromtimestamp_with(t: f64, local: &dyn Fn(i64) -> i64) -> DateTime {
        let (seconds, micro) = split_timestamp(t);
        let wall = chrono::DateTime::from_timestamp(local(seconds), micro * 1000)
            .expect("in range")
            .naive_utc();
        DateTime { wall, offset: None }
    }

    /// Python `fromtimestamp(t, timezone.utc)`.
    pub fn fromtimestamp_utc(t: f64) -> DateTime {
        let (seconds, micro) = split_timestamp(t);
        let wall = chrono::DateTime::from_timestamp(seconds, micro * 1000)
            .expect("in range")
            .naive_utc();
        DateTime {
            wall,
            offset: Some(0),
        }
    }
}

/// CPython `_PyTime_ObjectToTimeval` with ROUND_HALF_EVEN: whole seconds and microseconds of a float timestamp.
fn split_timestamp(t: f64) -> (i64, u32) {
    let floor = t.floor();
    let mut seconds = floor as i64;
    let fraction = (t - floor) * 1e6;
    let mut micro = fraction.round_ties_even() as i64;
    if micro >= 1_000_000 {
        seconds += 1;
        micro -= 1_000_000;
    }
    (seconds, micro as u32)
}

/// CPython `local_to_seconds(…, fold=0)`: the instant whose local wall time is `wall`.
pub fn local_to_seconds(local: &dyn Fn(i64) -> i64, wall: NaiveDateTime) -> i64 {
    let t = wall.and_utc().timestamp();
    let a = local(t) - t;
    let u1 = t - a;
    let t1 = local(u1);
    let b = if t1 == t {
        let u2 = u1 - MAX_FOLD_SECONDS;
        let other = local(u2) - u2;
        if a == other {
            return u1;
        }
        other
    } else {
        t1 - u1
    };
    let u2 = t - b;
    let t2 = local(u2);
    if t2 == t {
        return u2;
    }
    if t1 == t {
        return u1;
    }
    // In a gap: neither offset solves it; fold 0 takes the later instant.
    u1.max(u2)
}

// CPython `local(u)`: the local wall time of instant `u`, as seconds read like UTC.
#[cfg(unix)]
unsafe extern "C" {
    fn tzset();
}

#[cfg(unix)]
pub fn host_local(u: i64) -> i64 {
    let time = u as libc::time_t;
    // Read `TZ` once, as Python does at start; `localtime_r` alone need not.
    static ZONE: std::sync::Once = std::sync::Once::new();
    // SAFETY: `tzset` only reads the environment and sets libc's zone globals.
    ZONE.call_once(|| unsafe { tzset() });
    // SAFETY: `localtime_r` writes into the provided `tm`; both pointers are valid for the call.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&time, &mut tm).is_null() };
    if !ok {
        return u;
    }
    NaiveDate::from_ymd_opt(tm.tm_year + 1900, (tm.tm_mon + 1) as u32, tm.tm_mday as u32)
        .and_then(|d| {
            d.and_hms_opt(
                tm.tm_hour as u32,
                tm.tm_min as u32,
                tm.tm_sec.min(59) as u32,
            )
        })
        .map_or(u, |w| w.and_utc().timestamp())
}

#[cfg(windows)]
pub fn host_local(u: i64) -> i64 {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(u, 0) {
        chrono::LocalResult::Single(d) | chrono::LocalResult::Ambiguous(d, _) => {
            d.naive_local().and_utc().timestamp()
        }
        chrono::LocalResult::None => u,
    }
}

/// IANA offset at an absolute instant, without changing the process timezone.
pub fn named_zone_offset(zone: &str, timestamp: i64) -> Option<i32> {
    let tz = jiff::tz::TimeZone::get(zone).ok()?;
    // The database accepts case-insensitive lookups; Python's IANA keys use exact spelling.
    if tz.iana_name()? != zone {
        return None;
    }
    let instant = jiff::Timestamp::new(timestamp, 0).ok()?;
    Some(tz.to_offset(instant).seconds())
}

/// Python `datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z')` (`common.now`).
pub fn now_utc() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let wall = chrono::DateTime::from_timestamp(now.as_secs() as i64, now.subsec_micros() * 1000)
        .expect("in range")
        .naive_utc();
    DateTime {
        wall,
        offset: Some(0),
    }
    .isoformat()
    .replace("+00:00", "Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zone like America/Denver in 2026: -7 h, -6 h from 8 Mar 09:00 UTC to 1 Nov 08:00 UTC.
    fn denver(u: i64) -> i64 {
        let start = NaiveDate::from_ymd_opt(2026, 3, 8)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let end = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        u + if (start..end).contains(&u) {
            -6 * 3600
        } else {
            -7 * 3600
        }
    }

    fn at(text: &str) -> DateTime {
        DateTime::fromisoformat(text).unwrap()
    }

    #[test]
    fn named_zones_resolve_at_the_instant_and_preserve_key_spelling() {
        let offset = |zone, iso| {
            named_zone_offset(
                zone,
                DateTime::fromisoformat(iso).unwrap().timestamp().unwrap(),
            )
        };
        assert_eq!(
            offset("America/New_York", "2026-11-01T05:59:59+00:00"),
            Some(-14_400)
        );
        assert_eq!(
            offset("America/New_York", "2026-11-01T06:00:00+00:00"),
            Some(-18_000)
        );
        assert_eq!(
            offset("America/Hermosillo", "2026-07-01T12:00:00+00:00"),
            Some(-25_200)
        );
        assert_eq!(
            offset("US/Eastern", "2026-07-01T12:00:00+00:00"),
            Some(-14_400)
        );
        assert_eq!(offset("Etc/UTC", "2026-07-01T12:00:00+00:00"), Some(0));
        assert_eq!(
            offset("america/new_york", "2026-07-01T12:00:00+00:00"),
            None
        );
        assert_eq!(
            offset("../America/New_York", "2026-07-01T12:00:00+00:00"),
            None
        );
    }

    #[test]
    fn fromisoformat_and_isoformat_round_trip_like_python() {
        assert_eq!(at("2026-11-04").isoformat(), "2026-11-04T00:00:00");
        assert_eq!(at("2026-11-04 16:32:12").isoformat(), "2026-11-04T16:32:12");
        assert_eq!(at("2026-11-04T16:32").isoformat(), "2026-11-04T16:32:00");
        assert!(DateTime::fromisoformat("2026-11-04T16:32:12.5000").is_none());
    }

    #[test]
    fn parsing_rejects_what_python_rejects() {
        for bad in [
            "2026-11-04T16:32:12.5",
            "2026-13-01",
            "2026/11/04",
            "2026-11-04T1:2",
            "2026-11-04T16:32:12Z",
            "",
        ] {
            assert!(DateTime::fromisoformat(bad).is_none(), "{bad}");
        }
        assert_eq!(
            at("2026-11-04T16:32:12.123").isoformat(),
            "2026-11-04T16:32:12.123000"
        );
        assert_eq!(
            at("2026-11-04T16:32:12-07:00").isoformat(),
            "2026-11-04T16:32:12-07:00"
        );
        assert_eq!(
            at("2026-11-04T16:32:12+05:30:15").offset,
            Some(5 * 3600 + 30 * 60 + 15)
        );
    }

    #[test]
    fn strftime_codes_of_the_naming() {
        let d = at("2026-11-04T06:02:09");
        assert_eq!(
            d.strftime("%y") + &d.strftime("%b").to_uppercase() + &d.strftime("%d-%H%M%S"),
            "26NOV04-060209"
        );
        assert_eq!(d.strftime("%Y/%m %d%m-%H%M%S %%"), "2026/11 0411-060209 %");
    }

    #[test]
    fn local_time_follows_cpython_in_folds_and_gaps() {
        // Ordinary times.
        let summer = at("2026-07-01T12:00:00").astimezone_with(&denver);
        assert_eq!(summer.isoformat(), "2026-07-01T12:00:00-06:00");
        assert_eq!(
            at("2026-07-01T18:00:00+00:00")
                .astimezone_with(&denver)
                .isoformat(),
            "2026-07-01T12:00:00-06:00"
        );
        // 1 Nov 01:30 happens twice: fold 0 is the first (daylight) one.
        assert_eq!(
            at("2026-11-01T01:30:00")
                .astimezone_with(&denver)
                .isoformat(),
            "2026-11-01T01:30:00-06:00"
        );
        // 8 Mar 02:30 never happens: CPython moves it forward with the earlier offset (03:30 daylight).
        assert_eq!(
            at("2026-03-08T02:30:00")
                .astimezone_with(&denver)
                .isoformat(),
            "2026-03-08T03:30:00-06:00"
        );
    }

    #[test]
    fn timestamps_round_microseconds_half_to_even() {
        let utc = DateTime::fromtimestamp_utc(1_778_000_000.000_000_5);
        assert_eq!(utc.wall.nanosecond(), 0);
        assert_eq!(
            DateTime::fromtimestamp_utc(-0.25).isoformat(),
            "1969-12-31T23:59:59.750000+00:00"
        );
        let local = DateTime::fromtimestamp_with(1_783_000_000.0, &denver);
        assert_eq!(local.offset, None);
        assert_eq!(local.astimezone_with(&denver).offset, Some(-6 * 3600));
        assert_eq!(
            at("2026-07-01T12:00:00-06:00").to_utc().isoformat(),
            "2026-07-01T18:00:00+00:00"
        );
    }
}
