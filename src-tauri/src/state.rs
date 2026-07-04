//! Shared application state.
//!
//! Held in a `tokio::sync::Mutex` and passed to Tauri commands via
//! `tauri::State`. Each command takes the lock briefly, performs its
//! operation, and releases it. Long-running operations should spawn a
//! background task and notify the UI via events.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sinfonic_domain::{QueueEngine, ServerId};
use sinfonic_lastfm::LastFmClient;
use sinfonic_library::{AlbumArtCache, Store};
use sinfonic_lyrics::LrclibClient;
use sinfonic_playback::AudioPlayer;
use sinfonic_secrets::KeyringStore;
use sinfonic_source::MusicProvider;
use sinfonic_source_subsonic::SubsonicProvider;
use tokio::sync::Mutex;

/// The single bag of state shared across all Tauri commands.
///
/// `QueueEngine` owns *what plays next* (entries, order, repeat,
/// shuffle). `AudioPlayer` owns the rodio audio engine and is the
/// single source of truth for runtime playback state (position,
/// volume, mute, is-playing). `Store` owns the on-disk SQLite cache
/// of the library. `provider` holds the optional active music
/// provider (Jellyfin, Subsonic, …) — set by the corresponding login
/// command, cleared by `provider_logout`. Keeping them separate
/// follows the layering: queue is a content concern, playback is a
/// runtime concern, library is a persistence concern, provider is an
/// external-service concern.
#[derive(Clone)]
pub struct AppState {
    /// The current playback queue.
    pub queue: QueueEngine,
    /// Rodio-backed audio engine. Owns the actual sink that produces
    /// sound and is the single source of truth for runtime playback
    /// state (`cached_state()` reflects the rodio sink). Cheap to
    /// clone (just an Arc bump).
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
    /// Typed `Arc<SubsonicProvider>` mirror of `provider`, set only
    /// when the active provider is a Subsonic server. Used by
    /// `commands::kick_subsonic_background_sync` to call
    /// `SubsonicProvider::sync_album_tracks` — a Subsonic-specific
    /// helper that lives outside the `MusicProvider` trait because
    /// no other source needs the album-fan-out pattern. Kept in
    /// lockstep with `provider`: cleared on logout / server switch,
    /// re-populated on subsonic login / activate.
    pub subsonic: Arc<Mutex<Option<Arc<SubsonicProvider>>>>,
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
    /// LRCLIB lyrics lookup client. Always present — it has no
    /// state and is safe to share globally. `commands::get_lyrics`
    /// uses it as the fallback when no music provider (Subsonic,
    /// Jellyfin, local) ships lyrics for the current track.
    pub lyrics_client: Arc<LrclibClient>,
    /// Set to `true` once the `try_restore_provider` background task
    /// finishes. The frontend polls `bootstrap_state` until this flips
    /// so the route guard can decide between the main UI and the
    /// setup view with the latest snapshot of the saved servers.
    pub bootstrap_complete: Arc<AtomicBool>,
    /// Pause-switch for the queue-snapshot persist path. Set to
    /// `true` around server-switch / logout teardowns so the
    /// `queue.clear()` in `teardown_active_provider` doesn't
    /// overwrite the previous server's persisted snapshot with the
    /// now-empty queue. The flag is shared via `Arc` so the
    /// commands that hold a clone of the state see the same value
    /// the teardown helper set.
    pub persist_guard: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        // Default uses an in-memory store, which is what tests and
        // the dev environment (no real cache file) want.
        let secrets = KeyringStore::new("sinfonic");
        let lyrics_client = Arc::new(build_lrclib_client());
        Self {
            queue: QueueEngine::default(),
            player: Arc::new(AudioPlayer::new()),
            library: Store::open_memory().expect("open_memory never fails"),
            provider: Arc::new(Mutex::new(None)),
            subsonic: Arc::new(Mutex::new(None)),
            secrets: Arc::new(secrets),
            device_id: default_device_id(),
            album_art: None,
            lastfm: Arc::new(Mutex::new(None)),
            lyrics_client,
            bootstrap_complete: Arc::new(AtomicBool::new(false)),
            persist_guard: Arc::new(AtomicBool::new(false)),
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
            player: Arc::new(AudioPlayer::new()),
            library,
            provider: Arc::new(Mutex::new(None)),
            subsonic: Arc::new(Mutex::new(None)),
            secrets: Arc::new(KeyringStore::new("sinfonic")),
            device_id: default_device_id(),
            album_art: None,
            lastfm: Arc::new(Mutex::new(None)),
            lyrics_client: Arc::new(build_lrclib_client()),
            bootstrap_complete: Arc::new(AtomicBool::new(false)),
            persist_guard: Arc::new(AtomicBool::new(false)),
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
            player: Arc::new(AudioPlayer::new()),
            library,
            provider: Arc::new(Mutex::new(None)),
            subsonic: Arc::new(Mutex::new(None)),
            secrets: Arc::new(KeyringStore::new("sinfonic")),
            device_id: default_device_id(),
            album_art: Some(Arc::new(album_art)),
            lastfm: Arc::new(Mutex::new(None)),
            lyrics_client: Arc::new(build_lrclib_client()),
            bootstrap_complete: Arc::new(AtomicBool::new(false)),
            persist_guard: Arc::new(AtomicBool::new(false)),
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

/// Default LRCLIB client pointing at the public service. Tests that
/// want to mock the upstream should build their own client (see
/// `src-tauri/tests/lyrics_fallback.rs`) and override the field — we
/// can't change `base_url` post-construction.
fn build_lrclib_client() -> LrclibClient {
    let base_url: url::Url = "https://lrclib.net"
        .parse()
        .expect("lrclib.net is a valid URL");
    LrclibClient::new(base_url, env!("CARGO_PKG_VERSION").to_string())
        .expect("LRCLIB client with sensible defaults always builds")
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