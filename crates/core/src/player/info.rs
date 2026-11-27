//! Technical description of a decoded track.

use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    /// Short codec name, uppercase (e.g. `FLAC`, `MP3`).
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Source bit depth when the format declares one (lossless formats).
    pub bits_per_sample: Option<u32>,
    pub duration: Option<Duration>,
}

impl TrackInfo {
    /// Status line in the `FLAC · 44.1kHz · 16bit` format.
    pub fn summary(&self) -> String {
        let mut parts = vec![self.codec.clone(), format_sample_rate(self.sample_rate)];
        if let Some(bits) = self.bits_per_sample {
            parts.push(format!("{bits}bit"));
        }
        parts.join(" · ")
    }
}

/// `44100` -> `44.1kHz`, `48000` -> `48kHz`.
pub fn format_sample_rate(hz: u32) -> String {
    let whole = hz / 1000;
    let frac = hz % 1000;
    if frac == 0 {
        return format!("{whole}kHz");
    }
    let decimals = format!("{frac:03}");
    format!("{whole}.{}kHz", decimals.trim_end_matches('0'))
}

/// `mm:ss` for the progress bar (minutes are not capped at 59).
pub fn format_clock(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(codec: &str, rate: u32, bits: Option<u32>) -> TrackInfo {
        TrackInfo {
            codec: codec.into(),
            sample_rate: rate,
            channels: 2,
            bits_per_sample: bits,
            duration: None,
        }
    }

    #[test]
    fn summary_matches_spec_example() {
        assert_eq!(
            info("FLAC", 44_100, Some(16)).summary(),
            "FLAC · 44.1kHz · 16bit"
        );
    }

    #[test]
    fn summary_omits_unknown_bit_depth() {
        assert_eq!(info("MP3", 48_000, None).summary(), "MP3 · 48kHz");
    }

    #[test]
    fn sample_rates_drop_trailing_zero_decimals() {
        assert_eq!(format_sample_rate(44_100), "44.1kHz");
        assert_eq!(format_sample_rate(88_200), "88.2kHz");
        assert_eq!(format_sample_rate(96_000), "96kHz");
        assert_eq!(format_sample_rate(22_050), "22.05kHz");
    }

    #[test]
    fn clock_is_zero_padded_mm_ss() {
        assert_eq!(format_clock(Duration::from_secs(0)), "00:00");
        assert_eq!(format_clock(Duration::from_millis(65_900)), "01:05");
        assert_eq!(format_clock(Duration::from_secs(3_725)), "62:05");
    }
}
