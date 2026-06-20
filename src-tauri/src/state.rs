//! Shared application state.
//!
//! Held in a `tokio::sync::Mutex` and passed to Tauri commands via
//! `tauri::State`. Each command takes the lock briefly, performs its
//! operation, and releases it. Long-running operations should spawn a
//! background task and notify the UI via events.

use sinfonic_domain::{PlaybackState, QueueEngine, ServerId};

/// The single bag of state shared across all Tauri commands.
///
/// `QueueEngine` owns *what plays next* (entries, order, repeat,
/// shuffle). `PlaybackState` owns *what's playing right now* (is it
/// playing, where the playhead is, volume). Keeping them separate
/// follows the layering: queue is a content concern, playback is a
/// runtime concern.
#[derive(Default)]
pub struct AppState {
    /// The current playback queue.
    pub queue: QueueEngine,
    /// The current playback state (playhead, volume, mute).
    pub playback: PlaybackState,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience constructor for tests and seed flows that already
    /// have a `ServerId` to anchor the engine.
    pub fn with_server(server_id: ServerId) -> Self {
        Self {
            queue: QueueEngine::new(server_id),
            ..Self::default()
        }
    }
}
