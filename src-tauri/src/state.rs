//! Shared application state.
//!
//! Held in a `tokio::sync::Mutex` and passed to Tauri commands via
//! `tauri::State`. Each command takes the lock briefly, performs its
//! operation, and releases it. Long-running operations should spawn a
//! background task and notify the UI via events.

use sinfonic_domain::{PlaybackState, QueueEngine, ServerId};
use sinfonic_library::Store;

/// The single bag of state shared across all Tauri commands.
///
/// `QueueEngine` owns *what plays next* (entries, order, repeat,
/// shuffle). `PlaybackState` owns *what's playing right now* (is it
/// playing, where the playhead is, volume). `Store` owns the
/// on-disk SQLite cache of the library (albums, artists, tracks,
/// playlists + FTS5 search index). Keeping them separate follows
/// the layering: queue is a content concern, playback is a runtime
/// concern, library is a persistence concern.
#[derive(Clone)]
pub struct AppState {
    /// The current playback queue.
    pub queue: QueueEngine,
    /// The current playback state (playhead, volume, mute).
    pub playback: PlaybackState,
    /// The SQLite library cache. Shared by clone, so cloning the
    /// handle is cheap (just a pool clone).
    pub library: Store,
}

impl Default for AppState {
    fn default() -> Self {
        // Default uses an in-memory store, which is what tests and
        // the dev environment (no real cache file) want.
        Self {
            queue: QueueEngine::default(),
            playback: PlaybackState::default(),
            library: Store::open_memory().expect("open_memory never fails"),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an `AppState` with the library cache pointed at a real
    /// on-disk file. Called from `lib.rs` on app startup.
    pub fn with_library_path(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let library = Store::open(path).map_err(|e| e.to_string())?;
        Ok(Self {
            queue: QueueEngine::default(),
            playback: PlaybackState::default(),
            library,
        })
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
