//! Tauri command surface.
//!
//! Every command returns `Result<T, String>` so we can send
//! human-readable error messages to the frontend without a custom
//! `Serialize` impl on each error type. This is the pattern recommended
//! by the tauri-v2 skill and keeps the IPC boundary trivial.
//!
//! All commands are stubs in Phase 0: they compile, accept their
//! parameters, and return `Err("not implemented in skeleton")`. The
//! real bodies land as each feature phase lands.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::events::{EventName, PlaybackStatePayload, TrackChangedPayload};
use crate::state::AppState;
use sinfonic_domain::{Album, Artist, PagedResponse, QueueSnapshot, SearchResults, ServerId, Track};

type SharedState<'a> = State<'a, Arc<Mutex<AppState>>>;

// ─── Greet (kept from the original scaffold) ────────────────────

#[tauri::command]
pub fn greet(name: String) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

// ─── Library queries ────────────────────────────────────────────

#[tauri::command]
pub async fn get_albums(
    offset: usize,
    limit: usize,
    _state: SharedState<'_>,
) -> Result<PagedResponse<Album>, String> {
    let _ = (offset, limit);
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn get_artists(
    offset: usize,
    limit: usize,
    _state: SharedState<'_>,
) -> Result<PagedResponse<Artist>, String> {
    let _ = (offset, limit);
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn get_tracks(
    offset: usize,
    limit: usize,
    _state: SharedState<'_>,
) -> Result<PagedResponse<Track>, String> {
    let _ = (offset, limit);
    Err("not implemented in skeleton".into())
}

// ─── Playback ───────────────────────────────────────────────────

#[tauri::command]
pub async fn get_playback_state(
    _state: SharedState<'_>,
) -> Result<PlaybackStatePayload, String> {
    Ok(PlaybackStatePayload::default())
}

#[tauri::command]
pub async fn get_queue(_state: SharedState<'_>) -> Result<QueueSnapshot, String> {
    Ok(QueueSnapshot::default())
}

#[tauri::command]
pub async fn play_track(
    track_id: String,
    app: tauri::AppHandle,
    _state: SharedState<'_>,
) -> Result<(), String> {
    // Stub: emit a fake track-changed so the UI wiring is testable.
    let _ = app.emit(
        EventName::TrackChanged.as_str(),
        TrackChangedPayload {
            track_id,
            title: "Stub track".into(),
            artist: "Stub artist".into(),
            album: "Stub album".into(),
        },
    );
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn pause(app: tauri::AppHandle) -> Result<(), String> {
    let _ = app.emit(EventName::PlaybackStateChanged.as_str(), PlaybackStatePayload::paused());
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn resume(app: tauri::AppHandle) -> Result<(), String> {
    let _ = app.emit(
        EventName::PlaybackStateChanged.as_str(),
        PlaybackStatePayload::playing(),
    );
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn stop(app: tauri::AppHandle) -> Result<(), String> {
    let _ = app.emit(
        EventName::PlaybackStateChanged.as_str(),
        PlaybackStatePayload::default(),
    );
    Ok(())
}

#[tauri::command]
pub async fn next(_state: SharedState<'_>) -> Result<(), String> {
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn previous(_state: SharedState<'_>) -> Result<(), String> {
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn seek(_position_seconds: u32) -> Result<(), String> {
    Err("not implemented in skeleton".into())
}

#[tauri::command]
pub async fn set_volume(volume: f32, app: tauri::AppHandle) -> Result<(), String> {
    let _ = volume.clamp(0.0, 1.0);
    let _ = app.emit(
        EventName::PlaybackStateChanged.as_str(),
        PlaybackStatePayload::default(),
    );
    Ok(())
}

#[tauri::command]
pub async fn set_muted(muted: bool) -> Result<(), String> {
    let _ = muted;
    Ok(())
}

// ─── Search ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn search(query: String) -> Result<SearchResults, String> {
    let _ = query;
    Ok(SearchResults::default())
}

// ─── Jellyfin provider ──────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredServer {
    pub name: String,
    pub base_url: String,
    pub server_id: String,
}

#[tauri::command]
pub async fn jellyfin_discover() -> Result<Vec<DiscoveredServer>, String> {
    // Phase 3: hook source-jellyfin's UDP discovery.
    Ok(Vec::new())
}

#[tauri::command]
pub async fn jellyfin_login(
    base_url: String,
    username: String,
    password: String,
) -> Result<ServerId, String> {
    let _ = (base_url, username, password);
    Err("Jellyfin login not implemented in skeleton".into())
}
