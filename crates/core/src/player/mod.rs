//! Audio playback: queue, decoding, output, and the player engine.
//!
//! [`Player`] owns a dedicated `audio-engine` thread; callers send
//! [`Command`]s and receive [`PlayerEvent`]s over channels.

mod decoder;
mod engine;
mod error;
pub mod info;
mod output;
pub mod queue;
pub mod remix;

pub use decoder::Decoder;
pub use engine::{Command, PlaybackState, Player, PlayerEvent};
pub use error::{PlayerError, Result};
pub use info::TrackInfo;
pub use queue::{Queue, RepeatMode};
