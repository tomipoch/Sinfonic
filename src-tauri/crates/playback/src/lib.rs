//! Audio playback engine.
//!
//! `AudioPlayer` owns a rodio output stream and sink. The high-level API:
//!
//! - [`AudioPlayer::new`] — open the default audio device (silent fallback
//!   if no device is present, e.g. headless CI).
//! - [`AudioPlayer::play`] — start a track given a stream URI.
//! - [`AudioPlayer::pause`], [`AudioPlayer::resume`], [`AudioPlayer::stop`].
//! - [`AudioPlayer::seek`], [`AudioPlayer::set_volume`], [`AudioPlayer::set_muted`].
//! - [`AudioPlayer::set_eq_band`], [`AudioPlayer::reset_eq`] — 10-band graphic EQ.
//! - [`AudioPlayer::set_event_callback`] — wire [`PlayerEvent`]s to the UI.
//!
//! Streams are decoded via [`rodio::Decoder`] (backed by Symphonia).
//! HTTP sources are downloaded into memory and decoded from there;
//! local files are read straight from disk.

pub mod eq;
pub mod player;
pub mod stream;

mod biquad;

pub use eq::{BandGain, Equalizer, SharedEqualizer};
pub use player::{AudioPlayer, PlayerError, PlayerEvent};
pub use stream::{StreamError, StreamHandle};

/// Re-export of rodio's [`Source`] trait so downstream crates don't
/// need to add rodio as a dependency just to iterate samples.
pub use rodio::Source;