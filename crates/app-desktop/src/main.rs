// Hide the console window in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::time::Duration;

use retro_core::system::{battery, clock};
use slint::{ComponentHandle, Timer, TimerMode};

slint::include_modules!();

/// Number of cells drawn in the pixel battery icon.
const BATTERY_CELLS: u8 = 4;

/// The clock only shows minutes, so polling a few times per minute is plenty
/// and keeps the UI idle (no per-frame redraws).
const CLOCK_POLL: Duration = Duration::from_secs(5);
const BATTERY_POLL: Duration = Duration::from_secs(60);

fn main() -> anyhow::Result<()> {
    let window = AppWindow::new()?;

    refresh_clock(&window);
    refresh_battery(&window);

    let clock_timer = Timer::default();
    let weak = window.as_weak();
    clock_timer.start(TimerMode::Repeated, CLOCK_POLL, move || {
        if let Some(window) = weak.upgrade() {
            refresh_clock(&window);
        }
    });

    let battery_timer = Timer::default();
    let weak = window.as_weak();
    battery_timer.start(TimerMode::Repeated, BATTERY_POLL, move || {
        if let Some(window) = weak.upgrade() {
            refresh_battery(&window);
        }
    });

    window.run()?;
    Ok(())
}

/// Updates the clock only when the visible text changes, so Slint does not
/// schedule a redraw on every tick.
fn refresh_clock(window: &AppWindow) {
    let text = clock::now_status_time();
    if window.get_clock_text() != text.as_str() {
        window.set_clock_text(text.into());
    }
}

fn refresh_battery(window: &AppWindow) {
    match battery::read() {
        Some(status) => {
            window.set_battery_visible(true);
            window.set_battery_cells(i32::from(battery::filled_cells(
                status.percent,
                BATTERY_CELLS,
            )));
            window.set_battery_charging(status.charging);
        }
        None => window.set_battery_visible(false),
    }
}
