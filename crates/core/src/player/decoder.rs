//! File decoding with symphonia into interleaved `f32` chunks.

use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::units::{Time, TimeBase, Timestamp};

use super::error::{PlayerError, Result};
use super::info::TrackInfo;

pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    info: TrackInfo,
    /// After an accurate seek, packets ending before this timestamp are dropped.
    skip_until: Option<Timestamp>,
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|source| PlayerError::Open {
            path: path.to_owned(),
            source,
        })?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format = symphonia::default::get_probe().probe(
            &hint,
            mss,
            Default::default(),
            Default::default(),
        )?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or(PlayerError::NoAudioTrack)?;
        let track_id = track.id;
        let time_base = track.time_base;
        let duration = time_base
            .zip(track.duration)
            .and_then(|(tb, d)| tb.calc_duration(d))
            .map(|t| Duration::from_secs_f64(t.as_secs_f64().max(0.0)));
        let params = track
            .codec_params
            .as_ref()
            .and_then(|c| c.audio())
            .ok_or(PlayerError::NoAudioTrack)?
            .clone();

        let codecs = symphonia::default::get_codecs();
        let codec = codecs
            .get_audio_decoder(params.codec)
            .map(|r| r.codec.info.short_name.to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".into());
        let decoder = codecs.make_audio_decoder(&params, &AudioDecoderOptions::default())?;

        let sample_rate = params
            .sample_rate
            .ok_or(PlayerError::MissingParameter("sample rate"))?;
        let channels = params
            .channels
            .as_ref()
            .map(|c| c.count())
            .ok_or(PlayerError::MissingParameter("channel layout"))?;
        let channels =
            u16::try_from(channels).map_err(|_| PlayerError::MissingParameter("channel layout"))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            time_base,
            info: TrackInfo {
                codec,
                sample_rate,
                channels,
                bits_per_sample: params.bits_per_sample,
                duration,
            },
            skip_until: None,
        })
    }

    pub fn info(&self) -> &TrackInfo {
        &self.info
    }

    /// Decodes the next packet into `out` (interleaved, `info().channels`).
    /// Returns `Ok(false)` at end of stream. Corrupt packets are skipped.
    pub fn next_chunk(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(false),
                Err(SymError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(false);
                }
                // Chained streams (e.g. OGG with a new logical stream) are
                // treated as the end of this track for now.
                Err(SymError::ResetRequired) => return Ok(false),
                Err(e) => return Err(e.into()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let packet_end = Timestamp::new(packet.pts.get() + packet.dur.get() as i64);
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    if let Some(target) = self.skip_until {
                        if packet_end <= target {
                            continue;
                        }
                        self.skip_until = None;
                    }
                    buf.copy_to_vec_interleaved(out);
                    return Ok(true);
                }
                Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Seeks to `position` and returns the position actually reached.
    pub fn seek(&mut self, position: Duration) -> Result<Duration> {
        let time = Time::try_from_secs_f64(position.as_secs_f64())
            .ok_or(PlayerError::MissingParameter("valid seek time"))?;
        let seeked = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.decoder.reset();
        self.skip_until = Some(seeked.required_ts);
        Ok(self.ts_to_duration(seeked.required_ts).unwrap_or(position))
    }

    fn ts_to_duration(&self, ts: Timestamp) -> Option<Duration> {
        let secs = self.time_base?.calc_time(ts)?.as_secs_f64();
        Some(Duration::from_secs_f64(secs.max(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const RATE: u32 = 44_100;

    /// Writes a 16-bit stereo PCM WAV of `frames` frames to a temp file.
    fn write_wav(name: &str, frames: u32) -> PathBuf {
        let channels: u16 = 2;
        let data_len = frames * u32::from(channels) * 2;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&RATE.to_le_bytes());
        bytes.extend_from_slice(&(RATE * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let s = ((i % 100) as i16 - 50) * 200;
            bytes.extend_from_slice(&s.to_le_bytes());
            bytes.extend_from_slice(&(-s).to_le_bytes());
        }
        let dir = std::env::temp_dir().join(format!("retro-core-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write wav");
        path
    }

    fn cleanup(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn decode_all(dec: &mut Decoder) -> usize {
        let mut total = 0;
        let mut buf = Vec::new();
        while dec.next_chunk(&mut buf).expect("decode") {
            total += buf.len();
        }
        total / usize::from(dec.info().channels)
    }

    #[test]
    fn reads_track_info_from_wav() {
        let path = write_wav("info.wav", RATE);
        let dec = Decoder::open(&path).expect("open");
        let info = dec.info();
        assert_eq!(info.sample_rate, RATE);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, Some(16));
        let secs = info.duration.expect("duration").as_secs_f64();
        assert!((secs - 1.0).abs() < 0.01, "duration was {secs}");
        assert!(
            info.summary().ends_with("44.1kHz · 16bit"),
            "{}",
            info.summary()
        );
        cleanup(&path);
    }

    #[test]
    fn decodes_every_frame() {
        let path = write_wav("frames.wav", RATE / 2);
        let mut dec = Decoder::open(&path).expect("open");
        assert_eq!(decode_all(&mut dec), (RATE / 2) as usize);
        cleanup(&path);
    }

    #[test]
    fn seek_skips_to_requested_position() {
        let path = write_wav("seek.wav", RATE);
        let mut dec = Decoder::open(&path).expect("open");
        let reached = dec.seek(Duration::from_millis(500)).expect("seek");
        assert!(
            (reached.as_secs_f64() - 0.5).abs() < 0.05,
            "reached {reached:?}"
        );
        let remaining = decode_all(&mut dec) as f64 / f64::from(RATE);
        // Packet granularity: allow a small tolerance around the 0.5 s left.
        assert!((remaining - 0.5).abs() < 0.05, "remaining {remaining}");
        cleanup(&path);
    }

    #[test]
    fn missing_file_is_an_open_error() {
        let err = Decoder::open(Path::new("/nonexistent/track.flac"))
            .err()
            .expect("should fail");
        assert!(matches!(err, PlayerError::Open { .. }), "{err}");
    }
}
