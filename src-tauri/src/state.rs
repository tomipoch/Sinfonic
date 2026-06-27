//! Shared application state.
//!
//! Held in a `tokio::sync::Mutex` and passed to Tauri commands via
//! `tauri::State`. Each command takes the lock briefly, performs its
//! operation, and releases it. Long-running operations should spawn a
//! background task and notify the UI via events.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sinfonic_domain::{PlaybackState, QueueEngine, ServerId};
use sinfonic_lastfm::LastFmClient;
use sinfonic_library::{AlbumArtCache, Store};
use sinfonic_playback::AudioPlayer;
use sinfonic_secrets::KeyringStore;
use sinfonic_source::MusicProvider;
use tokio::sync::Mutex;

/// The single bag of state shared across all Tauri commands.
///
/// `QueueEngine` owns *what plays next* (entries, order, repeat,
/// shuffle). `PlaybackState` owns *what's playing right now* (is it
/// playing, where the playhead is, volume). `AudioPlayer` owns the
/// rodio audio engine — its cached state shadows `PlaybackState` but
/// it's the source of truth for `position_seconds`. `Store` owns the
/// on-disk SQLite cache of the library. `provider` holds the
/// optional active music provider (Jellyfin, Subsonic, …) — set by
/// the corresponding login command, cleared by `provider_logout`.
/// Keeping them separate follows the layering: queue is a content
/// concern, playback is a runtime concern, library is a persistence
/// concern, provider is an external-service concern.
#[derive(Clone)]
pub struct AppState {
    /// The current playback queue.
    pub queue: QueueEngine,
    /// The current playback state (playhead, volume, mute). Mirrors
    /// the AudioPlayer's cached state — kept here so the domain type
    /// stays self-contained for tests and queue logic.
    pub playback: PlaybackState,
    /// Rodio-backed audio engine. Owns the actual sink that produces
    /// sound. Cheap to clone (just an Arc bump).
    pub player: Arc<AudioPlayer>,
    /// The SQLite library cache. Shared by clone, so cloning the
    /// handle is cheap (just a pool clone).
    pub library: Store,
    /// Active music provider (only when a server is connected).
    /// `None` means the user has not logged in yet. The provider is
    /// stored as `Arc<dyn MusicProvider>` so we can swap Jellyfin,
    /// Subsonic, or any future implementation without changing
    /// command code.
    pub provider: Arc<Mutex<Option<Arc<dyn MusicProvider>>>>,
    /// OS keyring wrapper. Cloned handles share the same backend.
    pub secrets: Arc<KeyringStore>,
    /// Stable device id sent in the Jellyfin auth header. Generated
    /// once per process and kept on the state so login calls always
    /// send the same id (Jellyfin tracks devices by id and rotates
    /// tokens if it changes).
    pub device_id: String,
    /// Filesystem-backed album art cache. `None` when no data dir
    /// is available (e.g. a misconfigured mobile sandbox). Commands
    /// that need it short-circuit to a provider-direct fetch in that
    /// case so the UI still works.
    pub album_art: Option<Arc<AlbumArtCache>>,
    /// In-memory Last.fm client. Present only after `lastfm_connect`
    /// succeeds; the session key is persisted in the OS keyring so
    /// the next launch can `resume` it without re-prompting.
    pub lastfm: Arc<Mutex<Option<LastFmClient>>>,
    /// Set to `true` once the `try_restore_provider` background task
    /// finishes. The frontend polls `bootstrap_state` until this flips
    /// so the route guard can decide between the main UI and the
    /// setup view with the latest snapshot of the saved servers.
    pub bootstrap_complete: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        // Default uses an in-memory store, which is what tests and
        // the dev environment (no real cache file) want.
        let secrets = KeyringStore::new("sinfonic");
        Self {
            queue: QueueEngine::default(),
            playback: PlaybackState::default(),
            player: Arc::new(AudioPlayer::new()),
            library: Store::open_memory().expect("open_memory never fails"),
            provider: Arc::new(Mutex::new(None)),
            secrets: Arc::new(secrets),
            device_id: default_device_id(),
            album_art: None,
            lastfm: Arc::new(Mutex::new(None)),
            bootstrap_complete: Arc::new(AtomicBool::new(false)),
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
            player: Arc::new(AudioPlayer::new()),
            library,
            provider: Arc::new(Mutex::new(None)),
            secrets: Arc::new(KeyringStore::new("sinfonic")),
            device_id: default_device_id(),
            album_art: None,
            lastfm: Arc::new(Mutex::new(None)),
            bootstrap_complete: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Build an `AppState` with both the library cache and the album
    /// art cache pointed at real on-disk locations. Called from
    /// `lib.rs` when the app data directory is available.
    pub fn with_paths(
        library_path: impl AsRef<std::path::Path>,
        album_art_dir: impl AsRef<std::path::Path>,
    ) -> Result<Self, String> {
        let library = Store::open(library_path).map_err(|e| e.to_string())?;
        let album_art =
            AlbumArtCache::open(album_art_dir.as_ref()).map_err(|e| e.to_string())?;
        Ok(Self {
            queue: QueueEngine::default(),
            playback: PlaybackState::default(),
            player: Arc::new(AudioPlayer::new()),
            library,
            provider: Arc::new(Mutex::new(None)),
            secrets: Arc::new(KeyringStore::new("sinfonic")),
            device_id: default_device_id(),
            album_art: Some(Arc::new(album_art)),
            lastfm: Arc::new(Mutex::new(None)),
            bootstrap_complete: Arc::new(AtomicBool::new(false)),
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