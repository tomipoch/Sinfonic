//! Tauri command surface.
//!
//! Every command returns `Result<T, String>` so we can send
//! human-readable error messages to the frontend without a custom
//! `Serialize` impl on each error type. This is the pattern recommended
//! by the tauri-v2 skill and keeps the IPC boundary trivial.
//!
//! # Phase status
//!
//! - Phase 1: the queue + playback commands operate on the
//!   in-memory `AppState` and emit real events.
//! - Phase 2: library reads (`get_albums`, `get_artists`,
//!   `get_tracks`, `search`) are wired against the real SQLite
//!   cache. They return real data. The default `server_id` is
//!   `"server-local"` until a Jellyfin login provides the real one.
//! - Phase 3: Jellyfin provider + login flow. Library reads now
//!   prefer the active provider's `ServerId` if a session is
//!   connected, falling back to the placeholder otherwise.
//! - Phase 4: `play_track` resolves the track's stream URI from the
//!   active provider and pipes it through `AudioPlayer` (rodio +
//!   Symphonia + 10-band EQ). `pause`, `resume`, `seek`,
//!   `set_volume`, `set_muted`, `set_eq_band`, `reset_eq` all drive
//!   the AudioPlayer too. `get_playback_state` reflects the rodio
//!   sink's position.
//! - Phase 5: the active provider is stored as `Arc<dyn MusicProvider>`,
//!   so Jellyfin and Subsonic share a single `provider_sync_library`
//!   command. `jellyfin_login` and `subsonic_login` build their
//!   respective providers and the dispatcher just calls
//!   `provider.albums(...).await`.

use rusqlite as sqlite;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::events::{
    EventName, LibrarySyncStatusPayload, PlaybackConfigPayload, PlaybackStatePayload,
    QueueSnapshotPayload, TrackChangedPayload,
};
use crate::lastfm;
use crate::state::AppState;
use sinfonic_domain::{
    Album, AlbumDetail, AlbumId, Artist, ArtistId, Genre, ImageKind, PagedResponse, Playlist,
    PlaylistDetail, PlaylistId, QueueEntryId, QueueSnapshot, RepeatMode, SearchResults, ServerId,
    SmartPlaylist, SmartPlaylistId, SmartPlaylistRuleField, SmartPlaylistRuleOperator,
    SmartPlaylistSortDirection, SmartPlaylistSortField, Track, TrackId,
};
use sinfonic_library::ImageCacheKey;
use sinfonic_secrets::SecretStore;
use sinfonic_source::MusicProvider;
use sinfonic_source::{ImageBytes, ImageRequest, Lyrics};
use sinfonic_source_jellyfin::auth::{login as jellyfin_login_inner, LoginRequest as JellyfinAuthRequest};
use sinfonic_source_subsonic::auth::{login as subsonic_login_inner, LoginRequest as SubsonicAuthRequest};

type SharedState<'a> = State<'a, Arc<Mutex<AppState>>>;

/// Placeholder `ServerId` used when no provider is active. Library
/// reads return empty pages in that state instead of erroring — the
/// UI surfaces a "connect a server" hint.
const DEFAULT_SERVER_ID: &str = "server-local";

fn default_server_id() -> ServerId {
    ServerId::new(DEFAULT_SERVER_ID)
}

/// Return the `ServerId` of the active provider if one is connected,
/// otherwise the placeholder. Used by library reads so they
/// automatically follow the active session.
async fn active_server_id(state: &SharedState<'_>) -> ServerId {
    let guard = state.lock().await;
    let provider_guard = guard.provider.lock().await;
    let server_id = provider_guard
        .as_ref()
        .map(|p| p.identity().server_id.clone())
        .unwrap_or_else(default_server_id);
    drop(provider_guard);
    server_id
}

// ─── Private playback helpers ───────────────────────────────────
//
// Centralises the "switch the rodio sink to a new track" pipeline
// that was previously duplicated across `play_track`, `play_album`,
// `next`, `previous` and `advance_queue_on_end`. Adding a new event
// or a new source-resolution step now touches one function instead
// of four.
//
// Centralises the "tear down the active provider" pipeline that was
// previously duplicated across `provider_logout`,
// `provider_set_active`, and `provider_delete`. The three call sites
// must stop playback, clear the queue (track ids from the previous
// provider would never resolve against the next one), and notify the
// frontend so the PlayerBar / QueuePanel refresh without waiting for
// the next user action.

mod playback_helpers {
    use super::*;

    /// Resolve a `Track`'s stream URI through the active provider and
    /// start playing it. Returns `None` if no provider is connected
    /// (offline browsing + tests); callers handle that by skipping
    /// the audio path and emitting the state event anyway.
    async fn resolve_track_uri_by_id(
        state: &Arc<Mutex<AppState>>,
        track_id: &TrackId,
    ) -> Option<String> {
        let guard = state.lock().await;
        let provider_guard = guard.provider.lock().await;
        let provider = provider_guard.as_ref()?;
        let descriptor = provider.stream(track_id).await.ok()?;
        Some(descriptor.uri().to_string())
    }

    /// Common body shared by `play_track`, `play_album`, `next`,
    /// `previous`, and `advance_queue_on_end`. Resolves the stream
    /// URI, hands it to `AudioPlayer::play`, then emits
    /// `queue-changed` + `track-changed` + `playback-state-changed`
    /// so every listener sees the same snapshot. Failures degrade
    /// gracefully: a missing URI or a `player.play` error still
    /// emits the events with the cached duration so the seekbar
    /// stays in sync.
    ///
    /// When crossfade is enabled in `AudioPlayer`, we call
    /// `preload_next` first so the fade can start the moment
    /// `play` consumes the preloaded sink. When crossfade is
    /// disabled `preload_next` is a no-op (it decodes and drops
    /// the source), so the only added cost is the extra decode.
    pub(super) async fn play_entry_from_queue_entry(
        app: &tauri::AppHandle,
        state: &Arc<Mutex<AppState>>,
        entry: sinfonic_domain::QueueEntry,
    ) {
        let stream_uri = resolve_track_uri_by_id(state, &entry.track_id).await;
        let _ = match stream_uri.as_deref() {
            Some(uri) => {
                // Preload for the upcoming fade (no-op when off).
                let _ = {
                    let guard = state.lock().await;
                    guard.player.preload_next(entry.track_id.clone(), uri).await
                };
                let guard = state.lock().await;
                match guard.player.play(entry.track_id.clone(), uri).await {
                    Ok(duration) => duration,
                    Err(e) => {
                        tracing::warn!(
                            target: "sinfonic::playback",
                            error = %e,
                            "play_entry_from_queue_entry: player.play failed"
                        );
                        entry.duration_seconds
                    }
                }
            }
            None => entry.duration_seconds,
        };
        emit_queue_changed(app, state).await;
        emit_track_changed_from_entry(app, &entry);
        emit_playback_state(app, state).await;
    }

    /// Same as `play_entry_from_queue_entry` but starts from a `Track`
    /// (used by `play_track` and `play_album`, where the queue entry
    /// hasn't been created yet at the call site). Keeps a single
    /// emission pipeline so adding a new event touches one helper.
    pub(super) async fn play_track_and_emit(
        app: &tauri::AppHandle,
        state: &Arc<Mutex<AppState>>,
        track: &Track,
    ) {
        let stream_uri = resolve_track_uri_by_id(state, &track.id).await;
        let _ = match stream_uri.as_deref() {
            Some(uri) => {
                let _ = {
                    let guard = state.lock().await;
                    guard.player.preload_next(track.id.clone(), uri).await
                };
                let guard = state.lock().await;
                match guard.player.play(track.id.clone(), uri).await {
                    Ok(duration) => duration,
                    Err(e) => {
                        tracing::warn!(
                            target: "sinfonic::playback",
                            error = %e,
                            "play_track_and_emit: player.play failed"
                        );
                        track.duration_seconds
                    }
                }
            }
            None => track.duration_seconds,
        };
        emit_playback_state(app, state).await;
    }
}

mod provider_helpers {
    use super::*;

    /// Borrow a clone of the active `Arc<dyn MusicProvider>` from the
    /// app state without holding the AppState mutex across the
    /// network call. Returns `None` when no provider is connected —
    /// the caller surfaces that as an empty page or `null` so the
    /// UI renders the "connect a server" hint instead of erroring.
    ///
    /// Centralised here so the provider-direct read commands
    /// (`provider_list_albums` etc.) follow the same lock pattern.
    pub(super) async fn current_provider(
        state: &Arc<Mutex<AppState>>,
    ) -> Option<Arc<dyn MusicProvider>> {
        let guard = state.lock().await;
        let provider = guard.provider.lock().await.as_ref().cloned();
        provider
    }

    /// Install `provider` as the active provider and clear the
    /// typed `subsonic` slot. Centralises the field-write so the
    /// login, set_active and restore call sites can't drift apart.
    /// Caller already holds the AppState mutex from the surrounding
    /// command; we only need the inner `provider` / `subsonic`
    /// locks here.
    pub(super) async fn install_provider(
        guard: &AppState,
        provider: Arc<dyn MusicProvider>,
    ) {
        *guard.provider.lock().await = Some(provider);
        *guard.subsonic.lock().await = None;
    }

    /// Same as `install_provider` but also stores the typed
    /// `SubsonicProvider` so `commands::kick_subsonic_background_sync`
    /// can reach Subsonic-specific helpers that don't live on the
    /// `MusicProvider` trait.
    pub(super) async fn install_subsonic_provider(
        guard: &AppState,
        typed: Arc<sinfonic_source_subsonic::SubsonicProvider>,
    ) {
        let dyn_provider: Arc<dyn MusicProvider> = typed.clone();
        *guard.provider.lock().await = Some(dyn_provider);
        *guard.subsonic.lock().await = Some(typed);
    }

    /// Stop the rodio sink, clear the queue (track ids from the
    /// previous provider would never resolve against the next one),
    /// and notify the frontend so the PlayerBar / QueuePanel refresh
    /// without waiting for the next user action. Used by
    /// `provider_logout`, `provider_set_active` and
    /// `provider_delete` (when the deleted server was active).
    pub(super) async fn teardown_active_provider(
        app: &tauri::AppHandle,
        state: &Arc<Mutex<AppState>>,
    ) {
        // Pause the queue-snapshot persist path so the upcoming
        // `queue.clear()` doesn't overwrite the previous server's
        // persisted snapshot with an empty queue. The next
        // user-driven mutation (play, add, jump, …) will re-enable
        // persistence via `persist_queue`'s guard check.
        {
            let guard = state.lock().await;
            guard.persist_guard.store(true, std::sync::atomic::Ordering::Release);
        }
{
        let mut guard = state.lock().await;
            guard.player.stop();
            guard.queue.clear();
            guard.queue.clear_server_id();
            // Phase 3: drop the typed Subsonic slot alongside the
            // generic provider so a future Subsonic login gets a
            // fresh handle and the background sync starts from
            // scratch.
            *guard.subsonic.lock().await = None;
        }
        emit_queue_changed(app, state).await;
        emit_playback_state(app, state).await;
        {
            let guard = state.lock().await;
            guard.persist_guard.store(false, std::sync::atomic::Ordering::Release);
        }
    }
}

// ─── Library queries (Phase 2) ──────────────────────────────────

#[tauri::command]
pub async fn get_albums(
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Album>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_albums(&server_id, offset, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_artists(
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Artist>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_artists(&server_id, offset, limit)
        .map_err(|e| e.to_string())
}

/// Distinct genres for the active server, computed from the
/// `album_genres` join table. Each row carries a count of distinct
/// albums and a (rolled-up) count of tracks under that genre.
#[tauri::command]
pub async fn get_genres(
    state: SharedState<'_>,
) -> Result<Vec<Genre>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_genres(&server_id)
        .map_err(|e| e.to_string())
}

/// Paged list of albums carrying the given genre tag. Used by the
/// genre detail view. `genre` is the raw genre string (the cache
/// stores genres as plain text, case-insensitive match).
#[tauri::command]
pub async fn get_albums_by_genre(
    genre: String,
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Album>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_albums_by_genre(&server_id, &genre, offset, limit)
        .map_err(|e| e.to_string())
}

/// Paged list of tracks under the given genre. Joins through
/// `album_genres` (per-track genres are not stored in the schema
/// today). Used by the genre detail view's tracks section.
#[tauri::command]
pub async fn get_tracks_by_genre(
    genre: String,
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Track>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_tracks_by_genre(&server_id, &genre, offset, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_tracks(
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Track>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_tracks(&server_id, offset, limit)
        .map_err(|e| e.to_string())
}

/// Album detail: the album row plus its tracks in disc/track-number
/// order. Reads from the SQLite cache (same scope as `get_albums`),
/// so the detail view works offline after a sync. Returns `None`
/// for the album when the cache doesn't know about that id (the
/// frontend shows a "not found" state instead of erroring).
#[tauri::command]
pub async fn get_album_detail(
    album_id: String,
    state: SharedState<'_>,
) -> Result<Option<AlbumDetail>, String> {
    let server_id = active_server_id(&state).await;
    let parsed = AlbumId::try_new(album_id).map_err(|e| e.to_string())?;
    let guard = state.lock().await;
    let album = guard
        .library
        .get_album(&server_id, &parsed)
        .map_err(|e| e.to_string())?;
    let Some(album) = album else {
        return Ok(None);
    };
    let tracks = guard
        .library
        .list_album_tracks(&server_id, &parsed)
        .map_err(|e| e.to_string())?;
    Ok(Some(AlbumDetail { album, tracks }))
}

/// Single album row by id. Cheap lookup used by views that only
/// need the cover (e.g. resolving a track's parent album when it
/// wasn't in the first page of `get_albums`). Skips the track list
/// to keep the payload small.
#[tauri::command]
pub async fn get_album(
    album_id: String,
    state: SharedState<'_>,
) -> Result<Option<Album>, String> {
    let server_id = active_server_id(&state).await;
    let parsed = AlbumId::try_new(album_id).map_err(|e| e.to_string())?;
    let guard = state.lock().await;
    let album = guard
        .library
        .get_album(&server_id, &parsed)
        .map_err(|e| e.to_string())?;
    Ok(album)
}

// ─── Provider-direct reads (Phase 1 of feature/direct-fetch) ─────
//
// Each command resolves the active `Arc<dyn MusicProvider>` from the
// app state and calls the matching trait method directly. The page
// arrives from the upstream server, not from the SQLite cache — so
// the UI is usable as soon as the first HTTP response lands and we
// don't need the full `provider_sync_library` to finish.
//
// The existing `get_albums` / `get_artists` / `get_tracks` commands
// stay in place as the offline / cached-reads path. The frontend
// picks between them per-view (the new wrappers below).
//
// No provider is connected → return an empty page with total = 0
// instead of erroring. The UI already surfaces a "connect a server"
// hint in that state; raising a `Result::Err` would trigger a toast
// on every cold start.

#[tauri::command]
pub async fn provider_list_albums(
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Album>, String> {
    let Some(provider) = provider_helpers::current_provider(state.inner()).await else {
        return Ok(PagedResponse::new(Vec::new(), 0));
    };
    let req = sinfonic_domain::PagedRequest::new(offset, limit);
    provider.albums(req).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn provider_list_artists(
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Artist>, String> {
    let Some(provider) = provider_helpers::current_provider(state.inner()).await else {
        return Ok(PagedResponse::new(Vec::new(), 0));
    };
    let req = sinfonic_domain::PagedRequest::new(offset, limit);
    provider.artists(req).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn provider_list_tracks(
    offset: usize,
    limit: usize,
    state: SharedState<'_>,
) -> Result<PagedResponse<Track>, String> {
    let server_id = active_server_id(&state).await;
    // Phase 3 of feature/direct-fetch-providers: Subsonic has no
    // "list every track" endpoint so we serve the request from the
    // SQLite cache populated by `kick_subsonic_background_sync`.
    // While the background sync is running, the cache is partial
    // and the user sees only the tracks already ingested — the
    // `sync-progress` event keeps the UI honest about that.
    //
    // The trait-object `provider.tracks(...)` path is still
    // available as a fallback for Subsonic (used by the background
    // sync itself and by the legacy fan-out window path), but the
    // UI no longer hits it directly.
    let guard = state.lock().await;
    let provider_kind = guard
        .provider
        .lock()
        .await
        .as_ref()
        .map(|p| p.identity().provider_id.clone());
    if provider_kind.as_deref() == Some("subsonic") {
        return guard
            .library
            .list_tracks(&server_id, offset, limit)
            .map_err(|e| e.to_string());
    }
    drop(guard);
    let Some(provider) = provider_helpers::current_provider(state.inner()).await else {
        return Ok(PagedResponse::new(Vec::new(), 0));
    };
    let req = sinfonic_domain::PagedRequest::new(offset, limit);
    provider.tracks(req).await.map_err(|e| e.to_string())
}

/// Album with its tracks resolved straight from the active provider.
/// Returns `Ok(None)` when no provider is connected or the album id
/// doesn't resolve to a row — same shape as `get_album_detail` so
/// the view layer can swap the wrapper with no further changes.
#[tauri::command]
pub async fn provider_album_detail(
    album_id: String,
    state: SharedState<'_>,
) -> Result<Option<AlbumDetail>, String> {
    let Some(provider) = provider_helpers::current_provider(state.inner()).await else {
        return Ok(None);
    };
    let parsed = AlbumId::try_new(album_id).map_err(|e| e.to_string())?;
    match provider.album_detail(&parsed).await {
        Ok(resp) => Ok(Some(resp.detail)),
        Err(sinfonic_source::ProviderError::NotFound) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ─── Subsonic background sync (Phase 3) ───────────────────────────
//
// Subsonic's API has no "list every track" endpoint, so the
// provider-direct `/songs` view needs every album's tracks cached
// in SQLite first. `kick_subsonic_background_sync` spawns a tokio
// task that fans out `getAlbum` for every album on the server and
// upserts the resulting tracks into the SQLite `tracks` table —
// emitting the existing `library-sync-status` event stream so the
// UI can show "Sincronizando canciones (X / Y)…" while it runs.
//
// The task is intentionally fire-and-forget: this Tauri command
// returns immediately after spawning so the loading flow can move
// on. A subsequent `kick_subsonic_background_sync` while one is
// already running is a no-op (the running task owns the
// `in_progress` flag).

/// In-process guard: `true` while a Subsonic background sync is
/// running. Stored as `AtomicBool` so the `kick_*` command and the
/// spawned task can race-safely check it without holding the
/// AppState mutex.
static SUBSONIC_BACKGROUND_SYNC_IN_PROGRESS: AtomicBool =
    AtomicBool::new(false);

/// Phase 3 entry point. Spawns the background sync and returns
/// immediately. Errors are logged inside the task — the command
/// returns `Ok(())` whenever the provider is not Subsonic or the
/// task was already running so callers (frontend) don't have to
/// handle transient states.
#[tauri::command]
pub async fn kick_subsonic_background_sync(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    kick_subsonic_background_sync_inner(app, state.inner().clone()).await
}

/// Same body as the Tauri command but takes the raw
/// `Arc<Mutex<AppState>>` so internal callers (subsonic_login,
/// provider_set_active, try_restore_provider) can fire the
/// background sync without going through the IPC layer.
async fn kick_subsonic_background_sync_inner(
    app: tauri::AppHandle,
    state: Arc<Mutex<AppState>>,
) -> Result<(), String> {
    // Reject re-entry: only one sync at a time.
    if SUBSONIC_BACKGROUND_SYNC_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::debug!(
            target: "sinfonic::commands",
            "subsonic background sync already running, ignoring kick"
        );
        return Ok(());
    }

    let (typed_provider, library_handle, server_id) = {
        let guard = state.lock().await;
        let typed = guard.subsonic.lock().await.as_ref().cloned();
        let library = guard.library.clone();
        let server_id = guard
            .provider
            .lock()
            .await
            .as_ref()
            .map(|p| p.identity().server_id.clone())
            .unwrap_or_else(default_server_id);
        (typed, library, server_id)
    };

    let Some(typed) = typed_provider else {
        SUBSONIC_BACKGROUND_SYNC_IN_PROGRESS.store(false, Ordering::Release);
        return Ok(());
    };

    // Emit the 'started' state so the sidebar indicator appears.
    let _ = app.emit(
        EventName::LibrarySyncStatus.as_str(),
        crate::events::LibrarySyncStatusPayload {
            server_id: Some(server_id.to_string()),
            state: "started".into(),
            progress: 0.0,
        },
    );

    tokio::spawn(async move {
        let result = run_subsonic_background_sync(typed.clone(), library_handle.clone(), server_id.clone(), app.clone()).await;
        SUBSONIC_BACKGROUND_SYNC_IN_PROGRESS.store(false, Ordering::Release);
        if let Err(e) = result {
            tracing::warn!(
                target: "sinfonic::commands",
                error = %e,
                "subsonic background sync failed"
            );
            let _ = app.emit(
                EventName::LibrarySyncStatus.as_str(),
                crate::events::LibrarySyncStatusPayload {
                    server_id: Some(server_id.to_string()),
                    state: "error".into(),
                    progress: 0.0,
                },
            );
        }
    });

    Ok(())
}

/// Worker body for `kick_subsonic_background_sync`. Pulls every
/// album's tracks through the provider's fan-out helper and writes
/// them into the SQLite cache so subsequent `provider_list_tracks`
/// calls are instant reads.
async fn run_subsonic_background_sync(
    provider: Arc<sinfonic_source_subsonic::SubsonicProvider>,
    library: sinfonic_library::Store,
    server_id: ServerId,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Track which tracks we've written so the final 'complete'
    // event can report the actual count. Lives in an Arc so the
    // Fn callbacks in `sync_album_tracks` can bump it without
    // holding a mutable borrow on the outer function frame.
    let total_tracks_written = Arc::new(AtomicUsize::new(0));
    let server_id_arc = Arc::new(server_id);

    let stats = provider
        .sync_album_tracks(
            {
                let total = Arc::clone(&total_tracks_written);
                let library = library.clone();
                let server_id = Arc::clone(&server_id_arc);
                move |album, tracks| {
                    if tracks.is_empty() {
                        return;
                    }
                    // The SQLite schema requires:
                    //   tracks.album_id  → albums.album_id
                    //   tracks.artist_id → artists.artist_id (nullable)
                    //   albums.artist_id → artists.artist_id (nullable)
                    // Insert artists → album → tracks within each batch.
                    //
                    // Synthesise Artist rows from the album + track
                    // payloads. Only `id` and `name` matter for FK
                    // satisfaction; album_count / track_count are
                    // recomputed separately by the manual sync path
                    // and are not on the critical path here.
                    let mut artist_seen: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    let mut artists_to_upsert: Vec<sinfonic_domain::Artist> = Vec::new();
                    if let Some(aid) = album.artist_id.as_ref() {
                        if artist_seen.insert(aid.as_str().to_string()) {
                            artists_to_upsert.push(sinfonic_domain::Artist {
                                id: aid.clone(),
                                name: album.artist.clone(),
                                album_count: 0,
                                track_count: 0,
                                favorite: false,
                                image_ref: None,
                            });
                        }
                    }
                    for track in &tracks {
                        if let Some(aid) = track.artist_id.as_ref() {
                            if artist_seen.insert(aid.as_str().to_string()) {
                                artists_to_upsert.push(sinfonic_domain::Artist {
                                    id: aid.clone(),
                                    name: track.artist.clone(),
                                    album_count: 0,
                                    track_count: 0,
                                    favorite: false,
                                    image_ref: None,
                                });
                            }
                        }
                    }
                    if !artists_to_upsert.is_empty() {
                        if let Err(e) =
                            library.upsert_artists(&server_id, &artists_to_upsert)
                        {
                            eprintln!(
                                "sinfonic::commands subsonic background sync: upsert_artists failed: {e}"
                            );
                            return;
                        }
                    }
                    if let Err(e) = library.upsert_album(&server_id, &album) {
                        eprintln!(
                            "sinfonic::commands subsonic background sync: upsert_album failed: {e}"
                        );
                        return;
                    }
                    if let Err(e) = library.upsert_tracks(&server_id, &tracks) {
                        eprintln!(
                            "sinfonic::commands subsonic background sync: upsert_tracks failed: {e}"
                        );
                        return;
                    }
                    total.fetch_add(tracks.len(), Ordering::Relaxed);
                }
            },
            |hint_id, err| {
                eprintln!(
                    "sinfonic::commands subsonic background sync: album {} failed: {}",
                    hint_id, err
                );
            },
        )
        .await
        .map_err(|e| format!("sync_album_tracks failed: {e}"))?;

    let total = total_tracks_written.load(Ordering::Relaxed);
    tracing::info!(
        target: "sinfonic::commands",
        albums_total = stats.albums_total,
        albums_failed = stats.albums_failed,
        tracks_total = total,
        "subsonic background sync complete"
    );

    let _ = app.emit(
        EventName::LibrarySyncStatus.as_str(),
        crate::events::LibrarySyncStatusPayload {
            server_id: Some(server_id_arc.to_string()),
            state: "complete".into(),
            progress: 1.0,
        },
    );
    Ok(())
}

/// Artist with their albums resolved from the active provider.
/// Returns `Ok(None)` when no provider is connected or the artist id
/// is unknown.
#[tauri::command]
pub async fn provider_artist_detail(
    artist_id: String,
    state: SharedState<'_>,
) -> Result<Option<sinfonic_domain::ArtistDetail>, String> {
    let Some(provider) = provider_helpers::current_provider(state.inner()).await else {
        return Ok(None);
    };
    let parsed = ArtistId::try_new(artist_id).map_err(|e| e.to_string())?;
    match provider.artist_detail(&parsed).await {
        Ok(resp) => Ok(Some(resp.detail)),
        Err(sinfonic_source::ProviderError::NotFound) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Playlist with its tracks resolved from the active provider.
/// Returns `Ok(None)` when no provider is connected, the provider
/// doesn't support playlist reads, or the playlist id is unknown.
#[tauri::command]
pub async fn provider_playlist_detail(
    playlist_id: String,
    state: SharedState<'_>,
) -> Result<Option<PlaylistDetail>, String> {
    let Some(provider) = provider_helpers::current_provider(state.inner()).await else {
        return Ok(None);
    };
    let parsed: PlaylistId = playlist_id.into();
    match provider.playlist_detail(&parsed).await {
        Ok(detail) => Ok(Some(detail)),
        Err(sinfonic_source::ProviderError::NotFound) => Ok(None),
        Err(sinfonic_source::ProviderError::Unsupported(_)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Replace the queue with `tracks` and start playing the first one
/// in one atomic step. `play_track` only handles a single track and
/// would clobber the queue; `queue_play_now` queues without
/// starting playback. `play_album` is the right shape for the
/// "Play album" button in the UI.
#[tauri::command]
pub async fn play_album(
    tracks: Vec<Track>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    if tracks.is_empty() {
        return Err("play_album: track list is empty".into());
    }
    let first = tracks[0].clone();

    // 1) Queue + emit early so the UI updates immediately. Streaming
    //    can take several seconds on Subsonic, and the user shouldn't
    //    stare at a stale UI while we wait for the network.
    {
        let mut guard = state.lock().await;
        guard.queue.play_now(&tracks);
    }
    emit_queue_changed(&app, state.inner()).await;
    emit_track_changed(&app, state.inner(), &first).await;

    // 2) Resolve and start playback on the rodio sink for the first
    //    track. Subsequent tracks in the album stay in the queue;
    //    pressing next/previous will route through the shared
    //    helper so the sink swaps too.
    playback_helpers::play_track_and_emit(&app, state.inner(), &first).await;
    Ok(())
}

// ─── Playback (Phase 1: in-memory only) ─────────────────────────

#[tauri::command]
pub async fn get_playback_state(
    state: SharedState<'_>,
) -> Result<PlaybackStatePayload, String> {
    let guard = state.lock().await;
    // The rodio AudioPlayer is the source of truth for
    // position / is-playing / volume — but we layer our domain
    // PlaybackState on top so the queue + repeat + shuffle fields
    // come from the right places.
    let cached = guard.player.cached_state();
    let payload = PlaybackStatePayload {
        is_playing: cached.is_playing,
        position_seconds: cached.position_seconds,
        duration_seconds: cached.duration_seconds,
        volume: cached.volume,
        muted: cached.muted,
        repeat: guard.queue.repeat(),
        shuffle: guard.queue.shuffle_enabled(),
    };
    Ok(payload)
}

#[tauri::command]
pub async fn get_queue(state: SharedState<'_>) -> Result<QueueSnapshot, String> {
    let guard = state.lock().await;
    Ok(guard.queue.snapshot())
}

#[tauri::command]
pub async fn play_track(
    track: Track,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<QueueEntryId, String> {
    // 1) Register the track in the queue *and* emit `queue-changed` +
    //    `track-changed` immediately. Resolving the stream URI can
    //    take a while on slow Subsonic servers (up to the HTTP
    //    timeout), so the UI should reflect the user's click before
    //    we go off and block on the network.
    let id = {
        let mut guard = state.lock().await;
        let ids = guard.queue.play_now(std::slice::from_ref(&track));
        ids.into_iter()
            .next()
            .ok_or_else(|| "play_track: empty track list".to_string())?
    };
    emit_queue_changed(&app, state.inner()).await;
    emit_track_changed(&app, state.inner(), &track).await;

    // 2) Resolve the stream URI from the active provider. If no
    //    provider is connected we still register the track in the
    //    queue (so the UI shows it as "next") but skip the actual
    //    audio — useful for offline browsing and tests.
    playback_helpers::play_track_and_emit(&app, state.inner(), &track).await;
    Ok(id)
}

/// Like [`play_track`] but accepts a `PlayContext` so the backend
/// can auto-fill the upcoming portion of the queue with the rest of
/// the album / playlist / favourites the user just clicked into.
///
/// **Preserves history**: the new track is appended to the queue
/// (or jumped to if it's already there) rather than replacing the
/// queue. The previously-current track becomes part of the
/// history instead of being wiped.
///
/// Without a context, behaves like `play_track` minus the wipe.
#[tauri::command]
pub async fn play_track_with_context(
    track: Track,
    context: Option<sinfonic_domain::PlayContext>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<QueueEntryId, String> {
    let id = {
        let mut guard = state.lock().await;
        guard.queue.set_last_context(context.clone());
        // If the track is already in the queue (rare, but possible
        // if the user clicked a track shown in History), jump to the
        // existing entry instead of adding a duplicate.
        let id = guard
            .queue
            .find_by_track_id(&track.id)
            .map(|e| e.id.clone())
            .unwrap_or_else(|| guard.queue.add_to_queue(&track));
        let _ = guard.queue.jump_to(&id);
        id
    };
    emit_queue_changed(&app, state.inner()).await;
    emit_track_changed(&app, state.inner(), &track).await;

    // Auto-extend the queue with the next ~30 tracks from the
    // context, if any. The cap is large enough that a typical album
    // fits entirely; the "+N más" button is only meaningful for
    // long playlists / favourites collections.
    auto_extend_from_context(&app, &state, DEFAULT_AUTO_EXTEND_LIMIT).await;

    playback_helpers::play_track_and_emit(&app, state.inner(), &track).await;
    Ok(id)
}

/// Like [`play_album`] but accepts a `PlayContext` so the queue can
/// be filled with the remaining tracks of the source.
///
/// **Preserves history**: tracks are appended (not replaced) and
/// the first one becomes the new current. The previously-current
/// track, if any, slides into the history section.
#[tauri::command]
pub async fn play_album_with_context(
    tracks: Vec<Track>,
    context: Option<sinfonic_domain::PlayContext>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    if tracks.is_empty() {
        return Err("play_album_with_context: track list is empty".into());
    }
    let first = tracks[0].clone();

    let first_id = {
        let mut guard = state.lock().await;
        guard.queue.set_last_context(context);
        let mut ids = Vec::with_capacity(tracks.len());
        for track in &tracks {
            let id = guard
                .queue
                .find_by_track_id(&track.id)
                .map(|e| e.id.clone())
                .unwrap_or_else(|| guard.queue.add_to_queue(track));
            ids.push(id);
        }
        // Jump to the first track (preserving history of everything
        // before it). De-dup is handled per-track above so this
        // entry always exists.
        if let Some(first_id) = ids.first() {
            let _ = guard.queue.jump_to(first_id);
            Some(first_id.clone())
        } else {
            None
        }
    };

    if first_id.is_none() {
        return Err("play_album_with_context: failed to enqueue any track".into());
    }

    emit_queue_changed(&app, state.inner()).await;
    emit_track_changed(&app, state.inner(), &first).await;

    // Auto-extend from the context so a partial page in SongsView
    // still gets the next N library tracks appended.
    auto_extend_from_context(&app, &state, DEFAULT_AUTO_EXTEND_LIMIT).await;

    playback_helpers::play_track_and_emit(&app, state.inner(), &first).await;
    Ok(())
}

/// Append `n` more tracks from the active `PlayContext` to the
/// end of the queue. No-op if no context is set, the context has
/// been fully consumed, or the library cache doesn't know about it.
/// Returns the number of tracks actually added.
#[tauri::command]
pub async fn queue_extend_more(
    n: u32,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<u32, String> {
    if n == 0 {
        return Ok(0);
    }
    Ok(auto_extend_from_context(&app, &state, n as usize).await)
}

/// How many tracks `play_track_with_context` / `play_album_with_context`
/// auto-append on the first call. Larger queues (1000-track playlists)
/// are still loaded fully via successive `queue_extend_more` calls
/// driven by the QueuePanel's "+N más" button.
const DEFAULT_AUTO_EXTEND_LIMIT: usize = 30;

/// Resolves the active `PlayContext` (if any) and appends up to
/// `limit` tracks from that source to the end of the queue. Tracks
/// already in the queue (whether from the context itself or added
/// manually) are skipped to avoid duplicates. Emits `queue-changed`
/// only if at least one track was added. Returns the count added.
async fn auto_extend_from_context(
    app: &tauri::AppHandle,
    state: &SharedState<'_>,
    limit: usize,
) -> u32 {
    let to_add = {
        let guard = state.lock().await;
        let snapshot = guard.queue.snapshot();
        let Some(context) = snapshot.last_context.clone() else {
            return 0;
        };
        let Some(server_id) = context.server_id().cloned().or(snapshot.server_id.clone())
        else {
            return 0;
        };
        resolve_next_from_context(&guard.library, &snapshot, &context, &server_id, limit)
    };
    let Some(to_add) = to_add else { return 0 };
    if to_add.is_empty() {
        return 0;
    }
    let added = to_add.len() as u32;
    {
        let mut guard = state.lock().await;
        guard.queue.add_many(&to_add);
    }
    emit_queue_changed(app, state.inner()).await;
    added
}

/// Pure helper: given the current queue snapshot and the active
/// play context, resolve up to `limit` tracks that come AFTER the
/// current entry in the context's natural order and that aren't
/// already in the queue. Returns `None` if the context can't be
/// resolved (e.g. unknown album).
fn resolve_next_from_context(
    library: &sinfonic_library::Store,
    snapshot: &sinfonic_domain::QueueSnapshot,
    context: &sinfonic_domain::PlayContext,
    server_id: &ServerId,
    limit: usize,
) -> Option<Vec<Track>> {
    let already: std::collections::HashSet<&str> = snapshot
        .entries
        .iter()
        .map(|e| e.track_id.as_str())
        .collect();
    let current_id = snapshot
        .entries
        .get(snapshot.current_index?)
        .map(|e| e.track_id.as_str());
    let slice = |all: &[Track]| -> Vec<Track> {
        // Drop everything up to and including the current track, then
        // skip duplicates already in the queue, then take `limit`.
        let after = if let Some(curr) = current_id {
            all.iter()
                .skip_while(|t| t.id.as_str() != curr)
                .skip(1)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            all.to_vec()
        };
        after
            .into_iter()
            .filter(|t| !already.contains(t.id.as_str()))
            .take(limit)
            .collect()
    };
    match context {
        sinfonic_domain::PlayContext::Album { album_id } => {
            let tracks = library.list_album_tracks(server_id, album_id).ok()?;
            Some(slice(&tracks))
        }
        sinfonic_domain::PlayContext::Playlist { playlist_id, .. } => {
            // Resolve track metadata from the library cache so the
            // returned Vec<Track> has everything `add_many` needs.
            let track_ids = library.list_playlist_tracks(server_id, playlist_id).ok()?;
            let all: Vec<Track> = track_ids
                .iter()
                .filter_map(|tid| library.get_track(server_id, tid).ok().flatten())
                .collect();
            Some(slice(&all))
        }
        sinfonic_domain::PlayContext::Favorites { .. } => {
            let (tracks, _, _) = library.get_favorites(server_id).ok()?;
            Some(slice(&tracks))
        }
        sinfonic_domain::PlayContext::All { .. } => {
            // Paginate `list_tracks` (which is sorted by title
            // ascending) until we find the current track, then
            // return the next N tracks skipping duplicates already
            // in the queue. We bound the loop at 50 pages (≈10k
            // tracks) — the safety cap matters when the current
            // track is somewhere near the end of a huge library;
            // 50 pages is enough for any realistic catalog.
            resolve_all_next_via_pagination(library, server_id, current_id, &already, limit)
        }
    }
}

/// Pagination helper for the `All` context. Pages through
/// `list_tracks` (title-ascending) until it finds `current_id`, then
/// keeps reading until it has collected `limit` tracks that are
/// not already in the queue. Returns `None` if the library query
/// fails.
fn resolve_all_next_via_pagination(
    library: &sinfonic_library::Store,
    server_id: &ServerId,
    current_id: Option<&str>,
    already: &std::collections::HashSet<&str>,
    limit: usize,
) -> Option<Vec<Track>> {
    // Edge case: no current track (queue not started). Just return
    // the first `limit` library tracks that aren't already queued.
    if current_id.is_none() {
        let page = library.list_tracks(server_id, 0, limit.max(1)).ok()?;
        return Some(
            page
                .items
                .into_iter()
                .filter(|t| !already.contains(t.id.as_str()))
                .take(limit)
                .collect(),
        );
    }

    const PAGE_SIZE: usize = 200;
    const MAX_PAGES: usize = 50;
    let mut offset: usize = 0;
    let mut found_current = false;
    let mut out: Vec<Track> = Vec::with_capacity(limit);
    let mut pages_read: usize = 0;

    while pages_read < MAX_PAGES {
        pages_read += 1;
        let page = library.list_tracks(server_id, offset, PAGE_SIZE).ok()?;
        let page_len = page.items.len();
        if page_len == 0 {
            break;
        }
        for track in page.items {
            if !found_current {
                if track.id.as_str() == current_id.unwrap_or("") {
                    found_current = true;
                }
            } else if !already.contains(track.id.as_str()) {
                out.push(track);
                if out.len() >= limit {
                    break;
                }
            }
        }
        if out.len() >= limit {
            break;
        }
        if page_len < PAGE_SIZE {
            // Last page; no point in continuing.
            break;
        }
        offset += page_len;
    }
    Some(out)
}

#[tauri::command]
pub async fn queue_play_now(
    tracks: Vec<Track>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<Vec<QueueEntryId>, String> {
    if tracks.is_empty() {
        return Err("queue_play_now: track list is empty".into());
    }
    let ids = {
        let mut guard = state.lock().await;
        guard.queue.play_now(&tracks)
    };
    if ids.is_empty() {
        return Err("queue_play_now: nothing to play".into());
    }

    emit_queue_changed(&app, state.inner()).await;
    if let Some(first) = tracks.first() {
        emit_track_changed(&app, state.inner(), first).await;
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(ids)
}

#[tauri::command]
pub async fn queue_play_next(
    track: Track,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<QueueEntryId, String> {
    let id = {
        let mut guard = state.lock().await;
        guard.queue.play_next(&track)
    };
    emit_queue_changed(&app, state.inner()).await;
    Ok(id)
}

#[tauri::command]
pub async fn queue_add(
    track: Track,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<QueueEntryId, String> {
    let id = {
        let mut guard = state.lock().await;
        guard.queue.add_to_queue(&track)
    };
    emit_queue_changed(&app, state.inner()).await;
    Ok(id)
}

#[tauri::command]
pub async fn queue_remove(
    entry_id: String,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<bool, String> {
    let parsed: QueueEntryId = entry_id.into();
    let removed = {
        let mut guard = state.lock().await;
        guard.queue.remove_entry(&parsed)
    };
    if removed {
        emit_queue_changed(&app, state.inner()).await;
    }
    Ok(removed)
}

#[tauri::command]
pub async fn queue_jump_to(
    entry_id: String,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<bool, String> {
    let parsed: QueueEntryId = entry_id.into();
    let found = {
        let mut guard = state.lock().await;
        guard.queue.jump_to(&parsed)
    };
    if found {
        let entry = {
            let guard = state.lock().await;
            guard.queue.current().cloned()
        };
        if let Some(entry) = entry {
            // Mirror what `next` does: route through the playback
            // helper so the rodio sink actually switches to the new
            // entry. Without this, the highlight moves in the UI but
            // audio keeps playing the previous track.
            playback_helpers::play_entry_from_queue_entry(&app, state.inner(), entry).await;
        }
    }
    Ok(found)
}

#[tauri::command]
pub async fn queue_move(
    entry_id: String,
    target_index: usize,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let parsed: QueueEntryId = entry_id.into();
    {
        let mut guard = state.lock().await;
        guard
            .queue
            .move_entry(&parsed, target_index)
            .map_err(|e| e.to_string())?;
    }
    emit_queue_changed(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn queue_clear(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.queue.clear();
        guard.player.stop();
    }
    emit_queue_changed(&app, state.inner()).await;
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn set_repeat(
    repeat: RepeatMode,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.queue.set_repeat(repeat);
    }
    // `repeat` is part of the queue snapshot; downstream consumers
    // reading `useQueueStore.repeat` (e.g. the QueueView subtitle)
    // only see updates via this event, not via the playback-state
    // poll.
    emit_queue_changed(&app, state.inner()).await;
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn set_shuffle(
    enabled: bool,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.queue.set_shuffle(enabled);
    }
    emit_queue_changed(&app, state.inner()).await;
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn pause(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.pause();
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn resume(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.resume();
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn stop(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.stop();
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn next(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let next_entry = {
        let mut guard = state.lock().await;
        guard.queue.next_track().cloned()
    };
    match next_entry {
        Some(entry) => {
            playback_helpers::play_entry_from_queue_entry(&app, state.inner(), entry).await;
            Ok(())
        }
        None => {
            // Queue ended — stop the rodio sink.
            {
                let guard = state.lock().await;
                guard.player.stop();
            }
            emit_playback_state(&app, state.inner()).await;
            Ok(())
        }
    }
}

/// Threshold below which `previous` restarts the current track
/// instead of stepping back. Mirrors the "press prev in the first
/// few seconds to restart" behaviour of most desktop players.
const PREV_RESTART_THRESHOLD_SECONDS: u32 = 3;

#[tauri::command]
pub async fn previous(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    // Fast path: if the current track has only been playing for a
    // short while, "previous" should restart it rather than step
    // back. The queue engine deliberately leaves this responsibility
    // to the player (see `queue.rs::previous_track`).
    let (has_current, position_seconds) = {
        let guard = state.lock().await;
        (
            guard.queue.current().is_some(),
            guard.player.cached_state().position_seconds,
        )
    };
    if has_current && position_seconds < PREV_RESTART_THRESHOLD_SECONDS {
        {
            let guard = state.lock().await;
            guard.player.seek(0);
        }
        emit_playback_state(&app, state.inner()).await;
        return Ok(());
    }

    let prev_entry = {
        let mut guard = state.lock().await;
        guard.queue.previous_track().cloned()
    };
    match prev_entry {
        Some(entry) => {
            playback_helpers::play_entry_from_queue_entry(&app, state.inner(), entry).await;
            Ok(())
        }
        None => Ok(()),
    }
}

#[tauri::command]
pub async fn seek(
    position_seconds: u32,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.seek(position_seconds);
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn set_volume(
    volume: f32,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.set_volume(volume);
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn set_muted(
    muted: bool,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.set_muted(muted);
    }
    emit_playback_state(&app, state.inner()).await;
    Ok(())
}

// ─── Equalizer (Phase 4) ────────────────────────────────────────

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBandPayload {
    pub hz: u32,
    pub gain_db: f32,
}

/// Update a single EQ band. `gain_db` is clamped to `[-12.0, +12.0]`
/// on the Rust side; an out-of-range value is treated as the clamped
/// value, never as an error.
#[tauri::command]
pub async fn set_eq_band(
    band: EqBandPayload,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.set_eq_band(band.hz, band.gain_db);
    }
    let _ = app.emit("eq-changed", &band);
    Ok(())
}

/// Reset every EQ band to 0 dB (flat response).
#[tauri::command]
pub async fn reset_eq(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.reset_eq();
    }
    let _ = app.emit("eq-reset", ());
    Ok(())
}

/// Snapshot the current EQ bands so the UI can hydrate its sliders
/// on mount. Order matches the player's `DEFAULT_BANDS` (60 Hz …
/// 16 kHz).
#[tauri::command]
pub async fn get_eq_bands(
    state: SharedState<'_>,
) -> Result<Vec<EqBandPayload>, String> {
    let guard = state.lock().await;
    Ok(guard
        .player
        .eq_bands()
        .into_iter()
        .map(|b| EqBandPayload {
            hz: b.hz as u32,
            gain_db: b.gain_db,
        })
        .collect())
}

// ─── Crossfade (Phase 3) ──────────────────────────────────────

/// Configure the crossfade. `seconds` is clamped on the Rust side
/// to `[0, 12]` so a hostile or buggy caller can't schedule
/// hour-long fades. The configuration is persisted via
/// `library.set_preference` so the next launch restores it before
/// the first track plays.
#[tauri::command]
pub async fn set_crossfade(
    enabled: bool,
    seconds: u32,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let guard = state.lock().await;
        guard.player.set_crossfade(enabled, seconds);
        guard
            .library
            .set_preference("playback.crossfade_enabled", if_enabled_str(enabled).as_deref())
            .map_err(|e| format!("save crossfade_enabled: {e}"))?;
        guard
            .library
            .set_preference("playback.crossfade_seconds", Some(&seconds.to_string()))
            .map_err(|e| format!("save crossfade_seconds: {e}"))?;
    }
    let payload = {
        let guard = state.lock().await;
        let (e, s) = guard.player.crossfade_config();
        PlaybackConfigPayload {
            crossfade_enabled: e,
            crossfade_seconds: s,
        }
    };
    let _ = app.emit(EventName::PlaybackConfigChanged.as_str(), payload);
    Ok(())
}

/// Snapshot the current crossfade configuration so the settings UI
/// can hydrate its slider on mount. After this call returns the
/// frontend can also subscribe to `playback-config-changed` for
/// subsequent updates.
#[tauri::command]
pub async fn get_crossfade_config(
    state: SharedState<'_>,
) -> Result<PlaybackConfigPayload, String> {
    let guard = state.lock().await;
    let (enabled, seconds) = guard.player.crossfade_config();
    Ok(PlaybackConfigPayload {
        crossfade_enabled: enabled,
        crossfade_seconds: seconds,
    })
}

fn if_enabled_str(enabled: bool) -> Option<String> {
    Some(if enabled { "true".to_string() } else { "false".to_string() })
}

// ─── Queue bulk mutations (Phase 9) ─────────────────────────────

#[tauri::command]
pub async fn queue_add_many(
    tracks: Vec<Track>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<Vec<QueueEntryId>, String> {
    if tracks.is_empty() {
        return Ok(vec![]);
    }
    let ids = {
        let mut guard = state.lock().await;
        guard.queue.add_many(&tracks)
    };
    emit_queue_changed(&app, state.inner()).await;
    Ok(ids)
}

#[tauri::command]
pub async fn queue_play_next_many(
    tracks: Vec<Track>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<Vec<QueueEntryId>, String> {
    if tracks.is_empty() {
        return Ok(vec![]);
    }
    let ids = {
        let mut guard = state.lock().await;
        guard.queue.play_next_many(&tracks)
    };
    emit_queue_changed(&app, state.inner()).await;
    Ok(ids)
}

// ─── Playlist CRUD (Phase 9) ────────────────────────────────────

/// Lists all playlists from the local SQLite cache.
#[tauri::command]
pub async fn playlists_get(state: SharedState<'_>) -> Result<Vec<Playlist>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_playlists(&server_id)
        .map_err(|e| e.to_string())
}

/// Returns a playlist with its tracks resolved from the local cache.
#[tauri::command]
pub async fn playlist_detail(
    playlist_id: String,
    state: SharedState<'_>,
) -> Result<PlaylistDetail, String> {
    let server_id = active_server_id(&state).await;
    let parsed_id: PlaylistId = playlist_id.into();
    let guard = state.lock().await;
    let playlist = {
        let playlists = guard.library.list_playlists(&server_id)
            .map_err(|e| e.to_string())?;
        playlists
            .into_iter()
            .find(|p| p.id == parsed_id)
            .ok_or_else(|| "playlist not found".to_string())?
    };
    let track_ids = guard
        .library
        .list_playlist_tracks(&server_id, &parsed_id)
        .map_err(|e| e.to_string())?;
    let mut tracks = Vec::with_capacity(track_ids.len());
    for tid in track_ids {
        if let Some(track) = guard.library.get_track(&server_id, &tid).map_err(|e| e.to_string())? {
            tracks.push(track);
        }
    }
    Ok(PlaylistDetail { playlist, tracks })
}

/// Creates a new local playlist and stores it in SQLite.
/// If a provider is connected and supports playlist mutations, also
/// calls `provider.create_playlist` (errors there are non-fatal).
#[tauri::command]
pub async fn create_playlist(
    name: String,
    track_ids: Vec<TrackId>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<PlaylistId, String> {
    let server_id = active_server_id(&state).await;
    let playlist_id = {
        let guard = state.lock().await;
        guard
            .library
            .create_playlist(&server_id, &name, &track_ids)
            .map_err(|e| e.to_string())?
    };
    emit_queue_changed(&app, state.inner()).await;
    Ok(playlist_id)
}

/// Renames an existing playlist.
#[tauri::command]
pub async fn rename_playlist(
    playlist_id: String,
    name: String,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: PlaylistId = playlist_id.into();
    let guard = state.lock().await;
    guard
        .library
        .rename_playlist(&server_id, &parsed, &name)
        .map_err(|e| e.to_string())
}

/// Deletes a playlist and its track associations.
#[tauri::command]
pub async fn delete_playlist(
    playlist_id: String,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: PlaylistId = playlist_id.into();
    {
        let guard = state.lock().await;
        guard
            .library
            .delete_playlist(&server_id, &parsed)
            .map_err(|e| e.to_string())?
    }
    emit_queue_changed(&app, state.inner()).await;
    Ok(())
}

/// Appends tracks to the end of a playlist.
#[tauri::command]
pub async fn add_playlist_tracks(
    playlist_id: String,
    track_ids: Vec<TrackId>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: PlaylistId = playlist_id.into();
    {
        let guard = state.lock().await;
        guard
            .library
            .add_playlist_tracks(&server_id, &parsed, &track_ids)
            .map_err(|e| e.to_string())?
    }
    emit_queue_changed(&app, state.inner()).await;
    Ok(())
}

/// Removes entries from a playlist by their position (entry ids = position strings).
#[tauri::command]
pub async fn remove_playlist_entries(
    playlist_id: String,
    entry_ids: Vec<String>,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: PlaylistId = playlist_id.into();
    {
        let guard = state.lock().await;
        guard
            .library
            .remove_playlist_entries(&server_id, &parsed, &entry_ids)
            .map_err(|e| e.to_string())?
    }
    emit_queue_changed(&app, state.inner()).await;
    Ok(())
}

/// Moves a playlist entry to a new position.
#[tauri::command]
pub async fn move_playlist_entry(
    playlist_id: String,
    entry_id: String,
    new_index: usize,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: PlaylistId = playlist_id.into();
    {
        let guard = state.lock().await;
        guard
            .library
            .move_playlist_entry(&server_id, &parsed, &entry_id, new_index)
            .map_err(|e| e.to_string())?
    }
    emit_queue_changed(&app, state.inner()).await;
    Ok(())
}

// ─── Favorites (Phase 9) ──────────────────────────────────────

#[derive(Clone, Serialize)]
pub struct FavoritesPayload {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

/// Sets the favorite flag on a track.
#[tauri::command]
pub async fn set_track_favorite(
    track_id: String,
    favorite: bool,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: TrackId = track_id.into();
    let guard = state.lock().await;
    guard
        .library
        .set_track_favorite(&server_id, &parsed, favorite)
        .map_err(|e| e.to_string())
}

/// Sets the favorite flag on an album.
#[tauri::command]
pub async fn set_album_favorite(
    album_id: String,
    favorite: bool,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: AlbumId = album_id.into();
    let guard = state.lock().await;
    guard
        .library
        .set_album_favorite(&server_id, &parsed, favorite)
        .map_err(|e| e.to_string())
}

/// Sets the favorite flag on an artist.
#[tauri::command]
pub async fn set_artist_favorite(
    artist_id: String,
    favorite: bool,
    state: SharedState<'_>,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let parsed: ArtistId = artist_id.into();
    let guard = state.lock().await;
    guard
        .library
        .set_artist_favorite(&server_id, &parsed, favorite)
        .map_err(|e| e.to_string())
}

/// Returns all favorited tracks, albums, and artists for the active server.
#[tauri::command]
pub async fn get_favorites(state: SharedState<'_>) -> Result<FavoritesPayload, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    let (tracks, albums, artists) = guard
        .library
        .get_favorites(&server_id)
        .map_err(|e| e.to_string())?;
    Ok(FavoritesPayload { tracks, albums, artists })
}

// ─── Search (Phase 2) ───────────────────────────────────────────

#[tauri::command]
pub async fn search(
    query: String,
    limit: Option<usize>,
    state: SharedState<'_>,
) -> Result<SearchResults, String> {
    let limit = limit.unwrap_or(20);
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .search(&server_id, &query, limit)
        .map_err(|e| e.to_string())
}

// ─── Provider commands (Phase 3 + Phase 5) ──────────────────────

/// Wire-format mirror of `sinfonic_source_jellyfin::discovery::DiscoveredJellyfinServer`.
/// Kept here so the frontend never imports the crate directly.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredServer {
    pub name: String,
    pub base_url: String,
    pub server_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedServer {
    pub server_id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JellyfinLoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicLoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

/// Discover Jellyfin servers on the local network. Listens on UDP
/// 7359 for ~1.5s and falls back to a localhost probe if nothing
/// answers. Subsonic has no equivalent discovery; users enter the
/// URL manually.
#[tauri::command]
pub async fn jellyfin_discover() -> Result<Vec<DiscoveredServer>, String> {
    use sinfonic_source_jellyfin::discovery;
    let servers =
        discovery::discover_jellyfin_servers(std::time::Duration::from_millis(1500)).await;
    Ok(servers
        .into_iter()
        .map(|s| DiscoveredServer {
            name: s.name,
            base_url: s.base_url,
            server_id: s.server_id,
        })
        .collect())
}

/// Log in to a Jellyfin server, persist the token in the OS keyring,
/// upsert the server row in the SQLite cache, and install the
/// provider on the app state.
#[tauri::command]
pub async fn jellyfin_login(
    request: JellyfinLoginRequest,
    state: SharedState<'_>,
) -> Result<ConnectedServer, String> {
    let (device_id, secrets) = {
        let guard = state.lock().await;
        (guard.device_id.clone(), guard.secrets.clone())
    };

    let login_request = JellyfinAuthRequest {
        base_url: request.base_url.clone(),
        username: request.username.clone(),
        password: request.password.clone(),
        device_id: device_id.clone(),
    };

    let success = jellyfin_login_inner(login_request)
        .await
        .map_err(|e| format!("login failed: {e}"))?;

    // Persist token before swapping in the provider so a keyring
    // failure doesn't leave a half-logged-in app.
    let token = success.session.access_token.clone();
    secrets
        .save_token(success.server_id.clone(), token)
        .await
        .map_err(|e| format!("save token: {e}"))?;

    // Build the provider and refresh the server display name. The
    // server name is what we show in the UI; if the refresh fails
    // we fall back to the base URL.
    let provider = sinfonic_source_jellyfin::JellyfinProvider::new(success.session.clone())
        .map_err(|e| format!("build provider: {e}"))?;
    let server_name = provider
        .refresh_server_name()
        .await
        .unwrap_or_else(|_| success.session.base_url.clone());

    {
        let guard = state.lock().await;
        guard
            .library
            .upsert_server(
                &success.server_id,
                "jellyfin",
                &server_name,
                &request.base_url,
                Some(&request.username),
            )
            .map_err(|e| format!("upsert server: {e}"))?;
        guard
            .library
            .set_preference("last_active_server_id", Some(success.server_id.as_str()))
            .map_err(|e| format!("save preference: {e}"))?;
        let provider: Arc<dyn MusicProvider> = Arc::new(provider);
        provider_helpers::install_provider(&guard, provider).await;
    }
    queue_anchor::anchor_to_server_after_unlock(&state, &success.server_id).await;

    Ok(ConnectedServer {
        server_id: success.server_id.to_string(),
        kind: "jellyfin".into(),
        name: server_name,
        base_url: request.base_url,
    })
}

/// Log in to a Subsonic/Navidrome/Airsonic server. Performs a `ping`
/// with a freshly-minted salt + md5 token to validate the
/// credentials, persists the password in the keyring (it must be
/// re-hashed on every request, so we keep the raw password around),
/// upserts the server row, and installs the provider.
#[tauri::command]
pub async fn subsonic_login(
    request: SubsonicLoginRequest,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<ConnectedServer, String> {
    let secrets = {
        let guard = state.lock().await;
        guard.secrets.clone()
    };

    let login_request = SubsonicAuthRequest {
        base_url: request.base_url.clone(),
        username: request.username.clone(),
        password: request.password.clone(),
    };

    let success = subsonic_login_inner(login_request)
        .await
        .map_err(|e| format!("login failed: {e}"))?;

    // Subsonic re-hashes the password on every request, so we have
    // to store the raw password in the keyring. Different namespace
    // (per-server-id) avoids collisions with the Jellyfin token
    // (which has the same SecretKey variant).
    secrets
        .save_token(success.server_id.clone(), request.password.clone())
        .await
        .map_err(|e| format!("save password: {e}"))?;

    let provider = sinfonic_source_subsonic::SubsonicProvider::new(success.session.clone())
        .map_err(|e| format!("build provider: {e}"))?
        .with_app_handle(app.clone());

    {
        let guard = state.lock().await;
        guard
            .library
            .upsert_server(
                &success.server_id,
                "subsonic",
                &success.server_name,
                &request.base_url,
                Some(&request.username),
            )
            .map_err(|e| format!("upsert server: {e}"))?;
        guard
            .library
            .set_preference("last_active_server_id", Some(success.server_id.as_str()))
            .map_err(|e| format!("save preference: {e}"))?;
        // Phase 3: keep the typed SubsonicProvider alongside the
        // dyn-trait slot so `kick_subsonic_background_sync` can
        // reach the Subsonic-specific `sync_album_tracks` helper.
        provider_helpers::install_subsonic_provider(&guard, Arc::new(provider)).await;
    }
    // Fire-and-forget: kicks the Subsonic album-tracks background
    // sync so /songs becomes instant after the cache warms up.
    // Idempotent — if one is already running (e.g. the user is
    // toggling logins) the kick is a no-op.
    kick_subsonic_background_sync_inner(app, state.inner().clone()).await?;
    queue_anchor::anchor_to_server_after_unlock(&state, &success.server_id).await;

    Ok(ConnectedServer {
        server_id: success.server_id.to_string(),
        kind: "subsonic".into(),
        name: success.server_name,
        base_url: request.base_url,
    })
}

// ─── Local-files provider (Phase 8) ───────────────────────────

/// Response shape for `local_login` and `local_rescan`. Mirrors
/// `LocalProvider::ScanStats` plus the active-server snapshot.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalScanResult {
    pub server_id: String,
    pub server_name: String,
    pub root: String,
    pub tracks: usize,
    pub albums: usize,
    pub artists: usize,
    pub errors: usize,
}

/// Set the music root directory, scan it, install the `LocalProvider`
/// as the active provider, and write the result to the SQLite cache.
/// Unlike the Jellyfin / Subsonic logins this is also the "sync"
/// step — there is no separate `provider_sync_library` for local
/// because the scan already populates the in-memory snapshot that
/// `library.replace_*` reads from.
#[tauri::command]
pub async fn local_login(
    path: String,
    state: SharedState<'_>,
    app: tauri::AppHandle,
) -> Result<LocalScanResult, String> {
    use sinfonic_source_local::LocalProvider;
    let root = std::path::PathBuf::from(path.trim());
    if !root.exists() {
        return Err(format!("local: path does not exist: {root:?}"));
    }
    if !root.is_dir() {
        return Err(format!("local: not a directory: {root:?}"));
    }

    // Phase 1: validate the path and prepare the provider.
    let _ = app.emit(
        EventName::LibrarySyncStatus.as_str(),
        LibrarySyncStatusPayload {
            server_id: None,
            state: "preparing".into(),
            progress: 0.05,
        },
    );

    let provider = LocalProvider::new(&root);

    // Stop any currently-playing audio — the stream URI of the old
    // provider will stop resolving after we swap providers.
    {
        let guard = state.lock().await;
        guard.player.stop();
    }

    // Phase 2: walk the directory tree and read metadata tags. This
    // is synchronous (filesystem-bound) but on a real library it
    // can take several seconds, so we tick progress along the way.
    let _ = app.emit(
        EventName::LibrarySyncStatus.as_str(),
        LibrarySyncStatusPayload {
            server_id: None,
            state: "scanning".into(),
            progress: 0.20,
        },
    );
    let stats = provider.rescan().map_err(|e| format!("scan: {e}"))?;
    let snapshot = provider.snapshot().ok_or_else(|| "scan produced no result".to_string())?;

    // Capture the set of album ids present in this scan. The album art
    // cache prunes entries outside this set after a rescan so art for
    // albums that no longer exist on disk does not linger.
    let current_album_ids: std::collections::HashSet<String> =
        snapshot.embedded_art.keys().cloned().collect();

    // Phase 3: write the snapshot to the SQLite cache so the UI can
    // render pages straight away.
    let server_id = sinfonic_domain::ServerId::new(sinfonic_source_local::LOCAL_SERVER_ID);
    let server_name = sinfonic_source_local::LOCAL_SERVER_NAME.to_string();
    let root_display = root.display().to_string();

    let _ = app.emit(
        EventName::LibrarySyncStatus.as_str(),
        LibrarySyncStatusPayload {
            server_id: Some(server_id.to_string()),
            state: "indexing".into(),
            progress: 0.65,
        },
    );
    {
        let guard = state.lock().await;
        guard
            .library
            .upsert_server(&server_id, "local", &server_name, &root_display, None)
            .map_err(|e| format!("upsert server: {e}"))?;
        guard
            .library
            .set_preference("last_active_server_id", Some(server_id.as_str()))
            .map_err(|e| format!("save preference: {e}"))?;
        // Order matters: albums FK-references artists, and tracks
        // FK-reference albums. Insert parents before children or the
        // SQLite foreign-key check rejects the child rows even
        // though the parents are about to land in the same batch.
        guard
            .library
            .replace_artists(&server_id, &snapshot.artists)
            .map_err(|e| format!("upsert artists: {e}"))?;
        guard
            .library
            .replace_albums(&server_id, &snapshot.albums)
            .map_err(|e| format!("upsert albums: {e}"))?;
        guard
            .library
            .replace_tracks(&server_id, &snapshot.tracks)
            .map_err(|e| format!("upsert tracks: {e}"))?;
        // Jellyfin doesn't expose a per-artist track count; rebuild it
        // from the freshly-cached tracks so the Artists view shows a
        // real number instead of `0`.
        guard
            .library
            .recompute_artist_track_counts(&server_id)
            .map_err(|e| format!("recompute artist track counts: {e}"))?;

        // Persist the embedded album art to the filesystem cache so it
        // survives app restarts and can be served without re-reading
        // audio file tags. Then prune any cached entries for albums
        // that no longer exist in this scan.
        if let Some(ref cache) = guard.album_art {
            for (album_id, art) in &snapshot.embedded_art {
                if art.bytes.is_empty() {
                    continue;
                }
                let cache_key = ImageCacheKey::new(
                    sinfonic_source_local::LOCAL_PROVIDER_ID,
                    album_id.clone(),
                    "embedded",
                );
                if let Err(e) = cache.put(&cache_key, &art.bytes, &art.content_type) {
                    tracing::warn!(
                        target: "sinfonic::commands",
                        album_id = %album_id,
                        error = %e,
                        "failed to cache embedded art"
                    );
                }
            }

            if let Err(e) = cache.remove_orphans(
                sinfonic_source_local::LOCAL_PROVIDER_ID,
                &current_album_ids,
            ) {
                tracing::warn!(
                    target: "sinfonic::commands",
                    error = %e,
                    "failed to prune orphaned album art"
                );
            }
        }

        let provider: Arc<dyn MusicProvider> = Arc::new(provider);
        provider_helpers::install_provider(&guard, provider).await;
    }
    queue_anchor::anchor_to_server_after_unlock(&state, &server_id).await;

    // Phase 4: hand off to the same `provider_sync_library` flow the
    // remote providers use so the UI sees a single "complete" event
    // regardless of source.
    let _ = app.emit(
        EventName::LibrarySyncStatus.as_str(),
        LibrarySyncStatusPayload {
            server_id: Some(server_id.to_string()),
            state: "complete".into(),
            progress: 1.0,
        },
    );

    Ok(LocalScanResult {
        server_id: server_id.to_string(),
        server_name,
        root: root_display,
        tracks: stats.tracks,
        albums: stats.albums,
        artists: stats.artists,
        errors: stats.errors,
    })
}

/// Re-scan the active local provider's root. Used by the
/// "Rescan library" button once the user is connected.
#[tauri::command]
pub async fn local_rescan(
    state: SharedState<'_>,
    app: tauri::AppHandle,
) -> Result<LocalScanResult, String> {
    // Read the root off the SQLite cache (it's stored there as the
    // server's `base_url` so we can survive an app restart without
    // serialising the provider). Cheaper than a downcast and
    // exercises the same `local_login` code path on the Rust side.
    let root = {
        let guard = state.lock().await;
        // Verify the active provider is local before we touch the
        // local server row.
        let provider_kind = guard
            .provider
            .lock()
            .await
            .as_ref()
            .map(|p| p.identity().provider_id.clone());
        if provider_kind.as_deref() != Some(sinfonic_source_local::LOCAL_PROVIDER_ID) {
            return Err("local_rescan: active provider is not local".into());
        }
        let conn = guard.library.connection().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT base_url FROM servers WHERE server_id = ?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row([sinfonic_source_local::LOCAL_SERVER_ID], |r| r.get::<_, String>(0))
            .map_err(|e| format!("read root: {e}"))?
    };
    local_login(root, state, app).await
}

/// Clear the active provider (any kind) and remove its token from the keyring.
/// Library data is left in place so the user can log back in
/// without a full re-sync. Audio playback is stopped — the stream
/// URL the rodio sink is consuming will no longer resolve after
/// logout. The kind doesn't matter: we always clear whatever
/// provider is currently active.
#[tauri::command]
pub async fn provider_logout(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let (server_id_opt, secrets) = {
        let mut guard = state.lock().await;
        let server_id = guard
            .provider
            .lock()
            .await
            .as_ref()
            .map(|p| p.identity().server_id.clone());
        *guard.provider.lock().await = None;
        (server_id, guard.secrets.clone())
    };
    if let Some(server_id) = server_id_opt {
        let _ = secrets.delete_token(server_id).await;
    }
    // Drop the last-active pointer too so a stale id doesn't get
    // restored on the next launch.
    if let Ok(guard) = state.try_lock() {
        let _ = guard.library.set_preference("last_active_server_id", None);
    }
    // Notify the frontend so the QueuePanel / PlayerBar clear out
    // without waiting for the next user action.
    provider_helpers::teardown_active_provider(&app, state.inner()).await;
    Ok(())
}

/// Look up a row from the `servers` table. Returns `(kind, base_url,
/// name, username)` or an error if the id is unknown.
async fn lookup_server(
    state: &SharedState<'_>,
    server_id: &ServerId,
) -> Result<(String, String, String, Option<String>), String> {
    let guard = state.lock().await;
    let conn = guard.library.connection().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT kind, base_url, name, username FROM servers WHERE server_id = ?1")
        .map_err(|e| e.to_string())?;
    stmt.query_row([server_id.as_str()], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })
    .map_err(|e| format!("server not found: {e}"))
}

/// Provider-construction helpers shared between the live login
/// (`jellyfin_login` / `subsonic_login`), the in-app switch
/// (`provider_set_active`) and the startup restore
/// (`try_restore_provider`). Centralising them avoids the previous
/// three-way duplication where adding a new field to e.g.
/// `JellyfinSession` meant editing three call sites.
///
/// Each helper takes everything it needs by value/reference and
/// returns either an `Arc<dyn MusicProvider>` ready to install, or
/// an `Err(String)` with a UI-friendly message.
mod provider_factory {
    use super::*;

    /// Shared state handle accepted by both Tauri command context
    /// (`SharedState<'_>`) and the raw `Arc<Mutex<AppState>>` used by
    /// `try_restore_provider`. We deref to the inner `Arc` for both.
    type SharedStateLike<'a> = &'a Arc<tokio::sync::Mutex<super::AppState>>;

    /// Build a `JellyfinProvider` from a keyring-stored access token.
    pub(super) async fn build_jellyfin(
        state: SharedStateLike<'_>,
        server_id: &ServerId,
        base_url: String,
    ) -> Result<Arc<dyn MusicProvider>, String> {
        let token = {
            let guard = state.lock().await;
            guard
                .secrets
                .load_token(server_id.clone())
                .await
                .map_err(|e| format!("load token: {e}"))?
                .ok_or_else(|| "jellyfin: token missing from keyring".to_string())?
        };
        let device_id = {
            let guard = state.lock().await;
            guard.device_id.clone()
        };
        let session = sinfonic_source_jellyfin::JellyfinSession {
            server_id: server_id.clone(),
            base_url,
            access_token: token,
            // The cached session does not include `user_id`; the
            // provider falls back to base_url as the display name
            // until `refresh_server_name` runs.
            user_id: String::new(),
            device_id,
        };
        Ok(Arc::new(
            sinfonic_source_jellyfin::JellyfinProvider::new(session)
                .map_err(|e| format!("build jellyfin provider: {e}"))?,
        ))
    }

    /// Same shape as `build_jellyfin` but for Subsonic. Returns
    /// `(typed, dyn)` so callers that need to call Subsonic-specific
    /// helpers (e.g. `commands::kick_subsonic_background_sync`) can
    /// keep the typed `Arc<SubsonicProvider>` instead of losing it
    /// behind the trait object.
    pub(super) async fn build_subsonic(
        state: SharedStateLike<'_>,
        server_id: &ServerId,
        base_url: String,
        username: String,
        app: &tauri::AppHandle,
    ) -> Result<(Arc<sinfonic_source_subsonic::SubsonicProvider>, Arc<dyn MusicProvider>), String>
    {
        let password = {
            let guard = state.lock().await;
            guard
                .secrets
                .load_token(server_id.clone())
                .await
                .map_err(|e| format!("load password: {e}"))?
                .ok_or_else(|| {
                    "Subsonic password not found in system keychain. \
                     Delete this server from the Saved Servers list \
                     and add it again to sign in."
                        .to_string()
                })?
        };
        let session = sinfonic_source_subsonic::SubsonicSession {
            server_id: server_id.clone(),
            base_url,
            username,
            password,
        };
        let typed = Arc::new(
            sinfonic_source_subsonic::SubsonicProvider::new(session)
                .map_err(|e| format!("build subsonic provider: {e}"))?
                .with_app_handle(app.clone()),
        );
        let dyn_provider: Arc<dyn MusicProvider> = typed.clone();
        Ok((typed, dyn_provider))
    }

    /// Same shape for the local-files provider. No network secret;
    /// the `base_url` field actually holds the absolute music root.
    pub(super) fn build_local(base_url: &str) -> Result<Arc<dyn MusicProvider>, String> {
        let root = std::path::PathBuf::from(base_url);
        if !root.exists() {
            return Err(format!("local root no longer exists: {root:?}"));
        }
        Ok(Arc::new(sinfonic_source_local::LocalProvider::new(root)))
    }
}

/// Switch the active provider to an already-saved server without
/// re-running the login flow. Reconstructs the `MusicProvider` from
/// the keyring (Jellyfin/Subsonic) or the cached root path (local).
/// Library data stays put — switching only swaps which `server_id`
/// the library reads are scoped to.
///
/// Returns the activated server's metadata so the frontend can
/// reflect it in the store without a follow-up `provider_servers`
/// round-trip.
#[tauri::command]
pub async fn provider_set_active(
    server_id: String,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<ConnectedServer, String> {
    tracing::debug!(target: "sinfonic::commands", server_id = %server_id, "provider_set_active called");
    let parsed = ServerId::try_new(server_id.clone()).map_err(|e| e.to_string())?;
    let (kind, base_url, name, username) = lookup_server(&state, &parsed).await?;
    tracing::debug!(target: "sinfonic::commands", kind = %kind, name = %name, "server row looked up");

    let provider: Arc<dyn MusicProvider> = match kind.as_str() {
        "jellyfin" => provider_factory::build_jellyfin(&state, &parsed, base_url.clone()).await?,
        "subsonic" => {
            let sub_user = username.clone().unwrap_or_else(|| name.clone());
            // Phase 3: build_subsonic now returns (typed, dyn) so the
            // typed SubsonicProvider ends up in the dedicated slot
            // before we install the dyn wrapper.
            let (typed, dyn_provider) = provider_factory::build_subsonic(
                &state,
                &parsed,
                base_url.clone(),
                sub_user,
                &app,
            )
            .await?;
            {
                let guard = state.lock().await;
                *guard.subsonic.lock().await = Some(typed);
            }
            dyn_provider
        }
        "local" => provider_factory::build_local(&base_url)?,
        other => return Err(format!("unknown provider kind: {other}")),
    };

    // Phase 3: when switching to a Subsonic server, fire the
    // background sync so /songs becomes instant after the cache
    // warms. The provider was just installed above so the inner
    // `subsonic` slot is populated; the inner helper resolves it.
    if kind.as_str() == "subsonic" {
        kick_subsonic_background_sync_inner(app.clone(), state.inner().clone()).await?;
    }

    // Stop any audio currently being decoded under the previous
    // provider — its stream URL will stop resolving after the swap.
    // The queue is cleared by `teardown_active_provider` below (so
    // the `persist_guard` can also block the empty snapshot from
    // clobbering the previous server's persisted history).
    {
        tracing::debug!(target: "sinfonic::commands", "stopping playback and swapping provider");
        let guard = state.lock().await;
        guard.player.stop();
        *guard.provider.lock().await = Some(provider.clone());
        // Non-subsonic switches must drop the typed slot too.
        if kind.as_str() != "subsonic" {
            *guard.subsonic.lock().await = None;
        }
        tracing::debug!(target: "sinfonic::commands", "provider swapped; persisting last_active_server_id");
        // Persist the pointer so the next launch can restore us.
        guard
            .library
            .set_preference("last_active_server_id", Some(&server_id))
            .map_err(|e| format!("save preference: {e}"))?;
    }

    // Tell the frontend about the cleared queue / playback state so
    // the QueuePanel and PlayerBar update immediately rather than
    // waiting for the next user action.
    provider_helpers::teardown_active_provider(&app, state.inner()).await;

    // Anchor the queue to the newly active server. This loads the
    // target server's persisted history (if any); if none exists the
    // queue stays empty until the user plays something.
    queue_anchor::anchor_to_server_after_unlock(&state, &parsed).await;

    tracing::debug!(target: "sinfonic::commands", "provider_set_active succeeded");
    Ok(ConnectedServer {
        server_id,
        kind,
        name,
        base_url,
    })
}

/// Trigger a sync: fetch the first page of albums / artists / tracks
/// from the active provider and pipe them into the SQLite cache.
/// Subsequent reads serve from the cache. Generic over the
/// `MusicProvider` trait — works for both Jellyfin and Subsonic.
#[tauri::command]
pub async fn provider_sync_library(
    state: SharedState<'_>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!(target: "sinfonic::sync", "provider_sync_library started");
    let payload_start = LibrarySyncStatusPayload {
        server_id: None,
        state: "started".into(),
        progress: 0.0,
    };
    let _ = app.emit(EventName::LibrarySyncStatus.as_str(), payload_start);

    // Fetch outside the lock; we only hold the lock when swapping
    // results into the SQLite cache.
    let (provider_snapshot, library_handle) = {
        let guard = state.lock().await;
        let provider = guard
            .provider
            .lock()
            .await
            .as_ref()
            .cloned()
            .ok_or_else(|| "no active provider session".to_string())?;
        (provider, guard.library.clone())
    };

    let server_id = provider_snapshot.identity().server_id.clone();
    tracing::info!(target: "sinfonic::sync", server_id = %server_id, "provider_sync_library syncing");

    sync_library_data(provider_snapshot.as_ref(), &library_handle, &server_id).await?;

    let payload_done = LibrarySyncStatusPayload {
        server_id: Some(server_id.to_string()),
        state: "complete".into(),
        progress: 1.0,
    };
    let _ = app.emit(EventName::LibrarySyncStatus.as_str(), payload_done);
    tracing::info!(target: "sinfonic::sync", server_id = %server_id, "provider_sync_library complete");

    Ok(())
}

/// Fetch every artist, album, and track from the given provider and
/// write them into the SQLite cache. Extracted from
/// `provider_sync_library` so the ordering logic can be unit-tested
/// without a Tauri runtime.
///
/// # Page size + chunked fetch
///
/// Each provider paginates its collection methods (`artists`,
/// `albums`, `tracks`). The first sync of a Subsonic/Navidrome
/// library returns a small first page (≤200), and `local_login`'s
/// `albums()` is in fact a snapshot of the entire library. To stay
/// correct for libraries of any size we loop through pages until the
/// provider reports fewer items than the page size (or `total` is
/// reached). Earlier this function used a single hard-coded
/// `PagedRequest::new(0, 200)` call per entity — which silently
/// dropped anything past page 1 and produced a `FOREIGN KEY
/// constraint failed` on `replace_albums` because albums past #200
/// referenced artists past #200 (sorted by name) that the previous
/// page had not inserted.
///
/// # Order matters
///
/// `albums.artist_id` is a foreign key into `artists(artist_id)`, and
/// `tracks.album_id` is a foreign key into `albums`. Even though
/// every `replace_*` call enables `PRAGMA defer_foreign_keys = ON`,
/// the deferred check still runs at transaction commit time — so
/// writing albums before the referenced artists exist causes
/// `sqlite error: FOREIGN KEY constraint failed`. The contract is:
/// **artists → albums → tracks**. The same order is used by
/// `local_login` (Phase 3 of that command).
pub async fn sync_library_data(
    provider: &dyn sinfonic_source::MusicProvider,
    library: &sinfonic_library::Store,
    server_id: &sinfonic_domain::ServerId,
) -> Result<(), String> {
    use sinfonic_domain::PagedRequest;

    const PAGE_SIZE: usize = 200;

    tracing::info!(target: "sinfonic::sync", "fetching artists");
    let artists = fetch_all_pages(PAGE_SIZE, |offset| async move {
        provider.artists(PagedRequest::new(offset, PAGE_SIZE)).await
    })
    .await
    .map_err(|e| format!("artists: {e}"))?;
    tracing::info!(target: "sinfonic::sync", count = artists.len(), "artists fetched; writing to DB");
    library
        .replace_artists(server_id, &artists)
        .map_err(|e| format!("upsert artists: {e}"))?;
    tracing::debug!(target: "sinfonic::sync", "artists written");

    tracing::info!(target: "sinfonic::sync", "fetching albums");
    let albums = fetch_all_pages(PAGE_SIZE, |offset| async move {
        provider.albums(PagedRequest::new(offset, PAGE_SIZE)).await
    })
    .await
    .map_err(|e| format!("albums: {e}"))?;
    tracing::info!(target: "sinfonic::sync", count = albums.len(), "albums fetched; writing to DB");
    library
        .replace_albums(server_id, &albums)
        .map_err(|e| format!("upsert albums: {e}"))?;
    tracing::debug!(target: "sinfonic::sync", "albums written");

    tracing::info!(target: "sinfonic::sync", "fetching tracks");
    let tracks = fetch_all_pages(PAGE_SIZE, |offset| async move {
        provider.tracks(PagedRequest::new(offset, PAGE_SIZE)).await
    })
    .await
    .map_err(|e| format!("tracks: {e}"))?;
    tracing::info!(target: "sinfonic::sync", count = tracks.len(), "tracks fetched; writing to DB");
    library
        .replace_tracks(server_id, &tracks)
        .map_err(|e| format!("upsert tracks: {e}"))?;
    // Jellyfin's `MusicArtist` DTO doesn't include a per-artist track
    // count, so its mapper hardcodes 0. Rebuild from the cached
    // tracks table so the Artists view shows the real number.
    library
        .recompute_artist_track_counts(server_id)
        .map_err(|e| format!("recompute artist track counts: {e}"))?;
    tracing::debug!(target: "sinfonic::sync", "tracks written; artist counts recomputed");

    tracing::info!(target: "sinfonic::sync", "fetching playlists");
    let playlists = fetch_all_pages(PAGE_SIZE, |offset| async move {
        provider
            .playlists(PagedRequest::new(offset, PAGE_SIZE))
            .await
    })
    .await
    .map_err(|e| format!("playlists: {e}"))?;
    tracing::info!(target: "sinfonic::sync", count = playlists.len(), "playlists fetched; resolving tracks");
    let mut synced = 0usize;
    let mut skipped = 0usize;
    for playlist in &playlists {
        // Per-playlist errors are non-fatal: a single corrupt
        // playlist shouldn't block the rest of the sync.
        match provider.playlist_detail(&playlist.id).await {
            Ok(detail) => {
                let track_ids: Vec<TrackId> =
                    detail.tracks.iter().map(|t| t.id.clone()).collect();
                if let Err(e) =
                    library.replace_playlist(server_id, &detail.playlist, &track_ids)
                {
                    tracing::warn!(
                        target: "sinfonic::sync",
                        playlist_id = %playlist.id.as_str(),
                        error = %e,
                        "skipping playlist"
                    );
                    skipped += 1;
                } else {
                    synced += 1;
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "sinfonic::sync",
                    playlist_id = %playlist.id.as_str(),
                    error = %e,
                    "skipping playlist"
                );
                skipped += 1;
            }
        }
    }
    tracing::info!(
        target: "sinfonic::sync",
        synced,
        skipped,
        "playlists synced"
    );

    Ok(())
}

/// Drive a paginated `MusicProvider` collection method to completion
/// by repeatedly issuing `PagedRequest`s until the provider reports
/// fewer items than `page_size`. The `total` field on the response is
/// NOT used as a loop guard: Subsonic reports `total = items.len()`
/// per page (the server caps the response), so trusting it would loop
/// forever on a Subsonic library with >`page_size` items.
///
/// Kept `pub` so the chunked-fetch semantics can be exercised in
/// isolation by the integration tests in `tests/sync_library_order.rs`.
pub async fn fetch_all_pages<T, F, Fut>(
    page_size: usize,
    mut fetch: F,
) -> sinfonic_source::ProviderResult<Vec<T>>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<
        Output = sinfonic_source::ProviderResult<sinfonic_domain::PagedResponse<T>>,
    >,
{
    let mut all: Vec<T> = Vec::new();
    let mut offset: usize = 0;
    loop {
        let page = fetch(offset).await?;
        let received = page.items.len();
        all.extend(page.items);
        if received < page_size {
            break;
        }
        offset += received;
    }
    Ok(all)
}

/// Return the list of servers the user has configured (rows in the
/// `servers` table). The active server — if any — is the first entry.
#[tauri::command]
pub async fn provider_servers(
    state: SharedState<'_>,
) -> Result<Vec<ConnectedServer>, String> {
    let guard = state.lock().await;
    let conn = guard.library.connection().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT server_id, kind, name, base_url FROM servers ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConnectedServer {
                server_id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                base_url: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let items: Vec<ConnectedServer> = rows.filter_map(|r| r.ok()).collect();
    Ok(items)
}

/// Delete a saved server from the `servers` table and clear its
/// credentials from the keyring. If the deleted server was the
/// active one, the in-memory provider is also cleared.
#[tauri::command]
pub async fn provider_delete(
    server_id: String,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    tracing::debug!(target: "sinfonic::commands", server_id = %server_id, "provider_delete called");
    let (was_active, secrets) = {
        let guard = state.lock().await;
        let was_active = guard
            .provider
            .lock()
            .await
            .as_ref()
            .is_some_and(|p| p.identity().server_id.to_string() == server_id);
        (was_active, guard.secrets.clone())
    };

    // If it was active, clear the in-memory provider and preferences.
    if was_active {
        tracing::debug!(target: "sinfonic::commands", "deleted server was active; clearing provider");
        let mut guard = state.lock().await;
        *guard.provider.lock().await = None;
        let _ = guard.library.set_preference("last_active_server_id", None);
    }

    // Delete from keyring.
    let _ = secrets.delete_token(ServerId::new(server_id.clone())).await;

    // Delete from database.
    {
        let guard = state.lock().await;
        let conn = guard.library.connection().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM servers WHERE server_id = ?1",
            sqlite::params![server_id],
        )
        .map_err(|e| format!("delete server: {e}"))?;
    }

    // Mirror the in-memory clearing to the frontend so the QueuePanel
    // and PlayerBar update immediately. Same teardown pipeline as
    // provider_logout / provider_set_active — track ids from the
    // previous provider would never resolve against the next one.
    if was_active {
        provider_helpers::teardown_active_provider(&app, state.inner()).await;
    }

    tracing::debug!(target: "sinfonic::commands", "provider_delete done");
    Ok(())
}

/// Surface the active server id (or `null` if none). Used by the
/// frontend to keep the Zustand store in sync.
#[tauri::command]
pub async fn provider_active_server(
    state: SharedState<'_>,
) -> Result<Option<String>, String> {
    let guard = state.lock().await;
    let provider = guard.provider.lock().await;
    Ok(provider
        .as_ref()
        .map(|p| p.identity().server_id.to_string()))
}

/// Snapshot the bootstrap state for the frontend route guard.
///
/// The `try_restore_provider` background task is spawned during
/// app startup; the frontend cannot rely on a single
/// `provider_active_server` poll to see the restored session
/// because the call may land before the task finishes. This
/// command bundles three things in one roundtrip:
///
///   * `ready` — true once the restore task has exited, so the
///     route guard can stop polling and decide where to land.
///   * `active_server_id` — the provider the restore picked (or
///     `None` if no session was persisted / the row was stale).
///   * `saved_servers` — every configured source. The setup view
///     uses this to render a "Quick connect" list so the user
///     can re-attach an existing source with one click instead
///     of running the full wizard again.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub ready: bool,
    pub active_server_id: Option<String>,
    pub saved_servers: Vec<ConnectedServer>,
}

#[tauri::command]
pub async fn bootstrap_state(state: SharedState<'_>) -> Result<BootstrapState, String> {
    use std::sync::atomic::Ordering;

    let guard = state.lock().await;
    let ready = guard.bootstrap_complete.load(Ordering::Relaxed);
    let active_server_id = guard
        .provider
        .lock()
        .await
        .as_ref()
        .map(|p| p.identity().server_id.to_string());

    let conn = guard.library.connection().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT server_id, kind, name, base_url FROM servers ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConnectedServer {
                server_id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                base_url: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let saved_servers: Vec<ConnectedServer> = rows.filter_map(|r| r.ok()).collect();

    Ok(BootstrapState {
        ready,
        active_server_id,
        saved_servers,
    })
}

// ─── Album art (Phase 7) ───────────────────────────────────────

/// Payload returned by `provider_image_bytes`. Mirrors the on-disk
/// cache shape so the frontend can build a blob URL straight away.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumArtResponse {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub cached: bool,
}

/// Resolve an album's primary cover image through the active
/// provider, with a read-through filesystem cache keyed by
/// `(provider, image_id, tag)`.
///
/// `album_id` is the provider's item id (e.g. `jellyfin`'s
/// `Album/abc…` or `subsonic`'s `album-12`). The optional `tag`
/// comes from the library cache's `image_tag` column — passing it
/// ensures a stale cache entry is invalidated when the server bumps
/// the tag.
#[tauri::command]
pub async fn provider_image_bytes(
    album_id: String,
    tag: Option<String>,
    state: SharedState<'_>,
) -> Result<AlbumArtResponse, String> {
    if album_id.is_empty() {
        return Err("provider_image_bytes: album_id is empty".into());
    }

    let provider_id;
    let request_kind = ImageKind::Primary;
    let request = ImageRequest {
        item_id: album_id.clone(),
        kind: request_kind,
        tag: tag.clone(),
        size: 600,
    };

    // Cache lookup is best-effort. The cache may be absent (e.g.
    // when the app data dir is unavailable) — in that case we just
    // fetch through the provider and skip the write.
    let cache_key = ImageCacheKey::new(
        {
            let guard = state.lock().await;
            let provider = guard.provider.lock().await;
            provider_id = provider
                .as_ref()
                .map(|p| p.identity().provider_id.clone())
                .unwrap_or_else(|| "unknown".into());
            provider_id.clone()
        },
        album_id.clone(),
        tag.clone().unwrap_or_default(),
    );

    let (cache_for_lookup, cache_for_write) = {
        let guard = state.lock().await;
        let cache = guard.album_art.clone();
        // Hand the same Arc clone to both the lookup and the eventual
        // write — no need to clone twice.
        (cache.clone(), cache)
    };

    if let Some(cache) = cache_for_lookup.as_ref() {
        if let Ok(Some(hit)) = cache.get(&cache_key) {
            return Ok(AlbumArtResponse {
                bytes: hit.bytes,
                content_type: hit.content_type,
                cached: true,
            });
        }
    }

    // Cache miss — fetch from the provider.
    let fetched: ImageBytes = {
        let guard = state.lock().await;
        let provider_guard = guard.provider.lock().await;
        let provider = provider_guard
            .as_ref()
            .ok_or_else(|| "provider_image_bytes: no active provider".to_string())?;
        provider
            .image_bytes(request)
            .await
            .map_err(|e| format!("image_bytes: {e}"))?
    };

    // Pick a sensible default if the provider did not surface a
    // content type — JPEG is the dominant format in the wild.
    let content_type = fetched
        .content_type
        .unwrap_or_else(|| guess_image_content_type(&fetched.bytes).to_string());

    // Best-effort write-through. Failures here are non-fatal — we
    // already have the bytes to return to the UI.
    if let Some(cache) = cache_for_write.as_ref() {
        let _ = cache.put(&cache_key, &fetched.bytes, &content_type);
    }

    Ok(AlbumArtResponse {
        bytes: fetched.bytes,
        content_type,
        cached: false,
    })
}

/// Bulk version of `provider_image_bytes` for the JS-side album art
/// prewarm. Each request is resolved against the on-disk cache first
/// and only falls through to the provider on a miss. Misses are
/// fetched in parallel so a single slow provider call does not
/// serialise the whole batch. Items that fail (no provider, network
/// error) are simply omitted from the response so the frontend can
/// still render the rest of the grid.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumArtRequest {
    pub album_id: String,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumArtBulkItem {
    pub album_id: String,
    pub tag: Option<String>,
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub cached: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumArtBulkResponse {
    pub images: Vec<AlbumArtBulkItem>,
    pub not_found: Vec<String>,
}

#[tauri::command]
pub async fn provider_image_bytes_bulk(
    requests: Vec<AlbumArtRequest>,
    state: SharedState<'_>,
) -> Result<AlbumArtBulkResponse, String> {
    use futures::future::join_all;

    let (provider_id, cache) = {
        let guard = state.lock().await;
        let provider_id = guard
            .provider
            .lock()
            .await
            .as_ref()
            .map(|p| p.identity().provider_id.clone())
            .unwrap_or_else(|| "unknown".into());
        (provider_id, guard.album_art.clone())
    };

    // Split into cache hits and misses first. Cache hits do not need
    // a provider roundtrip and can be returned immediately.
    let mut images: Vec<AlbumArtBulkItem> = Vec::with_capacity(requests.len());
    let mut misses: Vec<AlbumArtRequest> = Vec::new();
    let mut not_found: Vec<String> = Vec::new();

    for req in requests {
        if req.album_id.is_empty() {
            continue;
        }
        let key = ImageCacheKey::new(
            provider_id.clone(),
            req.album_id.clone(),
            req.tag.clone().unwrap_or_default(),
        );
        let hit = cache.as_ref().and_then(|c| c.get(&key).ok().flatten());
        match hit {
            Some(cached) => images.push(AlbumArtBulkItem {
                album_id: req.album_id.clone(),
                tag: req.tag.clone(),
                bytes: cached.bytes,
                content_type: cached.content_type,
                cached: true,
            }),
            None => misses.push(req),
        }
    }

    // Fetch misses in parallel from the provider. Each fetch goes
    // through the same `provider.image_bytes` path as the single
    // command, so write-through to the cache still happens.
    let fetch_futures = misses.iter().map(|req| {
        let req = req.clone();
        let state = state.inner().clone();
        async move {
            let request = ImageRequest {
                item_id: req.album_id.clone(),
                kind: ImageKind::Primary,
                tag: req.tag.clone(),
                size: 600,
            };
            let guard = state.lock().await;
            let provider_guard = guard.provider.lock().await;
            let provider = provider_guard.as_ref()?;
            let res = provider.image_bytes(request).await.ok()?;
            let content_type = res
                .content_type
                .unwrap_or_else(|| guess_image_content_type(&res.bytes).to_string());
            if let Some(ref cache) = guard.album_art {
                let key = ImageCacheKey::new(
                    provider.identity().provider_id.clone(),
                    req.album_id.clone(),
                    req.tag.clone().unwrap_or_default(),
                );
                let _ = cache.put(&key, &res.bytes, &content_type);
            }
            Some(AlbumArtBulkItem {
                album_id: req.album_id.clone(),
                tag: req.tag.clone(),
                bytes: res.bytes,
                content_type,
                cached: false,
            })
        }
    });
    let fetched = join_all(fetch_futures).await;
    for (req, maybe_image) in misses.into_iter().zip(fetched) {
        match maybe_image {
            Some(image) => images.push(image),
            None => not_found.push(req.album_id),
        }
    }

    Ok(AlbumArtBulkResponse { images, not_found })
}

/// Sniff a small set of magic bytes to fall back to a content type
/// when the provider's `Content-Type` header was missing. JPEG /
/// PNG / WebP / GIF cover the overwhelming majority of music
/// artwork; anything else becomes `application/octet-stream` so the
/// browser still tries to render.
fn guess_image_content_type(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        "image/jpeg"
    } else if bytes.len() >= 8
        && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    {
        "image/png"
    } else if bytes.len() >= 6 && &bytes[..6] == b"GIF87a" || bytes.len() >= 6 && &bytes[..6] == b"GIF89a" {
        "image/gif"
    } else if bytes.len() >= 12
        && &bytes[..4] == b"RIFF"
        && &bytes[8..12] == b"WEBP"
    {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

// ─── Lyrics ──────────────────────────────────────────────────────

/// Fetch lyrics for a track, with provider-first / LRCLIB-fallback
/// orchestration.
///
/// Lookup order:
/// 1. The active music provider (Subsonic / Navidrome /
///    Jellyfin / Local). Providers return `None` when they have no
///    lyrics for the track; they return `Err(Unsupported)` when the
///    provider doesn't ship lyrics at all (today that's Jellyfin —
///    `/Audio/{Id}/Lyrics` is a separate phase).
/// 2. **LRCLIB**, only when `allow_remote` is true. We look up the
///    track in the local SQLite library cache, build a
///    `(artist, title, album, duration)` query and ask LRCLIB.
///
/// Both layers swallow non-critical errors and return `Ok(None)`
/// so the lyrics panel can render the "no lyrics" empty state
/// instead of surfacing toasts every time the user skips a track.
#[tauri::command]
pub async fn get_lyrics(
    track_id: String,
    allow_remote: Option<bool>,
    state: SharedState<'_>,
) -> Result<Option<Lyrics>, String> {
    let parsed = TrackId::from(track_id.as_str());
    let allow_remote = allow_remote.unwrap_or(true);
    let (provider, lyrics_client, library, server_id) = {
        let guard = state.lock().await;
        let server_id = active_server_id(&state).await;
        let provider = guard.provider.lock().await.clone();
        // Drop the outer mutex guard and the inner provider guard
        // explicitly so the locks are released before the LRCLIB
        // round-trip — we don't want to wedge every other Tauri
        // command on a 5 s network call.
        (
            provider,
            guard.lyrics_client.clone(),
            guard.library.clone(),
            server_id,
        )
    };
    lookup_lyrics(
        provider,
        lyrics_client,
        &library,
        server_id,
        &parsed,
        allow_remote,
    )
    .await
}

/// Pure orchestration step that the `get_lyrics` Tauri command
/// delegates to. Lives as a free function so integration tests can
/// exercise it without spinning up a Tauri runtime.
pub async fn lookup_lyrics(
    provider: Option<Arc<dyn MusicProvider>>,
    lyrics_client: Arc<sinfonic_lyrics::LrclibClient>,
    library: &sinfonic_library::Store,
    server_id: ServerId,
    track_id: &TrackId,
    allow_remote: bool,
) -> Result<Option<Lyrics>, String> {
    // Layer 1 — active provider.
    if let Some(provider) = provider {
        match provider.lyrics(track_id, allow_remote).await {
            Ok(Some(lyrics)) => return Ok(Some(lyrics)),
            Ok(None) => {}
            // Providers that don't ship lyrics treat the question
            // as out-of-scope, not an error — fall through to
            // LRCLIB instead of bubbling a toast.
            Err(sinfonic_source::ProviderError::Unsupported(_)) => {}
            Err(e) => return Err(format!("lyrics: {e}")),
        }
    }

    if !allow_remote {
        return Ok(None);
    }

    // Layer 2 — LRCLIB. Need (artist, title, album, duration), so
    // look the track up in the local SQLite cache first.
    let track = match library.get_track(&server_id, track_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            tracing::trace!(track_id = %track_id, "lrclib: track not in library");
            return Ok(None);
        }
        Err(e) => {
            tracing::warn!(error = %e, "lrclib: library lookup failed");
            return Ok(None);
        }
    };
    let query = sinfonic_lyrics::LyricsQuery {
        artist: &track.artist,
        title: &track.title,
        album: Some(&track.album),
        duration_seconds: Some(track.duration_seconds),
    };
    match lyrics_client.fetch(&query).await {
        Ok(Some(hit)) if hit.instrumental => Ok(Some(Lyrics {
            plain: None,
            synced: None,
            source: Some("lrclib-instrumental".to_string()),
        })),
        Ok(Some(hit)) => Ok(Some(Lyrics {
            plain: hit.plain,
            synced: hit.synced,
            source: Some("lrclib".to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => {
            // Network down or LRCLIB returned a malformed body —
            // either way the user deserves the "no lyrics" empty
            // state, not an error toast.
            tracing::warn!(error = %e, track_id = %track_id, "lrclib fetch failed");
            Ok(None)
        }
    }
}

// ─── Last.fm (Phase 7) ─────────────────────────────────────────

/// Hash the password to md5 hex and exchange credentials for a
/// session key via `auth.getMobileSession`. Persists the api key +
/// secret pair AND the session key in the OS keyring so the next
/// launch can `resume` without re-prompting.
#[tauri::command]
pub async fn lastfm_connect(
    api_key: String,
    api_secret: String,
    username: String,
    password: String,
    state: SharedState<'_>,
) -> Result<lastfm::LastFmStatus, String> {
    let creds = lastfm::StoredCredentials {
        api_key: api_key.clone(),
        api_secret: api_secret.clone(),
    };
    let guard = state.lock().await;
    let session = lastfm::authenticate_and_store(
        guard.secrets.as_ref(),
        &creds,
        &username,
        &password,
        guard.lastfm.as_ref(),
    )
    .await?;
    let _ = session;
    Ok(lastfm::LastFmStatus {
        configured: true,
        authenticated: true,
        username: Some(username),
    })
}

/// Drop the in-memory Last.fm client and remove both entries from
/// the keyring. The next `lastfm_status` call will report
/// `configured=false`.
#[tauri::command]
pub async fn lastfm_disconnect(state: SharedState<'_>) -> Result<lastfm::LastFmStatus, String> {
    {
        let guard = state.lock().await;
        let mut slot = guard.lastfm.lock().await;
        slot.take();
    }
    {
        let guard = state.lock().await;
        lastfm::clear_secrets(guard.secrets.as_ref()).await?;
    }
    Ok(lastfm::LastFmStatus {
        configured: false,
        authenticated: false,
        username: None,
    })
}

/// Cheap status read used by the Settings UI on mount. Does not
/// trigger any network traffic.
#[tauri::command]
pub async fn lastfm_status(
    state: SharedState<'_>,
) -> Result<lastfm::LastFmStatus, String> {
    let guard = state.lock().await;
    let configured = lastfm::load_credentials(guard.secrets.as_ref())
        .await
        .map(|c| c.is_some())
        .unwrap_or(false);
    let authenticated = guard.lastfm.lock().await.is_some();
    Ok(lastfm::LastFmStatus {
        configured,
        authenticated,
        username: None,
    })
}

/// Re-attach a previously-persisted session key, if any. Called by
/// `lib.rs` once at startup so scrobbling resumes without a
/// re-prompt.
pub async fn try_resume_lastfm(state: &AppState) {
    let secrets = state.secrets.clone();
    let slot = state.lastfm.clone();
    let _ = lastfm::try_resume(secrets.as_ref(), slot.as_ref()).await;
}

/// Anchor + restore helper. Centralises the "set queue.server_id
    /// and apply the persisted snapshot" dance so every login path
    /// (startup restore, manual login, switch server) behaves the
    /// same way.
    mod queue_anchor {
        use super::*;

        /// Anchors the in-memory queue to `server_id` and, if a
        /// persisted snapshot exists for that server, rebuilds the
        /// queue from it (history only — the upcoming portion is
        /// truncated per product decision). The audio player is NOT
        /// auto-started; the user has to press Play. Takes an
        /// already-locked guard so it composes with
        /// `try_restore_provider` (which is already holding the
        /// lock at the call site).
        pub(super) async fn anchor_to_server(
            state_ref: &mut tokio::sync::MutexGuard<'_, AppState>,
            server_id: &ServerId,
        ) {
            state_ref.queue.set_server_id(server_id.clone());
            let snapshot = state_ref
                .library
                .load_queue_snapshot(server_id)
                .ok()
                .flatten();
            match snapshot {
                Some(snap) => {
                    let mut restored = sinfonic_domain::queue::QueueEngine::from_snapshot(snap);
                    let keep_until = restored.current_index().map(|i| i + 1).unwrap_or(0);
                    restored.truncate_after(keep_until);
                    restored.set_server_id(server_id.clone());
                    tracing::info!(
                        target: "sinfonic::commands",
                        server_id = %server_id,
                        history_entries = restored.len(),
                        current_index = ?restored.current_index(),
                        "restored queue history from disk"
                    );
                    state_ref.queue = restored;
                }
                None => {
                    tracing::debug!(
                        target: "sinfonic::commands",
                        server_id = %server_id,
                        "no persisted queue snapshot; starting fresh"
                    );
                }
            }
        }

        /// Same as [`anchor_to_server`] but takes the `Arc<Mutex<…>>`
        /// directly and acquires the lock internally. Used by the
        /// login commands which release the lock between the
        /// provider install and the queue anchor step (so the
        /// `anchor_to_server` borrow doesn't fight a held lock).
        pub(super) async fn anchor_to_server_after_unlock(
            state: &Arc<Mutex<AppState>>,
            server_id: &ServerId,
        ) {
            let mut guard = state.lock().await;
            anchor_to_server(&mut guard, server_id).await;
        }
    }

    /// Restore the provider that was active when the app last shut down.
    /// Reads the `last_active_server_id` preference and rebuilds the
    /// matching provider from the keyring / SQLite root path.
    ///
    /// Failures are logged and dropped: a stale pointer (server deleted,
    /// keyring entry gone, root directory moved) should land the user on
    /// the setup view, not on a crash screen.
    pub async fn try_restore_provider(state: &Arc<Mutex<AppState>>, app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;

    tracing::debug!(target: "sinfonic::commands", "try_restore_provider starting");
    // Run the actual restore in an inner block so we can mark the
    // bootstrap as complete on every exit path — success, missing
    // pointer, stale row, or provider build failure. The frontend
    // polls `bootstrap_state` until this flips.
    let result: Result<(), String> = async {
        let last_active = {
            let state_ref = state.lock().await;
            match state_ref.library.get_preference("last_active_server_id") {
                Ok(Some(id)) => {
                    tracing::debug!(target: "sinfonic::commands", last_active_server_id = %id, "found pointer");
                    id
                }
                Ok(None) => {
                    tracing::debug!(target: "sinfonic::commands", "no last_active_server_id preference");
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(target: "sinfonic::commands", error = %e, "could not read last_active_server_id");
                    return Err(format!("read last_active_server_id: {e}"));
                }
            }
        };

        let parsed = ServerId::new(last_active.clone());
        let (kind, base_url, name, username) = {
            let state_ref = state.lock().await;
            let conn = state_ref
                .library
                .connection()
                .map_err(|e| format!("open connection: {e}"))?;
            let mut stmt = conn
                .prepare("SELECT kind, base_url, name, username FROM servers WHERE server_id = ?1")
                .map_err(|e| format!("prepare: {e}"))?;
            match stmt.query_row([last_active.as_str()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            }) {
                Ok(row) => {
                    tracing::debug!(
                        target: "sinfonic::commands",
                        kind = %row.0,
                        name = %row.2,
                        "server row found"
                    );
                    row
                }
                // Pointer targets a row that no longer exists — clear it
                // so we don't retry every launch.
                Err(_) => {
                    tracing::warn!(target: "sinfonic::commands", server_id = %last_active, "server row not found; clearing pointer");
                    let _ = state_ref
                        .library
                        .set_preference("last_active_server_id", None);
                    return Ok(());
                }
            }
        };

        let provider: Option<Arc<dyn MusicProvider>> = match kind.as_str() {
            "jellyfin" => {
                tracing::debug!(target: "sinfonic::commands", "restoring Jellyfin provider");
                match provider_factory::build_jellyfin(state, &parsed, base_url).await {
                    Ok(p) => Some(p),
                    Err(e) => {
                        tracing::warn!(target: "sinfonic::commands", error = %e, "jellyfin restore failed");
                        return Err(e);
                    }
                }
            }
            "subsonic" => {
                tracing::debug!(target: "sinfonic::commands", "restoring Subsonic provider");
                let sub_user = username.unwrap_or(name);
                match provider_factory::build_subsonic(state, &parsed, base_url, sub_user, app).await {
                    Ok((typed, dyn_provider)) => {
                        // Phase 3: populate the typed slot on restore too.
                        let state_ref = state.lock().await;
                        *state_ref.subsonic.lock().await = Some(typed);
                        drop(state_ref);
                        Some(dyn_provider)
                    }
                    Err(e) => {
                        tracing::warn!(target: "sinfonic::commands", error = %e, "subsonic restore failed");
                        return Err(e);
                    }
                }
            }
            "local" => {
                tracing::debug!(target: "sinfonic::commands", "restoring Local provider");
                let root = std::path::PathBuf::from(&base_url);
                if !root.exists() {
                    tracing::warn!(
                        target: "sinfonic::commands",
                        root = %root.display(),
                        "local root no longer exists; clearing pointer"
                    );
                    let state_ref = state.lock().await;
                    let _ = state_ref
                        .library
                        .set_preference("last_active_server_id", None);
                    return Ok(());
                }
                Some(Arc::new(sinfonic_source_local::LocalProvider::new(root)))
            }
            other => {
                return Err(format!("unknown provider kind: {other}"));
            }
        };

        let Some(provider) = provider else {
            tracing::warn!(target: "sinfonic::commands", "provider build returned None");
            return Ok(());
        };
        tracing::debug!(target: "sinfonic::commands", "provider built; installing as active");
        let mut state_ref = state.lock().await;
        *state_ref.provider.lock().await = Some(provider);
        // Re-anchor the queue to the restored server and load the
        // persisted snapshot (history only — the upcoming portion is
        // truncated on restore per product decision).
        queue_anchor::anchor_to_server(&mut state_ref, &parsed).await;
        // Phase 3: on a Subsonic restore, fire the album-tracks
        // background sync so /songs becomes instant after the
        // cache warms. Drop the AppState lock first so the spawned
        // task can reacquire it freely.
        let restored_kind = kind.clone();
        drop(state_ref);
        if restored_kind == "subsonic" {
            kick_subsonic_background_sync_inner(app.clone(), state.clone()).await?;
        }
        tracing::info!(target: "sinfonic::commands", "try_restore_provider succeeded");
        Ok(())
    }
    .await;

    if let Err(e) = result {
        tracing::warn!(target: "sinfonic::commands", error = %e, "try_restore_provider failed");
    }

    state
        .lock()
        .await
        .bootstrap_complete
        .store(true, Ordering::Relaxed);
    tracing::debug!(target: "sinfonic::commands", "bootstrap_complete set to true");
}

// ─── Smart Playlists (Phase 9) ─────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSmartPlaylistArgs {
    name: String,
    field: SmartPlaylistRuleField,
    operator: SmartPlaylistRuleOperator,
    value: String,
    sort_field: SmartPlaylistSortField,
    sort_dir: SmartPlaylistSortDirection,
    limit_n: u16,
}

#[tauri::command]
pub async fn get_smart_playlists(
    state: SharedState<'_>,
) -> Result<Vec<SmartPlaylist>, String> {
    let server_id = active_server_id(&state).await;
    let guard = state.lock().await;
    guard
        .library
        .list_smart_playlists(&server_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_smart_playlist(
    state: SharedState<'_>,
    args: CreateSmartPlaylistArgs,
) -> Result<SmartPlaylist, String> {
    let server_id = active_server_id(&state).await;
    let sp_id = SmartPlaylistId::new(format!("sp-{}", uuid::Uuid::new_v4()));
    let sp = SmartPlaylist {
        id: sp_id.clone(),
        name: args.name,
        rule: sinfonic_domain::SmartPlaylistRule {
            field: args.field,
            operator: args.operator,
            value: args.value,
        },
        sort_field: args.sort_field,
        sort_dir: args.sort_dir,
        limit_n: args.limit_n,
    };
    let guard = state.lock().await;
    guard
        .library
        .replace_smart_playlists(&server_id, std::slice::from_ref(&sp))
        .map_err(|e| e.to_string())?;
    Ok(sp)
}

#[tauri::command]
pub async fn delete_smart_playlist(
    state: SharedState<'_>,
    sp_id: String,
) -> Result<(), String> {
    let server_id = active_server_id(&state).await;
    let id = SmartPlaylistId::try_new(sp_id).map_err(|e| e.to_string())?;
    let guard = state.lock().await;
    guard
        .library
        .delete_smart_playlist(&server_id, &id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn evaluate_smart_playlist(
    state: SharedState<'_>,
    sp_id: String,
) -> Result<Vec<Track>, String> {
    let server_id = active_server_id(&state).await;
    let id = SmartPlaylistId::try_new(sp_id).map_err(|e| e.to_string())?;
    let guard = state.lock().await;
    let playlists = guard
        .library
        .list_smart_playlists(&server_id)
        .map_err(|e| e.to_string())?;
    let sp = playlists
        .iter()
        .find(|p| p.id.as_str() == id.as_str())
        .ok_or_else(|| "Smart playlist not found".to_string())?;
    guard
        .library
        .evaluate_smart_playlist(&server_id, sp)
        .map_err(|e| e.to_string())
}

// ─── Internal event helpers ─────────────────────────────────────

/// Advance the queue when the rodio sink runs dry. Honors the queue's
/// repeat mode:
///   * `One`   — re-play the current track from position 0.
///   * `All`   — wrap to the first entry and play it.
///   * `Off`   — move to the next entry; if there isn't one, stop.
///
/// Resolves the next track's stream URI through the active provider
/// the same way `play_track` / `next` do. Failures are logged and
/// degraded gracefully (e.g. a missing stream URI uses the cached
/// duration so the seekbar still reflects the right length).
pub async fn advance_queue_on_end(state: &Arc<Mutex<AppState>>, app: &tauri::AppHandle) {
    enum AdvanceAction {
        Play(sinfonic_domain::QueueEntry),
        Stop,
    }

    let action: AdvanceAction = {
        let mut guard = state.lock().await;
        match guard.queue.repeat() {
            RepeatMode::One => match guard.queue.current().cloned() {
                Some(entry) => AdvanceAction::Play(entry),
                None => AdvanceAction::Stop,
            },
            RepeatMode::All | RepeatMode::Off => {
                let advanced = guard.queue.next_track().cloned();
                match advanced {
                    Some(entry) => AdvanceAction::Play(entry),
                    None => AdvanceAction::Stop,
                }
            }
        }
    };

    match action {
        AdvanceAction::Play(entry) => {
            playback_helpers::play_entry_from_queue_entry(app, state, entry).await;
        }
        AdvanceAction::Stop => {
            {
                let guard = state.lock().await;
                guard.player.stop();
            }
            emit_queue_changed(app, state).await;
            emit_playback_state(app, state).await;
        }
    }
}

async fn emit_playback_state(app: &tauri::AppHandle, state: &Arc<Mutex<AppState>>) {
    let payload = {
        let guard = state.lock().await;
        PlaybackStatePayload::from_state(&guard.player.cached_state(), &guard.queue)
    };
    let _ = app.emit(EventName::PlaybackStateChanged.as_str(), payload);
}

async fn emit_queue_changed(app: &tauri::AppHandle, state: &Arc<Mutex<AppState>>) {
    // Build the payload AND compute the play-context "remaining"
    // counter inside the same lock so the snapshot we persist
    // matches the snapshot we emit. The library handle is borrowed
    // from inside the guard; persisting happens after the lock is
    // released (rusqlite is sync and may block on the pool).
    let (payload, snap_to_persist) = {
        let guard = state.lock().await;
        let snap = guard.queue.snapshot();
        let remaining = persist_helpers::context_remaining(&guard.library, &snap);
        let payload = QueueSnapshotPayload {
            server_id: snap.server_id.as_ref().map(|s| s.as_str().to_string()),
            entries: snap
                .entries
                .iter()
                .map(|e| crate::events::QueueEntryView {
                    id: e.id.as_str().to_string(),
                    track_id: e.track_id.as_str().to_string(),
                    title: e.title.clone(),
                    artist: e.artist.clone(),
                    album: e.album.clone(),
                    duration_seconds: e.duration_seconds,
                })
                .collect(),
            current_index: snap.current_index,
            repeat: snap.repeat,
            shuffle: snap.shuffle,
            // Number of additional tracks available from the play
            // context, if any. The frontend uses this for the "+N
            // más" affordance in the QueuePanel. Computed from the
            // library cache + context so we don't have to ship the
            // whole track list over the wire.
            context_remaining: remaining,
        };
        (payload, snap)
    };
    let _ = app.emit(EventName::QueueChanged.as_str(), payload);
    // Persist the snapshot to disk after the event is emitted so a
    // crash between emit and persist doesn't leave the UI ahead of
    // the on-disk state. Failures are logged and dropped — the
    // in-memory queue is still authoritative until the next launch.
    persist_helpers::persist_queue(state, snap_to_persist).await;
}

async fn emit_track_changed(
    app: &tauri::AppHandle,
    state: &Arc<Mutex<AppState>>,
    track: &Track,
) {
    let payload = TrackChangedPayload {
        track_id: track.id.as_str().to_string(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
    };
    let _ = app.emit(EventName::TrackChanged.as_str(), payload);
    let _ = state;
}

fn emit_track_changed_from_entry(
    app: &tauri::AppHandle,
    entry: &sinfonic_domain::QueueEntry,
) {
    let payload = TrackChangedPayload {
        track_id: entry.track_id.as_str().to_string(),
        title: entry.title.clone(),
        artist: entry.artist.clone(),
        album: entry.album.clone(),
    };
    let _ = app.emit(EventName::TrackChanged.as_str(), payload);
}

// Kept for future use by the player when the resolved track differs
// from the queue entry (e.g., the entry was restored without metadata).

mod persist_helpers {
    //! Cross-session queue snapshot persistence.
    //!
    //! Two helpers live here so `emit_queue_changed` doesn't grow
    //! another 30 lines of inline plumbing:
    //!
    //! - [`persist_queue`] writes the current `QueueSnapshot` to the
    //!   `queue_snapshots` table, scoped by the queue's `server_id`.
    //!   No-op when:
    //!     - the queue has no server anchor (just logged out / about
    //!       to log in),
    //!     - `persist_guard` is set (active during
    //!       `teardown_active_provider` so a teardown doesn't wipe
    //!       the previous server's snapshot with an empty queue).
    //!   Failures are logged and dropped; the in-memory queue is
    //!   still authoritative until the next launch.
    //!
    //! - [`context_remaining`] resolves how many additional tracks
    //!   the user could pull from the active `PlayContext` (album /
    //!   playlist / favourites) minus the ones already in the queue.
    //!   Powers the QueuePanel "+N más" affordance.

    use std::sync::atomic::Ordering;

    use sinfonic_domain::{PlayContext, QueueSnapshot, ServerId, TrackId};

    use super::*;

    pub(super) async fn persist_queue(
        state: &Arc<Mutex<AppState>>,
        snapshot: QueueSnapshot,
    ) {
        let (server_id, library) = {
            let guard = state.lock().await;
            if guard.persist_guard.load(Ordering::Acquire) {
                return;
            }
            let Some(server_id) = guard.queue.server_id().cloned() else {
                return;
            };
            (server_id, guard.library.clone())
        };
        if let Err(e) = library.save_queue_snapshot(&server_id, &snapshot) {
            tracing::warn!(
                target: "sinfonic::commands",
                error = %e,
                server_id = %server_id,
                "queue snapshot persist failed"
            );
        }
    }

    pub(super) fn context_remaining(
        library: &sinfonic_library::Store,
        snapshot: &QueueSnapshot,
    ) -> Option<u32> {
        let context = snapshot.last_context.as_ref()?;
        let server_id = context_server_id(context, snapshot)?;
        match context {
            PlayContext::Album { album_id } => {
                let all = library
                    .list_album_tracks(&server_id, album_id)
                    .ok()
                    .unwrap_or_default();
                let already_in_queue: std::collections::HashSet<&TrackId> = snapshot
                    .entries
                    .iter()
                    .map(|e| &e.track_id)
                    .collect();
                let remaining = all
                    .iter()
                    .filter(|t| !already_in_queue.contains(&t.id))
                    .count();
                Some(remaining as u32)
            }
            PlayContext::Playlist { playlist_id, .. } => {
                let track_ids = library
                    .list_playlist_tracks(&server_id, playlist_id)
                    .ok()
                    .unwrap_or_default();
                let already_in_queue: std::collections::HashSet<&str> = snapshot
                    .entries
                    .iter()
                    .map(|e| e.track_id.as_str())
                    .collect();
                let remaining = track_ids
                    .iter()
                    .filter(|id| !already_in_queue.contains(id.as_str()))
                    .count();
                Some(remaining as u32)
            }
            PlayContext::Favorites { .. } => {
                let (tracks, _, _) = library
                    .get_favorites(&server_id)
                    .ok()
                    .unwrap_or_default();
                let already_in_queue: std::collections::HashSet<&TrackId> = snapshot
                    .entries
                    .iter()
                    .map(|e| &e.track_id)
                    .collect();
                let remaining = tracks
                    .iter()
                    .filter(|t| !already_in_queue.contains(&t.id))
                    .count();
                Some(remaining as u32)
            }
            PlayContext::All { .. } => {
                // Count tracks after the current one in title-ascending
                // order, skipping duplicates already in the queue.
                // Paginates up to MAX_PAGES to keep the count bounded.
                let already: std::collections::HashSet<&str> = snapshot
                    .entries
                    .iter()
                    .map(|e| e.track_id.as_str())
                    .collect();
                let current_id = snapshot
                    .entries
                    .get(snapshot.current_index?)
                    .map(|e| e.track_id.as_str());

                const PAGE_SIZE: usize = 500;
                const MAX_PAGES: usize = 50;
                let mut offset: usize = 0;
                let mut found_current = current_id.is_none();
                let mut count: u32 = 0;
                let mut pages_read: usize = 0;

                while pages_read < MAX_PAGES {
                    pages_read += 1;
                    let page = match library.list_tracks(&server_id, offset, PAGE_SIZE) {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    let page_len = page.items.len();
                    if page_len == 0 {
                        break;
                    }
                    for track in page.items {
                        if !found_current {
                            if Some(track.id.as_str()) == current_id {
                                found_current = true;
                            }
                        } else if !already.contains(track.id.as_str()) {
                            count += 1;
                        }
                    }
                    if page_len < PAGE_SIZE {
                        break;
                    }
                    offset += page_len;
                }
                Some(count)
            }
        }
    }

    /// Resolve the `ServerId` for a context. For Album, the album
    /// could live under different servers in theory; we use the
    /// queue's current server anchor as the lookup key. For
    /// Playlist and Favorites the context carries the id explicitly.
    fn context_server_id(context: &PlayContext, snapshot: &QueueSnapshot) -> Option<ServerId> {
        match context {
            PlayContext::Album { .. } => snapshot.server_id.clone(),
            PlayContext::Playlist { server_id, .. }
            | PlayContext::Favorites { server_id }
            | PlayContext::All { server_id } => Some(server_id.clone()),
        }
    }
}

/// Open the settings window. Idempotent — if the window is already open,
/// this brings it to the foreground instead of creating a duplicate.
#[tauri::command]
pub async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::WebviewWindowBuilder;
    use tauri::WebviewUrl;

    const LABEL: &str = "settings";

    if let Some(win) = app.get_webview_window(LABEL) {
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("settings.html".into()))
        .title("Settings")
        .inner_size(720.0, 540.0)
        .min_inner_size(600.0, 400.0)
        .center()
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}
