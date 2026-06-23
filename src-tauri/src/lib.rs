//! Sinfonic Tauri app entry point.
//!
//! All logic lives here (not in `main.rs`) so mobile builds can hook
//! the `mobile_entry_point` attribute. `main.rs` is a thin passthrough.
//!
//! # Phase status
//!
//! - Phase 0: every Tauri command is a stub returning `Result<T, String>`
//!   so the IPC surface compiles and the frontend can start wiring.
//! - Phase 1: the queue + playback commands (`queue_*`, `play_track`,
//!   `next`, `previous`, `pause`, `resume`, `set_repeat`, `set_shuffle`,
//!   `set_volume`, `set_muted`, `seek`, `stop`) operate on the
//!   in-memory `AppState` and emit real events.
//! - Phases 2–3: library cache (rusqlite) and Jellyfin provider land.
//! - Phase 4: rodio-backed `AudioPlayer` replaces the stub. Stream URIs
//!   from the active provider are resolved and pumped through a
//!   Symphonia decoder, optional 10-band graphic EQ, and out to the
//!   default OS sink. A background poller thread emits
//!   `playback-state-changed` every 250ms and `track-changed` on end.

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

mod commands;
mod events;
mod lastfm;
mod scrobble_watcher;
mod state;

pub use events::{
    EventName, LibrarySyncStatusPayload, PlaybackStatePayload, QueueSnapshotPayload,
    TrackChangedPayload,
};
pub use state::AppState;

/// Entrypoint invoked by `main.rs` (and the mobile target on iOS/Android).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            // Resolve the app's per-user data directory and open
            // the library cache there. If the path is unavailable
            // (e.g. a misconfigured mobile sandbox), fall back to
            // the in-memory cache so the app still boots.
            let state = match app.path().app_data_dir() {
                Ok(dir) => {
                    if let Err(e) = std::fs::create_dir_all(&dir) {
                        eprintln!("sinfonic: could not create {dir:?}: {e}");
                        AppState::new()
                    } else {
                        let db = dir.join("library.sqlite");
                        let art_dir = dir.join("album_art");
                        AppState::with_paths(&db, &art_dir)
                            .unwrap_or_else(|e| {
                                eprintln!("sinfonic: open({db:?}) failed: {e}, falling back to memory");
                                AppState::new()
                            })
                    }
                }
                Err(e) => {
                    eprintln!("sinfonic: app_data_dir unavailable: {e}, using in-memory cache");
                    AppState::new()
                }
            };

            // Wire the AudioPlayer's events to Tauri. The player emits
            // PlayerEvent::StateChanged on every position poll and
            // PlayerEvent::TrackEnded when the rodio sink runs dry;
            // we forward both to the webview as typed events.
            let app_handle = app.handle().clone();
            state.player.set_event_callback(move |event| {
                match event {
                    sinfonic_playback::PlayerEvent::StateChanged {
                        track_id,
                        position_seconds,
                        is_playing,
                        volume,
                        muted,
                        duration_seconds,
                    } => {
                        let payload = PlaybackStatePayload {
                            is_playing,
                            position_seconds,
                            duration_seconds,
                            volume,
                            muted,
                            ..Default::default()
                        };
                        let _ = app_handle.emit(EventName::PlaybackStateChanged.as_str(), &payload);
                        if let Some(track_id) = track_id {
                            let _ = app_handle.emit(
                                "track-position",
                                &serde_json::json!({
                                    "trackId": track_id,
                                    "positionSeconds": position_seconds,
                                }),
                            );
                        }
                    }
                    sinfonic_playback::PlayerEvent::TrackEnded { track_id } => {
                        let _ = app_handle.emit(
                            "track-ended",
                            &serde_json::json!({ "trackId": track_id }),
                        );
                    }
                }
            });

            // Resume a previously-persisted Last.fm session, if any,
            // and spawn the scrobble watcher. Both run on the tokio
            // runtime because the Tauri setup closure is sync; network
            // errors are swallowed (we just stay disconnected until the
            // user re-enters credentials).
            let state_for_resume = Arc::new(Mutex::new(state));
            let setup_handle = state_for_resume.clone();
            tauri::async_runtime::spawn(async move {
                // 1) Resume a persisted session (cheap — does not
                //    block startup if Last.fm is unreachable).
                {
                    let state_ref = setup_handle.lock().await;
                    commands::try_resume_lastfm(&state_ref).await;
                }
                // 2) Take clones of the bits the watcher needs, then
                //    hand them off. This keeps the watcher's mutex
                //    pressure off the IPC lock.
                let (queue_clone, player_clone, lastfm_clone) = {
                    let state_ref = setup_handle.lock().await;
                    (
                        Arc::new(Mutex::new(state_ref.queue.clone())),
                        state_ref.player.clone(),
                        state_ref.lastfm.clone(),
                    )
                };
                scrobble_watcher::run(queue_clone, player_clone, lastfm_clone).await;
            });

            app.manage(state_for_resume);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            // Library reads (Phase 2)
            commands::get_albums,
            commands::get_artists,
            commands::get_tracks,
            commands::get_album_detail,
            commands::play_album,
            // Playback (Phase 1 + Phase 4 audio)
            commands::get_playback_state,
            commands::get_queue,
            commands::play_track,
            commands::queue_play_now,
            commands::queue_play_next,
            commands::queue_add,
            commands::queue_remove,
            commands::queue_jump_to,
            commands::queue_move,
            commands::queue_clear,
            // Queue bulk + Playlist CRUD (Phase 9)
            commands::queue_add_many,
            commands::queue_play_next_many,
            commands::playlists_get,
            commands::playlist_detail,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_playlist_tracks,
            commands::remove_playlist_entries,
            commands::move_playlist_entry,
            // Favorites (Phase 9)
            commands::set_track_favorite,
            commands::set_album_favorite,
            commands::set_artist_favorite,
            commands::get_favorites,
            // Smart Playlists (Phase 9)
            commands::get_smart_playlists,
            commands::create_smart_playlist,
            commands::delete_smart_playlist,
            commands::evaluate_smart_playlist,
            commands::set_repeat,
            commands::set_shuffle,
            commands::pause,
            commands::resume,
            commands::stop,
            commands::next,
            commands::previous,
            commands::seek,
            commands::set_volume,
            commands::set_muted,
            commands::set_eq_band,
            commands::reset_eq,
            commands::get_eq_bands,
            // Search (Phase 2)
            commands::search,
            // Provider (Phase 3 + Phase 5)
            commands::jellyfin_discover,
            commands::jellyfin_login,
            commands::subsonic_login,
            commands::provider_logout,
            commands::provider_servers,
            commands::provider_active_server,
            commands::provider_sync_library,
            // Album art (Phase 7)
            commands::provider_image_bytes,
            // Local files (Phase 8)
            commands::local_login,
            commands::local_rescan,
            // Last.fm (Phase 7)
            commands::lastfm_connect,
            commands::lastfm_disconnect,
            commands::lastfm_status,
            commands::open_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
