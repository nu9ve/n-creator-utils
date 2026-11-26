//! RFC 3339 UTC timestamps without a date library. The calendar is the one of nu9ve-ipc/src/time.rs; this
//! copy adds microseconds, like the Python reference.

use std::time::{SystemTime, UNIX_EPOCH};

/// `YYYY-MM-DDTHH:MM:SS.ffffffZ` for now.
pub fn now_utc() -> String {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_utc(since.as_secs(), since.subsec_micros())
}

pub fn format_utc(seconds: u64, micros: u32) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{micros:06}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// `YYYY-MM-DD` of a Unix time in UTC.
pub(crate) fn day_utc(seconds: u64) -> String {
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's days-to-civil algorithm, proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::{day_utc, format_utc};

    #[test]
    fn formats_known_instants() {
        assert_eq!(format_utc(0, 0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(format_utc(951_782_400, 7), "2000-02-29T00:00:00.000007Z");
        assert_eq!(
            format_utc(1_795_100_645, 123_456),
            "2026-11-19T15:04:05.123456Z"
        );
        assert_eq!(day_utc(1_795_100_645), "2026-11-19");
    }
}
