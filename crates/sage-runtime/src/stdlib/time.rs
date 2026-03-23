//! Time helper functions for the Sage standard library.

use chrono::{DateTime, TimeZone, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current time in milliseconds since Unix epoch.
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis() as i64
}

/// Get the current time in seconds since Unix epoch.
#[must_use]
pub fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs() as i64
}

/// Format a Unix timestamp (in milliseconds) using the given format string.
///
/// Uses chrono format specifiers:
/// - `%Y` = year (4 digits)
/// - `%m` = month (01-12)
/// - `%d` = day (01-31)
/// - `%H` = hour (00-23)
/// - `%M` = minute (00-59)
/// - `%S` = second (00-59)
/// - `%F` = ISO date (YYYY-MM-DD)
/// - `%T` = ISO time (HH:MM:SS)
#[must_use]
pub fn format_timestamp(timestamp_ms: i64, format: &str) -> String {
    let secs = timestamp_ms / 1000;
    let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;
    let dt: DateTime<Utc> = Utc.timestamp_opt(secs, nanos).unwrap();
    dt.format(format).to_string()
}

/// Parse a string into a Unix timestamp (in milliseconds) using the given format.
///
/// Uses chrono format specifiers (see `format_timestamp`).
pub fn parse_timestamp(s: &str, format: &str) -> Result<i64, String> {
    let dt = DateTime::parse_from_str(s, format)
        .or_else(|_| {
            // Try parsing as UTC without timezone
            chrono::NaiveDateTime::parse_from_str(s, format).map(|d| d.and_utc().fixed_offset())
        })
        .map_err(|e| {
            format!(
                "failed to parse timestamp '{}' with format '{}': {}",
                s, format, e
            )
        })?;
    Ok(dt.timestamp_millis())
}

// Time constants (in milliseconds)
pub const MS_PER_SECOND: i64 = 1000;
pub const MS_PER_MINUTE: i64 = 60_000;
pub const MS_PER_HOUR: i64 = 3_600_000;
pub const MS_PER_DAY: i64 = 86_400_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_ms() {
        let ms1 = now_ms();
        // Ensure time has advanced
        let ms2 = now_ms();
        assert!(ms2 >= ms1);
    }

    #[test]
    fn test_now_s() {
        let s = now_s();
        // Should be a reasonable timestamp
        assert!(s > 1_700_000_000); // After 2023
    }

    #[test]
    fn test_format_timestamp() {
        // 2024-01-15 10:50:00 UTC
        let ts = 1705315800000_i64;
        assert_eq!(format_timestamp(ts, "%Y-%m-%d"), "2024-01-15");
        assert_eq!(format_timestamp(ts, "%H:%M:%S"), "10:50:00");
    }

    #[test]
    fn test_parse_timestamp() {
        // Parse a date with timezone
        let result = parse_timestamp("2024-01-15 10:50:00 +0000", "%Y-%m-%d %H:%M:%S %z");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1705315800000);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MS_PER_SECOND, 1000);
        assert_eq!(MS_PER_MINUTE, 60 * 1000);
        assert_eq!(MS_PER_HOUR, 60 * 60 * 1000);
        assert_eq!(MS_PER_DAY, 24 * 60 * 60 * 1000);
    }
}
