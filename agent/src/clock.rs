//! Minimal UTC RFC 3339 timestamp formatting, without a date dependency.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current time as a UTC RFC 3339 string, e.g. `2026-10-13T12:00:00Z`.
pub fn now_rfc3339() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    format_rfc3339(seconds)
}

/// Format a Unix timestamp (seconds) as UTC RFC 3339.
pub fn format_rfc3339(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert a day count since the Unix epoch to a civil (year, month, day).
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn formats_known_instants() {
        assert_eq!(format_rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        // A leap-day instant.
        assert_eq!(format_rfc3339(1_582_934_400), "2020-02-29T00:00:00Z");
    }

    #[test]
    fn now_is_well_formed() {
        let now = now_rfc3339();
        assert_eq!(now.len(), 20);
        assert!(now.ends_with('Z'));
        assert_eq!(&now[4..5], "-");
    }
}
