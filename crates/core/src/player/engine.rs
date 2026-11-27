//! Player engine: runs decoding on its own thread and talks to callers
//! through channels, so the UI never blocks audio and vice versa.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::decoder::Decoder;
use super::error::{PlayerError, Result};
use super::info::TrackInfo;
use super::output::{Output, OutputShared};
use super::queue::{Queue, RepeatMode};
use super::remix::remix_into;
use super::watchdog::StallDetector;

/// Going "previous" after this much playback restarts the current track.
const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
/// How long the engine waits for commands while the device buffer is full;
/// well below the 0.5 s buffer, so playback never underruns.
const FEED_INTERVAL: Duration = Duration::from_millis(40);
/// A device that consumes no audio for this long while playing is treated as lost.
const STALL_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub enum Command {
    Load(Vec<PathBuf>),
    Play,
    Pause,
    TogglePause,
    Next,
    Previous,
    Seek(Duration),
    SetVolume(f32),
    SetRepeat(RepeatMode),
    Stop,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PlaybackState {
    Stopped = 0,
    Playing = 1,
    Paused = 2,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    TrackStarted {
        index: usize,
        path: PathBuf,
        info: TrackInfo,
        device: String,
    },
    StateChanged(PlaybackState),
    Seeked(Duration),
    QueueEnded,
    /// The output device failed or disappeared (e.g. DAC unplugged);
    /// playback stopped and the queue position is kept.
    OutputLost {
        message: String,
    },
    /// A track could not be played; the engine skips to the next one.
    TrackError {
        path: PathBuf,
        message: String,
    },
}

/// Lock-free snapshot readable from any thread (e.g. the UI's progress bar).
#[derive(Debug)]
struct Status {
    state: AtomicU8,
    /// Position at which the current stream started, in milliseconds.
    base_ms: AtomicU64,
    sample_rate: AtomicU64,
    output: Mutex<Option<Arc<OutputShared>>>,
}

impl Status {
    fn state(&self) -> PlaybackState {
        match self.state.load(Ordering::Relaxed) {
            1 => PlaybackState::Playing,
            2 => PlaybackState::Paused,
            _ => PlaybackState::Stopped,
        }
    }

    /// Position at stream start plus frames the device consumed since.
    fn position(&self) -> Duration {
        let base = Duration::from_millis(self.base_ms.load(Ordering::Relaxed));
        let rate = self.sample_rate.load(Ordering::Relaxed);
        match self.output() {
            Some(out) if rate > 0 => {
                let frames = out.frames_played.load(Ordering::Relaxed);
                base + Duration::from_secs_f64(frames as f64 / rate as f64)
            }
            _ => base,
        }
    }

    fn output(&self) -> Option<Arc<OutputShared>> {
        self.output
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Handle to the player thread. Dropping it stops playback.
pub struct Player {
    commands: Sender<Command>,
    events: Receiver<PlayerEvent>,
    status: Arc<Status>,
    thread: Option<JoinHandle<()>>,
}

impl Player {
    pub fn spawn() -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let status = Arc::new(Status {
            state: AtomicU8::new(PlaybackState::Stopped as u8),
            base_ms: AtomicU64::new(0),
            sample_rate: AtomicU64::new(0),
            output: Mutex::new(None),
        });
        let engine_status = Arc::clone(&status);
        let thread = std::thread::Builder::new()
            .name("audio-engine".into())
            .spawn(move || Engine::new(cmd_rx, evt_tx, engine_status).run())
            .map_err(|_| PlayerError::Disconnected)?;
        Ok(Self {
            commands: cmd_tx,
            events: evt_rx,
            status,
            thread: Some(thread),
        })
    }

    pub fn send(&self, command: Command) -> Result<()> {
        self.commands
            .send(command)
            .map_err(|_| PlayerError::Disconnected)
    }

    /// Events emitted by the engine, in order.
    pub fn events(&self) -> &Receiver<PlayerEvent> {
        &self.events
    }

    pub fn state(&self) -> PlaybackState {
        self.status.state()
    }

    /// Current playback position, derived from frames the device consumed.
    pub fn position(&self) -> Duration {
        self.status.position()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The current track: decoder + device stream + samples not yet accepted
/// by the ring buffer.
struct Active {
    decoder: Decoder,
    output: Output,
    pending: Vec<f32>,
    pending_offset: usize,
    decode_buf: Vec<f32>,
    finished_decoding: bool,
}

struct Engine {
    commands: Receiver<Command>,
    events: Sender<PlayerEvent>,
    status: Arc<Status>,
    queue: Queue,
    active: Option<Active>,
    volume: f32,
    /// `Cell` so state changes (`&self`) can reset it.
    stall: Cell<StallDetector>,
}

impl Engine {
    fn new(commands: Receiver<Command>, events: Sender<PlayerEvent>, status: Arc<Status>) -> Self {
        Self {
            commands,
            events,
            status,
            queue: Queue::default(),
            active: None,
            volume: 1.0,
            stall: Cell::new(StallDetector::new(STALL_TIMEOUT)),
        }
    }

    fn run(mut self) {
        loop {
            let command = if self.state() == PlaybackState::Playing {
                match self.commands.recv_timeout(Duration::ZERO) {
                    Ok(c) => Some(c),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                // Idle: block until something happens, using no CPU.
                match self.commands.recv() {
                    Ok(c) => Some(c),
                    Err(_) => return,
                }
            };

            if let Some(command) = command {
                if matches!(command, Command::Shutdown) {
                    return;
                }
                self.handle(command);
                continue;
            }

            if !self.feed() {
                if self.output_lost() {
                    continue;
                }
                // Device buffer is full (or draining): wait briefly for commands.
                match self.commands.recv_timeout(FEED_INTERVAL) {
                    Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                    Ok(c) => self.handle(c),
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Load(tracks) => {
                let repeat = self.queue.repeat();
                self.queue = Queue::new(tracks);
                self.queue.set_repeat(repeat);
                match self.queue.current_index() {
                    Some(_) => self.start_current(),
                    None => self.stop(),
                }
            }
            Command::Play => self.resume(),
            Command::Pause => self.pause(),
            Command::TogglePause => match self.state() {
                PlaybackState::Playing => self.pause(),
                _ => self.resume(),
            },
            Command::Next => {
                if self.queue.skip_next().is_some() {
                    self.start_current();
                } else {
                    self.end_queue();
                }
            }
            Command::Previous => {
                let restart = self.active.is_some() && self.position() > RESTART_THRESHOLD;
                if !restart {
                    self.queue.skip_previous();
                }
                if self.queue.current_index().is_some() {
                    self.start_current();
                }
            }
            Command::Seek(position) => self.seek(position),
            Command::SetVolume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(out) = self.status.output() {
                    out.set_volume(self.volume);
                }
            }
            Command::SetRepeat(mode) => self.queue.set_repeat(mode),
            Command::Stop => self.stop(),
            Command::Shutdown => {}
        }
    }

    /// Starts the queue's current track, skipping tracks that fail to open.
    fn start_current(&mut self) {
        for _ in 0..self.queue.len().max(1) {
            let Some(path) = self.queue.current().map(|p| p.to_owned()) else {
                break;
            };
            match self.open(&path) {
                Ok(()) => return,
                Err(e) => {
                    self.emit(PlayerEvent::TrackError {
                        path,
                        message: e.to_string(),
                    });
                    if self.queue.skip_next().is_none() {
                        break;
                    }
                }
            }
        }
        self.end_queue();
    }

    fn open(&mut self, path: &std::path::Path) -> Result<()> {
        self.close_active();
        let decoder = Decoder::open(path)?;
        let info = decoder.info().clone();
        let output = self.open_output(&info, Duration::ZERO)?;
        let device = output.device_name.clone();
        output.play()?;
        self.active = Some(Active {
            decoder,
            output,
            pending: Vec::new(),
            pending_offset: 0,
            decode_buf: Vec::new(),
            finished_decoding: false,
        });
        self.set_state(PlaybackState::Playing);
        self.emit(PlayerEvent::TrackStarted {
            index: self.queue.current_index().unwrap_or(0),
            path: path.to_owned(),
            info,
            device,
        });
        Ok(())
    }

    fn open_output(&mut self, info: &TrackInfo, base: Duration) -> Result<Output> {
        let shared = Arc::new(OutputShared::new(self.volume));
        let output = Output::open(info.sample_rate, info.channels, Arc::clone(&shared))?;
        self.status
            .base_ms
            .store(base.as_millis() as u64, Ordering::Relaxed);
        self.status
            .sample_rate
            .store(u64::from(output.sample_rate), Ordering::Relaxed);
        *self
            .status
            .output
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(shared);
        Ok(output)
    }

    /// Moves decoded audio into the device buffer. Returns `true` if it made
    /// progress (so the loop should call it again immediately).
    fn feed(&mut self) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };

        if active.pending_offset < active.pending.len() {
            let written = active.output.push(&active.pending[active.pending_offset..]);
            active.pending_offset += written;
            return written > 0;
        }

        if active.finished_decoding {
            if active.output.is_drained() {
                self.on_track_finished();
                return true;
            }
            return false;
        }

        active.decode_buf.clear();
        match active.decoder.next_chunk(&mut active.decode_buf) {
            Ok(true) => {
                active.pending.clear();
                active.pending_offset = 0;
                remix_into(
                    &active.decode_buf,
                    usize::from(active.decoder.info().channels),
                    usize::from(active.output.channels),
                    &mut active.pending,
                );
                true
            }
            Ok(false) => {
                active.finished_decoding = true;
                true
            }
            Err(e) => {
                // Stop decoding this track; it ends once the buffer drains.
                active.finished_decoding = true;
                self.emit_track_error(&e);
                true
            }
        }
    }

    fn emit_track_error(&self, error: &PlayerError) {
        let path = self
            .queue
            .current()
            .map(|p| p.to_owned())
            .unwrap_or_default();
        self.emit(PlayerEvent::TrackError {
            path,
            message: error.to_string(),
        });
    }

    fn on_track_finished(&mut self) {
        if self.queue.advance_after_finish().is_some() {
            self.start_current();
        } else {
            self.end_queue();
        }
    }

    fn seek(&mut self, position: Duration) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        let was_playing = self.state() == PlaybackState::Playing;
        let result = active.decoder.seek(position).and_then(|reached| {
            // Reopen the stream to drop audio already buffered at the old position.
            drop(std::mem::replace(
                &mut active.output,
                self.open_output(&active.decoder.info().clone(), reached)?,
            ));
            if was_playing {
                active.output.play()?;
            }
            Ok(reached)
        });
        active.pending.clear();
        active.pending_offset = 0;
        active.finished_decoding = false;
        match result {
            Ok(reached) => {
                self.active = Some(active);
                self.reset_stall();
                self.emit(PlayerEvent::Seeked(reached));
            }
            Err(e) => {
                self.emit_track_error(&e);
                self.active = Some(active);
            }
        }
    }

    fn pause(&mut self) {
        if let Some(active) = &self.active
            && self.state() == PlaybackState::Playing
            && active.output.pause().is_ok()
        {
            self.set_state(PlaybackState::Paused);
        }
    }

    fn resume(&mut self) {
        match &self.active {
            Some(active) => {
                if self.state() != PlaybackState::Playing && active.output.play().is_ok() {
                    self.set_state(PlaybackState::Playing);
                }
            }
            None if self.queue.current_index().is_some() => self.start_current(),
            None => {}
        }
    }

    fn stop(&mut self) {
        self.close_active();
        self.set_state(PlaybackState::Stopped);
    }

    fn end_queue(&mut self) {
        self.stop();
        self.emit(PlayerEvent::QueueEnded);
    }

    fn close_active(&mut self) {
        self.active = None;
        *self
            .status
            .output
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        self.status.base_ms.store(0, Ordering::Relaxed);
    }

    fn position(&self) -> Duration {
        self.status.position()
    }

    fn state(&self) -> PlaybackState {
        self.status.state()
    }

    /// Stops playback if the device reported an error or silently stopped
    /// consuming audio. Returns `true` if playback was stopped.
    fn output_lost(&mut self) -> bool {
        let (Some(active), Some(shared)) = (&self.active, self.status.output()) else {
            return false;
        };
        let failed = shared.has_failed();
        let stalled = !active.output.is_drained() && {
            let mut detector = self.stall.get();
            let stalled =
                detector.observe(shared.frames_played.load(Ordering::Relaxed), Instant::now());
            self.stall.set(detector);
            stalled
        };
        if !failed && !stalled {
            return false;
        }
        self.stop();
        let message = if failed {
            "audio device reported an error"
        } else {
            "audio device stopped consuming audio"
        };
        self.emit(PlayerEvent::OutputLost {
            message: message.into(),
        });
        true
    }

    fn reset_stall(&self) {
        let mut detector = self.stall.get();
        detector.reset();
        self.stall.set(detector);
    }

    fn set_state(&self, state: PlaybackState) {
        // Pauses and new streams legitimately freeze the frame counter.
        self.reset_stall();
        if self.status.state() != state {
            self.status.state.store(state as u8, Ordering::Relaxed);
            self.emit(PlayerEvent::StateChanged(state));
        }
    }

    fn emit(&self, event: PlayerEvent) {
        // Nobody listening is fine (e.g. during shutdown).
        let _ = self.events.send(event);
    }
}
