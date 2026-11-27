//! Status bar clock in the `M/D h:mmAM/PM` format (e.g. `5/24 6:35PM`).

use chrono::{Datelike, Local, Timelike};

/// Formats a date/time as `M/D h:mmAM/PM`, with no leading zeros on
/// month, day, or hour, and a 12-hour clock (`0` and `12` both map to `12`).
pub fn format_status_time(month: u32, day: u32, hour24: u32, minute: u32) -> String {
    let suffix = if hour24 < 12 { "AM" } else { "PM" };
    let hour12 = match hour24 % 12 {
        0 => 12,
        h => h,
    };
    format!("{month}/{day} {hour12}:{minute:02}{suffix}")
}

/// Current local time formatted for the status bar.
pub fn now_status_time() -> String {
    let now = Local::now();
    format_status_time(now.month(), now.day(), now.hour(), now.minute())
}

#[cfg(test)]
mod tests {
    use super::format_status_time;

    #[test]
    fn matches_spec_example() {
        assert_eq!(format_status_time(5, 24, 18, 35), "5/24 6:35PM");
    }

    #[test]
    fn midnight_is_twelve_am() {
        assert_eq!(format_status_time(1, 1, 0, 5), "1/1 12:05AM");
    }

    #[test]
    fn noon_is_twelve_pm() {
        assert_eq!(format_status_time(12, 31, 12, 0), "12/31 12:00PM");
    }

    #[test]
    fn morning_has_no_leading_zero() {
        assert_eq!(format_status_time(3, 9, 9, 7), "3/9 9:07AM");
    }
}
