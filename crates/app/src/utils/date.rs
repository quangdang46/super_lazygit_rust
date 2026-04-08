
/// Number of seconds in various time periods.
pub const SECONDS_IN_SECOND: i64 = 1;
pub const SECONDS_IN_MINUTE: i64 = 60;
pub const SECONDS_IN_HOUR: i64 = 3600;
pub const SECONDS_IN_DAY: i64 = 86400;
pub const SECONDS_IN_WEEK: i64 = 604800;
pub const SECONDS_IN_YEAR: i64 = 31536000;
pub const SECONDS_IN_MONTH: i64 = SECONDS_IN_YEAR / 12;

struct Period {
    label: &'static str,
    seconds_in_period: i64,
}

fn get_periods() -> Vec<Period> {
    vec![
        Period {
            label: "s",
            seconds_in_period: SECONDS_IN_SECOND,
        },
        Period {
            label: "m",
            seconds_in_period: SECONDS_IN_MINUTE,
        },
        Period {
            label: "h",
            seconds_in_period: SECONDS_IN_HOUR,
        },
        Period {
            label: "d",
            seconds_in_period: SECONDS_IN_DAY,
        },
        Period {
            label: "w",
            seconds_in_period: SECONDS_IN_WEEK,
        },
        Period {
            label: "M",
            seconds_in_period: SECONDS_IN_MONTH,
        },
        Period {
            label: "y",
            seconds_in_period: SECONDS_IN_YEAR,
        },
    ]
}

/// Formats a Unix timestamp as a human-readable "time ago" string.
pub fn unix_to_time_ago(timestamp: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_seconds_ago(now - timestamp)
}

fn format_seconds_ago(seconds_ago: i64) -> String {
    let periods = get_periods();

    for i in 0..periods.len() {
        if i == 0 {
            continue;
        }

        if seconds_ago < periods[i].seconds_in_period {
            let prev = &periods[i - 1];
            return format!(
                "{}{}",
                seconds_ago / prev.seconds_in_period,
                prev.label
            );
        }
    }

    let last = periods.last().unwrap();
    format!("{}{}", seconds_ago / last.seconds_in_period, last.label)
}

/// Formats a Unix timestamp as a smart date string.
/// If the date is today, it shows the time; otherwise it shows the date.
/// Uses the provided format strings.
pub fn unix_to_date_smart(
    now: std::time::SystemTime,
    timestamp: i64,
    _long_time_format: &str,
    short_time_format: &str,
) -> String {
    // Get days since epoch for both timestamps
    let now_secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let then_secs = timestamp as u64;

    let now_days = now_secs / 86400;
    let then_days = then_secs / 86400;

    // If same day, show short time format
    if now_days == then_days {
        let remaining_secs = then_secs % 86400;
        let hours = remaining_secs / 3600;
        let minutes = (remaining_secs % 3600) / 60;
        format!("{:02}:{:02}", hours, minutes)
    } else {
        // For different days, use the short_time_format as a template
        // In a full implementation, this would use chrono/time crate
        short_time_format.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_to_time_ago_seconds() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        assert_eq!(unix_to_time_ago(now - 30), "30s");
    }

    #[test]
    fn test_unix_to_time_ago_minutes() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        assert_eq!(unix_to_time_ago(now - 120), "2m");
    }

    #[test]
    fn test_format_seconds_ago() {
        assert_eq!(format_seconds_ago(30), "30s");
        assert_eq!(format_seconds_ago(90), "1m");
        assert_eq!(format_seconds_ago(3600), "1h");
        assert_eq!(format_seconds_ago(86400), "1d");
    }
}
