//! Tauri command surface.
//!
//! Every command returns `Result<T, String>` so we can send
//! human-readable error messages to the frontend without a custom
//! `Serialize` impl on each error type. This is the pattern recommended
//! by the tauri-v2 skill and keeps the IPC boundary trivial.
//!
//! # Phase 1 status
//!
//! Queue mutations (`queue_add`, `queue_play_next`, `queue_clear`,
//! `queue_remove`, `queue_jump_to`, `queue_move`, `play_track`,
//! `next`, `previous`, `set_repeat`, `set_shuffle`, `set_volume`,
//! `set_muted`, `pause`, `resume`, `stop`, `seek`) operate on the
//! in-memory `AppState` only. They return real data and emit the
//! correct events, but no audio actually plays — the real rodio
//! player lands in Phase 4.
//!
//! Library reads (`get_albums`, `get_artists`, `get_tracks`,
//! `search`) and provider flows (`jellyfin_discover`,
//! `jellyfin_login`) still return `Err("not implemented")` until
//! Phases 2 and 3 land.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::events::{
    EventName, PlaybackStatePayload, QueueSnapshotPayload, TrackChangedPayload,
};
use crate::state::AppState;
use sinfonic_domain::{
    Album, Artist, PagedResponse, QueueEntryId, QueueSnapshot, RepeatMode, SearchResults, ServerId,
    Track, TrackId,
};

type SharedState<'a> = State<'a, Arc<Mutex<AppState>>>;

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

// ─── Playback (Phase 1: in-memory only) ─────────────────────────

#[tauri::command]
pub async fn get_playback_state(
    state: SharedState<'_>,
) -> Result<PlaybackStatePayload, String> {
    let guard = state.lock().await;
    Ok(PlaybackStatePayload::from_state(&guard.playback, &guard.queue))
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
    let ids = {
        let mut guard = state.lock().await;
        guard.queue.play_now(std::slice::from_ref(&track))
    };
    let id = ids
        .into_iter()
        .next()
        .ok_or_else(|| "play_track: empty track list".to_string())?;

    emit_queue_changed(&app, &state).await;
    emit_track_changed(&app, &state, &track).await;
    emit_playback_state(&app, &state).await;
    Ok(id)
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
        guard.playback.pause();
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn resume(app: tauri::AppHandle, state: SharedState<'_>) -> Result<(), String> {
    {
        let mut guard = state.lock().await;
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
        // Queue ended — stop the playhead.
        {
            let mut guard = state.lock().await;
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
        guard.playback.set_muted(muted);
    }
    emit_playback_state(&app, &state).await;
    Ok(())
}

// ─── Search (Phase 2) ───────────────────────────────────────────

#[tauri::command]
pub async fn search(query: String) -> Result<SearchResults, String> {
    let _ = query;
    Ok(SearchResults::default())
}

// ─── Jellyfin provider (Phase 3) ────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredServer {
    pub name: String,
    pub base_url: String,
    pub server_id: String,
}

#[tauri::command]
pub async fn jellyfin_discover() -> Result<Vec<DiscoveredServer>, String> {
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
