//! Shared application state.
//!
//! Held in a `tokio::sync::Mutex` and passed to Tauri commands via
//! `tauri::State`. Each command takes the lock briefly, performs its
//! operation, and releases it. Long-running operations should spawn a
//! background task and notify the UI via events.

use std::sync::Arc;

use sinfonic_domain::{PlaybackState, QueueEngine, ServerId};
use sinfonic_library::Store;
use sinfonic_secrets::KeyringStore;
use sinfonic_source_jellyfin::JellyfinProvider;
use tokio::sync::Mutex;

/// The single bag of state shared across all Tauri commands.
///
/// `QueueEngine` owns *what plays next* (entries, order, repeat,
/// shuffle). `PlaybackState` owns *what's playing right now* (is it
/// playing, where the playhead is, volume). `Store` owns the
/// on-disk SQLite cache of the library (albums, artists, tracks,
/// playlists + FTS5 search index). `provider` holds the optional
/// active Jellyfin session — set by `jellyfin_login`, cleared by
/// `jellyfin_logout`. Keeping them separate follows the layering:
/// queue is a content concern, playback is a runtime concern,
/// library is a persistence concern, provider is an external-service
/// concern.
#[derive(Clone)]
pub struct AppState {
    /// The current playback queue.
    pub queue: QueueEngine,
    /// The current playback state (playhead, volume, mute).
    pub playback: PlaybackState,
    /// The SQLite library cache. Shared by clone, so cloning the
    /// handle is cheap (just a pool clone).
    pub library: Store,
    /// Active Jellyfin provider (only when a server is connected).
    /// `None` means the user has not logged in yet.
    pub provider: Arc<Mutex<Option<JellyfinProvider>>>,
    /// OS keyring wrapper. Cloned handles share the same backend.
    pub secrets: Arc<KeyringStore>,
    /// Stable device id sent in the Jellyfin auth header. Generated
    /// once per process and kept on the state so login calls always
    /// send the same id (Jellyfin tracks devices by id and rotates
    /// tokens if it changes).
    pub device_id: String,
}

impl Default for AppState {
    fn default() -> Self {
        // Default uses an in-memory store, which is what tests and
        // the dev environment (no real cache file) want.
        let secrets = KeyringStore::new("sinfonic");
        Self {
            queue: QueueEngine::default(),
            playback: PlaybackState::default(),
            library: Store::open_memory().expect("open_memory never fails"),
            provider: Arc::new(Mutex::new(None)),
            secrets: Arc::new(secrets),
            device_id: default_device_id(),
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
            provider: Arc::new(Mutex::new(None)),
            secrets: Arc::new(KeyringStore::new("sinfonic")),
            device_id: default_device_id(),
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

/// Stable per-process device id. We deliberately keep it stable for
/// the lifetime of the process so two logins from the same app
/// instance look like the same device to Jellyfin.
fn default_device_id() -> String {
    // The keyring backend doesn't expose a process-wide device id, so
    // we use a UUIDv4-like value built from the process id and a
    // random suffix. Real platforms can swap this for `uuid::Uuid::new_v4()`.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sinfonic-{pid}-{nanos:x}")
}