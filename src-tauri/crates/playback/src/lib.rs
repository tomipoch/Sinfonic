//! Audio playback engine.
//!
//! Phase 0: skeleton. The real implementation (rodio + dsyneq + symphonia)
//! lands in Phase 4. Keeping the heavy audio dependencies out of Fase 0
//! keeps the first `cargo check` fast.

#![allow(dead_code)]

pub mod eq;
pub mod player;
pub mod stream;

pub use player::AudioPlayer;
pub use stream::StreamHandle;
