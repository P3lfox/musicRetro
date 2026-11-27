//! Platform-agnostic core of Reproductor Retro.
//!
//! This crate must never depend on UI code so it can be reused by the
//! desktop, Android, and iOS front-ends.

pub mod system;
