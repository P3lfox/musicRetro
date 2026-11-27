//! Detects an output device that silently stopped consuming audio
//! (e.g. a USB DAC unplugged on a backend that reports no error).

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct StallDetector {
    timeout: Duration,
    /// Last observed frame counter and when it last changed.
    last: Option<(u64, Instant)>,
}

impl StallDetector {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            last: None,
        }
    }

    /// Forgets history (call on pause, seek, or a new stream).
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// Records the device's played-frames counter while audio is pending.
    /// Returns `true` once the counter has not moved for `timeout`.
    pub fn observe(&mut self, frames_played: u64, now: Instant) -> bool {
        match self.last {
            Some((frames, since)) if frames == frames_played => {
                now.saturating_duration_since(since) >= self.timeout
            }
            _ => {
                self.last = Some((frames_played, now));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(2);

    #[test]
    fn first_observation_is_never_a_stall() {
        let mut d = StallDetector::new(TIMEOUT);
        assert!(!d.observe(0, Instant::now()));
    }

    #[test]
    fn advancing_counter_is_not_a_stall() {
        let mut d = StallDetector::new(TIMEOUT);
        let t0 = Instant::now();
        d.observe(0, t0);
        assert!(!d.observe(100, t0 + Duration::from_secs(3)));
        assert!(!d.observe(200, t0 + Duration::from_secs(6)));
    }

    #[test]
    fn frozen_counter_stalls_after_timeout() {
        let mut d = StallDetector::new(TIMEOUT);
        let t0 = Instant::now();
        d.observe(500, t0);
        assert!(!d.observe(500, t0 + Duration::from_millis(1_900)));
        assert!(d.observe(500, t0 + TIMEOUT));
    }

    #[test]
    fn timeout_restarts_when_counter_moves() {
        let mut d = StallDetector::new(TIMEOUT);
        let t0 = Instant::now();
        d.observe(0, t0);
        d.observe(10, t0 + Duration::from_millis(1_500));
        assert!(!d.observe(10, t0 + Duration::from_millis(3_000)));
        assert!(d.observe(10, t0 + Duration::from_millis(3_500)));
    }

    #[test]
    fn reset_forgets_previous_counter() {
        let mut d = StallDetector::new(TIMEOUT);
        let t0 = Instant::now();
        d.observe(7, t0);
        d.reset();
        assert!(!d.observe(7, t0 + Duration::from_secs(10)));
    }
}
