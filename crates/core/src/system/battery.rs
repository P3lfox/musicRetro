//! Battery level for the status bar.
//!
//! Returns `None` on machines without a battery (desktops) or when the
//! platform does not expose it, so the UI can hide the indicator.

/// Snapshot of the main battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryStatus {
    /// Charge level, 0..=100.
    pub percent: u8,
    /// True while plugged in and charging.
    pub charging: bool,
}

/// Reads the current battery status, if any.
pub fn read() -> Option<BatteryStatus> {
    imp::read()
}

/// Maps a charge percentage to a number of filled cells (0..=cells).
/// Any non-empty battery shows at least one cell.
pub fn filled_cells(percent: u8, cells: u8) -> u8 {
    let percent = percent.min(100);
    if percent == 0 || cells == 0 {
        return 0;
    }
    let filled = (u16::from(percent) * u16::from(cells)).div_ceil(100);
    filled as u8
}

#[cfg(target_os = "linux")]
mod imp {
    use super::BatteryStatus;
    use std::fs;
    use std::path::Path;

    const POWER_SUPPLY: &str = "/sys/class/power_supply";

    pub fn read() -> Option<BatteryStatus> {
        let entries = fs::read_dir(POWER_SUPPLY).ok()?;
        entries
            .flatten()
            .map(|e| e.path())
            .find(|p| read_trimmed(&p.join("type")).as_deref() == Some("Battery"))
            .and_then(|p| parse(&p))
    }

    fn parse(dir: &Path) -> Option<BatteryStatus> {
        let percent: u8 = read_trimmed(&dir.join("capacity"))?.parse().ok()?;
        let charging = read_trimmed(&dir.join("status")).as_deref() == Some("Charging");
        Some(BatteryStatus {
            percent: percent.min(100),
            charging,
        })
    }

    fn read_trimmed(path: &Path) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
    }
}

#[cfg(windows)]
mod imp {
    use super::BatteryStatus;
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    // Values documented for SYSTEM_POWER_STATUS.
    const NO_SYSTEM_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;
    const CHARGING: u8 = 8;
    const AC_ONLINE: u8 = 1;

    pub fn read() -> Option<BatteryStatus> {
        let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        // SAFETY: `status` is a valid, writable SYSTEM_POWER_STATUS.
        if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
            return None;
        }
        if status.BatteryFlag == NO_SYSTEM_BATTERY
            || status.BatteryFlag == UNKNOWN
            || status.BatteryLifePercent == UNKNOWN
        {
            return None;
        }
        Some(BatteryStatus {
            percent: status.BatteryLifePercent.min(100),
            charging: status.BatteryFlag & CHARGING != 0 || status.ACLineStatus == AC_ONLINE,
        })
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    use super::BatteryStatus;

    pub fn read() -> Option<BatteryStatus> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::filled_cells;

    #[test]
    fn empty_battery_has_no_cells() {
        assert_eq!(filled_cells(0, 4), 0);
    }

    #[test]
    fn full_battery_fills_all_cells() {
        assert_eq!(filled_cells(100, 4), 4);
    }

    #[test]
    fn low_battery_still_shows_one_cell() {
        assert_eq!(filled_cells(3, 4), 1);
    }

    #[test]
    fn rounds_up_to_next_cell() {
        assert_eq!(filled_cells(51, 4), 3);
        assert_eq!(filled_cells(50, 4), 2);
    }

    #[test]
    fn clamps_out_of_range_percent() {
        assert_eq!(filled_cells(250, 4), 4);
    }
}
