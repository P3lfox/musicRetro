//! M1 command-line player for `retro_core::player`.
//!
//! Usage: `cargo run -p retro-core --example play -- <file|folder>...`
//!
//! Commands (type and press Enter):
//!
//! ```text
//!   p          play / pause
//!   n / b      next / previous track
//!   s <secs>   seek to position, e.g. `s 90`
//!   + / -      volume up / down
//!   r          cycle repeat (off / all / one)
//!   i          show position
//!   q          quit
//! ```

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use retro_core::player::info::format_clock;
use retro_core::player::{Command, Player, PlayerEvent, RepeatMode};

const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "aac", "ogg", "oga", "wav"];

fn main() {
    let tracks = collect_tracks(std::env::args_os().skip(1).map(PathBuf::from));
    if tracks.is_empty() {
        eprintln!("usage: play <audio file or folder>...");
        std::process::exit(2);
    }

    let player = match Player::spawn() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "{} track(s) queued. Commands: p n b s<secs> + - r i q",
        tracks.len()
    );
    let total = tracks.len();
    let mut durations: Vec<Option<Duration>> = vec![None; total];
    let mut current: Option<usize> = None;
    let mut volume: f32 = 1.0;
    let mut repeat = RepeatMode::Off;

    if player.send(Command::Load(tracks)).is_err() {
        return;
    }

    // Stdin is read on its own thread so events keep printing while idle.
    let (line_tx, line_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if line_tx.send(line).is_err() {
                break;
            }
        }
    });

    loop {
        while let Ok(event) = player.events().try_recv() {
            match event {
                PlayerEvent::TrackStarted {
                    index,
                    path,
                    info,
                    device,
                } => {
                    current = Some(index);
                    durations[index] = info.duration;
                    let length = info
                        .duration
                        .map(format_clock)
                        .unwrap_or_else(|| "--:--".into());
                    println!(
                        "▶ [{}/{}] {}  ({})  {}  → {}",
                        index + 1,
                        total,
                        display_name(&path),
                        length,
                        info.summary(),
                        device
                    );
                }
                PlayerEvent::StateChanged(state) => println!("  state: {state:?}"),
                PlayerEvent::Seeked(pos) => println!("  seeked to {}", format_clock(pos)),
                PlayerEvent::TrackError { path, message } => {
                    eprintln!("  ✕ {}: {message}", display_name(&path));
                }
                PlayerEvent::OutputLost { message } => {
                    eprintln!("  ✕ output lost: {message} (press p to retry)");
                }
                PlayerEvent::QueueEnded => {
                    println!("■ queue ended");
                    return;
                }
            }
        }

        let line = match line_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };

        let mut parts = line.split_whitespace();
        let command = match parts.next() {
            Some("p") => Command::TogglePause,
            Some("n") => Command::Next,
            Some("b") => Command::Previous,
            Some("s") => match parts
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
            {
                // Rejects negative, NaN, and infinite input instead of panicking.
                Some(position) => Command::Seek(position),
                _ => {
                    println!("  usage: s <seconds>");
                    continue;
                }
            },
            Some("+") => {
                volume = (volume + 0.1).min(1.0);
                println!("  volume {:.0}%", volume * 100.0);
                Command::SetVolume(volume)
            }
            Some("-") => {
                volume = (volume - 0.1).max(0.0);
                println!("  volume {:.0}%", volume * 100.0);
                Command::SetVolume(volume)
            }
            Some("r") => {
                repeat = match repeat {
                    RepeatMode::Off => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::Off,
                };
                println!("  repeat {repeat:?}");
                Command::SetRepeat(repeat)
            }
            Some("i") => {
                let length = current
                    .and_then(|i| durations[i])
                    .map(format_clock)
                    .unwrap_or_else(|| "--:--".into());
                println!(
                    "  {} / {}  ({:?})",
                    format_clock(player.position()),
                    length,
                    player.state()
                );
                continue;
            }
            Some("q") => return,
            Some(other) => {
                println!("  unknown command: {other}");
                continue;
            }
            None => continue,
        };
        if player.send(command).is_err() {
            return;
        }
    }
}

/// Expands folders (one level, sorted) and keeps files with audio extensions.
fn collect_tracks(args: impl Iterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut tracks = Vec::new();
    for path in args {
        if path.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(&path)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| is_audio(p))
                        .collect()
                })
                .unwrap_or_default();
            entries.sort();
            tracks.extend(entries);
        } else if is_audio(&path) {
            tracks.push(path);
        } else {
            eprintln!("skipping {} (not a supported audio file)", path.display());
        }
    }
    tracks
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
