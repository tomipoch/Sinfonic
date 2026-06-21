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
//!   prefer the active Jellyfin `ServerId` if a session is
//!   connected, falling back to the placeholder otherwise.
//! - Phase 4: `play_track` resolves the track's stream URI from the
//!   active provider and pipes it through `AudioPlayer` (rodio +
//!   Symphonia + 10-band EQ). `pause`, `resume`, `seek`,
//!   `set_volume`, `set_muted`, `set_eq_band`, `reset_eq` all drive
//!   the AudioPlayer too. `get_playback_state` reflects the rodio
//!   sink's position.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::events::{
    EventName, LibrarySyncStatusPayload, PlaybackStatePayload, QueueSnapshotPayload,
    TrackChangedPayload,
};
use crate::state::AppState;
use sinfonic_domain::{
    Album, Artist, PagedResponse, QueueEntryId, QueueSnapshot, RepeatMode, SearchResults, ServerId,
    Track, TrackId,
};
use sinfonic_secrets::SecretStore;
use sinfonic_source::MusicProvider;
use sinfonic_source_jellyfin::auth::{login as jellyfin_login_inner, LoginRequest};

type SharedState<'a> = State<'a, Arc<Mutex<AppState>>>;

/// Placeholder `ServerId` used when no Jellyfin session is active.
/// Library reads return empty pages in that state instead of erroring
/// — the UI surfaces a "connect a server" hint.
const DEFAULT_SERVER_ID: &str = "server-local";

fn default_server_id() -> ServerId {
    ServerId::new(DEFAULT_SERVER_ID)
}

/// Return the `ServerId` of the active Jellyfin provider if one is
/// connected, otherwise the placeholder. Used by library reads so
/// they automatically follow the active session.
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

// ─── Greet (kept from the original scaffold) ────────────────────

#[tauri::command]
pub fn greet(name: String) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
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
    let id = {
        let mut guard = state.lock().await;
        let ids = guard.queue.play_now(std::slice::from_ref(&track));
        ids.into_iter()
            .next()
            .ok_or_else(|| "play_track: empty track list".to_string())?
    };

    // Resolve the stream URI from the active Jellyfin provider and
    // hand it to the rodio-backed AudioPlayer. If no provider is
    // connected we still register the track in the queue (so the UI
    // shows it as "next") but skip the actual audio — useful for
    // offline browsing and tests.
    let stream_uri = resolve_stream_uri(&state, &track).await;
    let track_id = track.id.clone();
    let duration_seconds = match stream_uri.as_deref() {
        Some(uri) => {
            let guard = state.lock().await;
            match guard.player.play(track_id.clone(), uri) {
                Ok(duration) => duration,
                Err(e) => {
                    eprintln!("sinfonic: player.play failed: {e}");
                    track.duration_seconds
                }
            }
        }
        None => 0,
    };

    // Sync the in-memory PlaybackState mirror so commands that read
    // it (without going through the player) stay consistent.
    {
        let mut guard = state.lock().await;
        guard.playback.start(duration_seconds);
    }

    emit_queue_changed(&app, &state).await;
    emit_track_changed(&app, &state, &track).await;
    emit_playback_state(&app, &state).await;
    Ok(id)
}

/// Ask the active Jellyfin provider for the track's stream URI.
/// Returns `None` if no provider is connected (the UI will show
/// "no source connected") or if the provider fails to resolve the URI.
async fn resolve_stream_uri(state: &SharedState<'_>, track: &Track) -> Option<String> {
    let guard = state.lock().await;
    let provider_guard = guard.provider.lock().await;
    let provider = provider_guard.as_ref()?;
    let track_id = track.id.clone();
    let descriptor = provider.stream(&track_id).await.ok()?;
    Some(descriptor.uri().to_string())
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

    emit_queue_changed(&app, &state).await;
    if let Some(first) = tracks.first() {
        emit_track_changed(&app, &state, first).await;
    }
    emit_playback_state(&app, &state).await;
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
    emit_queue_changed(&app, &state).await;
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
    emit_queue_changed(&app, &state).await;
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
        emit_queue_changed(&app, &state).await;
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
        emit_queue_changed(&app, &state).await;
        if let Some(entry) = {
            let guard = state.lock().await;
            guard.queue.current().cloned()
        } {
            emit_track_changed_from_entry(&app, &entry);
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
    emit_queue_changed(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn queue_clear(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.queue.clear();
        guard.player.stop();
        guard.playback.stop();
    }
    emit_queue_changed(&app, &state).await;
    emit_playback_state(&app, &state).await;
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
    emit_playback_state(&app, &state).await;
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
    emit_queue_changed(&app, &state).await;
    emit_playback_state(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn pause(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.player.pause();
        guard.playback.pause();
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn resume(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.player.resume();
        guard.playback.resume();
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn stop(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.player.stop();
        guard.playback.stop();
    }
    emit_playback_state(&app, &state).await;
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
    if let Some(entry) = next_entry {
        let duration = entry.duration_seconds;
        {
            let mut guard = state.lock().await;
            guard.playback.start(duration);
        }
        emit_track_changed_from_entry(&app, &entry);
        emit_queue_changed(&app, &state).await;
        emit_playback_state(&app, &state).await;
    } else {
        // Queue ended — stop the playhead and the rodio sink.
        {
            let mut guard = state.lock().await;
            guard.player.stop();
            guard.playback.stop();
        }
        emit_playback_state(&app, &state).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn previous(
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    let prev_entry = {
        let mut guard = state.lock().await;
        guard.queue.previous_track().cloned()
    };
    if let Some(entry) = prev_entry {
        let duration = entry.duration_seconds;
        {
            let mut guard = state.lock().await;
            guard.playback.start(duration);
        }
        emit_track_changed_from_entry(&app, &entry);
        emit_queue_changed(&app, &state).await;
        emit_playback_state(&app, &state).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn seek(
    position_seconds: u32,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.player.seek(position_seconds);
        guard.playback.seek(position_seconds);
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn set_volume(
    volume: f32,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.player.set_volume(volume);
        guard.playback.set_volume(volume);
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn set_muted(
    muted: bool,
    app: tauri::AppHandle,
    state: SharedState<'_>,
) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
        guard.player.set_muted(muted);
        guard.playback.set_muted(muted);
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

// ─── Equalizer (Phase 4) ────────────────────────────────────────

#[derive(Clone, Debug, Deserialize, Serialize)]
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

// ─── Jellyfin provider (Phase 3) ────────────────────────────────

/// Wire-format mirror of `sinfonic_source_jellyfin::discovery::DiscoveredJellyfinServer`.
/// Kept here so the frontend never imports the crate directly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredServer {
    pub name: String,
    pub base_url: String,
    pub server_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectedServer {
    pub server_id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JellyfinLoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

/// Discover Jellyfin servers on the local network. Listens on UDP
/// 7359 for ~1.5s and falls back to a localhost probe if nothing
/// answers.
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

    let login_request = LoginRequest {
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

    // Upsert the server row so `servers` knows about it before we
    // hand the provider to the library cache.
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
            .upsert_server(&success.server_id, "jellyfin", &server_name, &request.base_url)
            .map_err(|e| format!("upsert server: {e}"))?;
        *guard.provider.lock().await = Some(provider);
    }

    Ok(ConnectedServer {
        server_id: success.server_id.to_string(),
        kind: "jellyfin".into(),
        name: server_name,
        base_url: request.base_url,
    })
}

/// Clear the active Jellyfin provider and remove its token from the
/// keyring. Library data is left in place so the user can log back in
/// without a full re-sync. Audio playback is stopped — the stream URL
/// the rodio sink is consuming will no longer resolve after logout.
#[tauri::command]
pub async fn jellyfin_logout(state: SharedState<'_>) -> Result<(), String> {
    let (server_id_opt, secrets) = {
        let mut guard = state.lock().await;
        let server_id = guard
            .provider
            .lock()
            .await
            .as_ref()
            .map(|p| p.identity().server_id.clone());
        *guard.provider.lock().await = None;
        guard.player.stop();
        guard.playback.stop();
        (server_id, guard.secrets.clone())
    };
    if let Some(server_id) = server_id_opt {
        let _ = secrets.delete_token(server_id).await;
    }
    Ok(())
}

/// Trigger a sync: fetch the first page of albums / artists / tracks
/// from the active provider and pipe them into the SQLite cache.
/// Subsequent reads serve from the cache.
#[tauri::command]
pub async fn jellyfin_sync_library(
    state: SharedState<'_>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use sinfonic_domain::PagedRequest;

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
            .ok_or_else(|| "no active Jellyfin session".to_string())?;
        (provider, guard.library.clone())
    };

    let server_id = provider_snapshot.identity().server_id.clone();

    let albums = provider_snapshot
        .albums(PagedRequest::new(0, 200))
        .await
        .map_err(|e| format!("albums: {e}"))?;
    library_handle
        .replace_albums(&server_id, &albums.items)
        .map_err(|e| format!("upsert albums: {e}"))?;

    let artists = provider_snapshot
        .artists(PagedRequest::new(0, 200))
        .await
        .map_err(|e| format!("artists: {e}"))?;
    library_handle
        .replace_artists(&server_id, &artists.items)
        .map_err(|e| format!("upsert artists: {e}"))?;

    let tracks = provider_snapshot
        .tracks(PagedRequest::new(0, 500))
        .await
        .map_err(|e| format!("tracks: {e}"))?;
    library_handle
        .replace_tracks(&server_id, &tracks.items)
        .map_err(|e| format!("upsert tracks: {e}"))?;

    let payload_done = LibrarySyncStatusPayload {
        server_id: Some(server_id.to_string()),
        state: "complete".into(),
        progress: 1.0,
    };
    let _ = app.emit(EventName::LibrarySyncStatus.as_str(), payload_done);

    Ok(())
}

/// Return the list of servers the user has configured (rows in the
/// `servers` table). The active server — if any — is the first entry.
#[tauri::command]
pub async fn jellyfin_servers(
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

/// Surface the active server id (or `null` if none). Used by the
/// frontend to keep the Zustand store in sync.
#[tauri::command]
pub async fn jellyfin_active_server(
    state: SharedState<'_>,
) -> Result<Option<String>, String> {
    let guard = state.lock().await;
    let provider = guard.provider.lock().await;
    Ok(provider
        .as_ref()
        .map(|p| p.identity().server_id.to_string()))
}

// ─── Internal event helpers ─────────────────────────────────────

async fn emit_playback_state(app: &tauri::AppHandle, state: &SharedState<'_>) {
    let payload = {
        let guard = state.lock().await;
        PlaybackStatePayload::from_state(&guard.playback, &guard.queue)
    };
    let _ = app.emit(EventName::PlaybackStateChanged.as_str(), payload);
}

async fn emit_queue_changed(app: &tauri::AppHandle, state: &SharedState<'_>) {
    let payload = {
        let guard = state.lock().await;
        let snap = guard.queue.snapshot();
        QueueSnapshotPayload {
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
        }
    };
    let _ = app.emit(EventName::QueueChanged.as_str(), payload);
}

async fn emit_track_changed(
    app: &tauri::AppHandle,
    state: &SharedState<'_>,
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
#[allow(dead_code)]
fn _ensure_track_id_used(_: TrackId) {}
