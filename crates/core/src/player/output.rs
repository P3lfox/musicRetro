//! Audio device output with cpal, fed through a lock-free ring buffer.
//!
//! The device is opened at the track's native sample rate whenever it
//! supports it, so no resampling happens in this app (bit-perfect at
//! volume 1.0). On Windows, WASAPI converts unsupported rates itself.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};
use rtrb::{Producer, RingBuffer};

use super::error::{PlayerError, Result};

/// Seconds of decoded audio buffered ahead of the device.
const BUFFER_SECONDS: f32 = 0.5;

/// State shared with the realtime audio callback.
#[derive(Debug)]
pub struct OutputShared {
    /// Frames handed to the device since the stream was opened.
    pub frames_played: AtomicU64,
    /// Linear gain as `f32` bits.
    volume_bits: AtomicU32,
}

impl OutputShared {
    pub fn new(volume: f32) -> Self {
        Self {
            frames_played: AtomicU64::new(0),
            volume_bits: AtomicU32::new(volume.to_bits()),
        }
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

pub struct Output {
    stream: Stream,
    producer: Producer<f32>,
    capacity: usize,
    pub sample_rate: u32,
    pub channels: u16,
    pub device_name: String,
}

impl Output {
    /// Opens the default device for `sample_rate` / `channels` audio.
    /// The stream starts paused; call [`Output::play`].
    pub fn open(sample_rate: u32, channels: u16, shared: Arc<OutputShared>) -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(PlayerError::NoOutputDevice)?;
        let supported = pick_config(&device, sample_rate, channels)?;
        let config: StreamConfig = supported.config();
        let out_channels = config.channels;

        let capacity = (sample_rate as f32 * BUFFER_SECONDS) as usize * usize::from(out_channels);
        let (producer, mut consumer) = RingBuffer::<f32>::new(capacity);

        shared.frames_played.store(0, Ordering::Relaxed);
        let cb_shared = Arc::clone(&shared);
        let ch = usize::from(out_channels).max(1);

        let stream = device.build_output_stream::<f32, _, _>(
            config,
            move |out: &mut [f32], _| {
                let available = consumer.slots().min(out.len());
                // Only hand whole frames to the device.
                let available = available - available % ch;
                let volume = cb_shared.volume();
                if let Ok(chunk) = consumer.read_chunk(available) {
                    let (a, b) = chunk.as_slices();
                    out[..a.len()].copy_from_slice(a);
                    out[a.len()..a.len() + b.len()].copy_from_slice(b);
                    chunk.commit_all();
                }
                // Underrun (or end of track): pad with silence.
                out[available..].fill(0.0);
                if volume != 1.0 {
                    for s in &mut out[..available] {
                        *s *= volume;
                    }
                }
                cb_shared
                    .frames_played
                    .fetch_add((available / ch) as u64, Ordering::Relaxed);
            },
            |err| eprintln!("audio stream error: {err}"),
            None,
        )?;

        Ok(Self {
            stream,
            producer,
            capacity,
            sample_rate,
            channels: out_channels,
            device_name: device.to_string(),
        })
    }

    pub fn play(&self) -> Result<()> {
        Ok(self.stream.play()?)
    }

    pub fn pause(&self) -> Result<()> {
        Ok(self.stream.pause()?)
    }

    /// Pushes as many samples from `samples` as fit; returns how many were written.
    pub fn push(&mut self, samples: &[f32]) -> usize {
        let n = self.producer.slots().min(samples.len());
        if n == 0 {
            return 0;
        }
        match self.producer.write_chunk_uninit(n) {
            Ok(chunk) => chunk.fill_from_iter(samples[..n].iter().copied()),
            Err(_) => 0,
        }
    }

    /// True once the device has consumed everything that was pushed.
    pub fn is_drained(&self) -> bool {
        self.producer.slots() == self.capacity
    }
}

/// Picks an `f32` output config, preferring the exact sample rate and
/// channel count of the track.
fn pick_config(
    device: &cpal::Device,
    sample_rate: u32,
    channels: u16,
) -> Result<SupportedStreamConfig> {
    let ranges: Vec<_> = device
        .supported_output_configs()?
        .filter(|r| r.sample_format() == SampleFormat::F32)
        .collect();

    let at_rate = |want_channels: Option<u16>| {
        ranges
            .iter()
            .filter(|r| want_channels.is_none_or(|c| r.channels() == c))
            .find_map(|r| r.try_with_sample_rate(sample_rate))
    };

    at_rate(Some(channels))
        .or_else(|| at_rate(Some(2)))
        .or_else(|| at_rate(None))
        .ok_or(PlayerError::UnsupportedSampleRate(sample_rate))
}
