//! Shared application state.
//!
//! Held in a `tokio::sync::Mutex` and passed to Tauri commands via
//! `tauri::State`. Each command takes the lock briefly, performs its
//! operation, and releases it. Long-running operations should spawn a
//! background task and notify the UI via events.

use sinfonic_domain::QueueEngine;

/// The single bag of state shared across all Tauri commands.
#[derive(Default)]
pub struct AppState {
    /// The current playback queue. Phase 4 wires the real `QueueEngine`.
    pub queue: QueueEngine,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}
