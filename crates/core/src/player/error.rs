use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("cannot open {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported or corrupt audio file: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("file has no playable audio track")]
    NoAudioTrack,
    #[error("track is missing {0}")]
    MissingParameter(&'static str),
    #[error("no audio output device available")]
    NoOutputDevice,
    #[error("audio output error: {0}")]
    Output(#[from] cpal::Error),
    #[error("output device cannot play {0} Hz audio")]
    UnsupportedSampleRate(u32),
    #[error("player thread is not running")]
    Disconnected,
}

pub type Result<T> = std::result::Result<T, PlayerError>;
