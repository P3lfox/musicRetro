//! Playback queue: ordered tracks, current position, and repeat mode.

use std::path::{Path, PathBuf};

/// What happens when the queue runs out or a track finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    /// Stop after the last track.
    #[default]
    Off,
    /// Wrap around to the first track.
    All,
    /// Replay the current track when it finishes naturally.
    One,
}

#[derive(Debug, Default, Clone)]
pub struct Queue {
    tracks: Vec<PathBuf>,
    current: Option<usize>,
    repeat: RepeatMode,
}

impl Queue {
    pub fn new(tracks: Vec<PathBuf>) -> Self {
        let current = (!tracks.is_empty()).then_some(0);
        Self {
            tracks,
            current,
            repeat: RepeatMode::Off,
        }
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current(&self) -> Option<&Path> {
        self.current.map(|i| self.tracks[i].as_path())
    }

    /// User-requested skip forward. Returns `None` at the end of the queue
    /// (unless repeating all), keeping the last track as current.
    pub fn skip_next(&mut self) -> Option<&Path> {
        let i = self.current?;
        let next = if i + 1 < self.tracks.len() {
            i + 1
        } else if self.repeat == RepeatMode::All {
            0
        } else {
            return None;
        };
        self.current = Some(next);
        self.current()
    }

    /// User-requested skip back. Stays on the first track unless repeating all.
    pub fn skip_previous(&mut self) -> Option<&Path> {
        let i = self.current?;
        let prev = match (i, self.repeat) {
            (0, RepeatMode::All) => self.tracks.len() - 1,
            (0, _) => 0,
            (i, _) => i - 1,
        };
        self.current = Some(prev);
        self.current()
    }

    /// Called when the current track finished playing on its own.
    pub fn advance_after_finish(&mut self) -> Option<&Path> {
        match self.repeat {
            RepeatMode::One => self.current(),
            RepeatMode::Off | RepeatMode::All => self.skip_next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(n: usize) -> Queue {
        Queue::new((0..n).map(|i| PathBuf::from(format!("{i}.flac"))).collect())
    }

    fn name(p: Option<&Path>) -> Option<String> {
        p.map(|p| p.to_string_lossy().into_owned())
    }

    #[test]
    fn new_queue_starts_at_first_track() {
        let q = queue(3);
        assert_eq!(q.len(), 3);
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(name(q.current()), Some("0.flac".into()));
    }

    #[test]
    fn empty_queue_has_no_current_track() {
        let mut q = queue(0);
        assert!(q.is_empty());
        assert_eq!(q.current(), None);
        assert_eq!(q.skip_next(), None);
        assert_eq!(q.skip_previous(), None);
        assert_eq!(q.advance_after_finish(), None);
    }

    #[test]
    fn next_advances_until_the_end_without_repeat() {
        let mut q = queue(2);
        assert_eq!(name(q.skip_next()), Some("1.flac".into()));
        assert_eq!(q.skip_next(), None);
        // Stays on the last track so the UI can still show it.
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn next_wraps_with_repeat_all() {
        let mut q = queue(2);
        q.set_repeat(RepeatMode::All);
        q.skip_next();
        assert_eq!(name(q.skip_next()), Some("0.flac".into()));
    }

    #[test]
    fn previous_stops_at_first_track_without_repeat() {
        let mut q = queue(2);
        assert_eq!(name(q.skip_previous()), Some("0.flac".into()));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn previous_wraps_with_repeat_all() {
        let mut q = queue(3);
        q.set_repeat(RepeatMode::All);
        assert_eq!(name(q.skip_previous()), Some("2.flac".into()));
    }

    #[test]
    fn finish_with_repeat_one_replays_current_track() {
        let mut q = queue(3);
        q.set_repeat(RepeatMode::One);
        assert_eq!(name(q.advance_after_finish()), Some("0.flac".into()));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn user_next_with_repeat_one_still_skips() {
        let mut q = queue(3);
        q.set_repeat(RepeatMode::One);
        assert_eq!(name(q.skip_next()), Some("1.flac".into()));
    }

    #[test]
    fn finish_on_last_track_stops_without_repeat() {
        let mut q = queue(1);
        assert_eq!(q.advance_after_finish(), None);
    }

    #[test]
    fn finish_on_last_track_wraps_with_repeat_all() {
        let mut q = queue(2);
        q.set_repeat(RepeatMode::All);
        q.skip_next();
        assert_eq!(name(q.advance_after_finish()), Some("0.flac".into()));
    }
}
