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
use tauri::Manager;
use tokio::sync::Mutex;

mod commands;
mod events;
mod lastfm;
mod scrobble_watcher;
mod state;

pub use events::{
    EventName, LibrarySyncStatusPayload, PlaybackStatePayload, QueueSnapshotPayload,
    SyncProgressPayload, TrackChangedPayload,
};
pub use state::AppState;
pub use sinfonic_domain::RepeatMode;

/// Re-exported so integration tests can drive the sync pipeline
/// without going through Tauri.
pub use commands::sync_library_data;

/// Re-exported so integration tests can exercise the lyrics
/// orchestration without going through Tauri.
pub use commands::lookup_lyrics;

/// Entrypoint invoked by `main.rs` (and the mobile target on iOS/Android).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Resolve the app's per-user data directory and open
            // the library cache there. If the path is unavailable
            // (e.g. a misconfigured mobile sandbox), fall back to
            // the in-memory cache so the app still boots.
            let state = match app.path().app_data_dir() {
                Ok(dir) => {
                    if let Err(e) = std::fs::create_dir_all(&dir) {
                        tracing::warn!(
                            target: "sinfonic::app",
                            dir = %dir.display(),
                            error = %e,
                            "could not create app data dir; falling back to in-memory cache"
                        );
                        AppState::new()
                    } else {
                        let db = dir.join("library.sqlite");
                        let art_dir = dir.join("album_art");
                        AppState::with_paths(&db, &art_dir)
                            .unwrap_or_else(|e| {
                                tracing::warn!(
                                    target: "sinfonic::app",
                                    db = %db.display(),
                                    error = %e,
                                    "library sqlite open failed; falling back to in-memory cache"
                                );
                                AppState::new()
                            })
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target: "sinfonic::app",
                        error = %e,
                        "app_data_dir unavailable; using in-memory cache"
                    );
                    AppState::new()
                }
            };

            // Wrap the state in Arc<Mutex<>> first so the player's
            // event callback can reach the queue for auto-advance.
            // The callback is Fn + Send + Sync + 'static so it must
            // own its handles outright — we clone Arc handles and the
            // AppHandle before handing the closure off.
            //
            // The Tauri setup closure is `Fn` (not async) so we cannot
            // `await` the outer Mutex here. The AudioPlayer reference
            // is itself an `Arc<AudioPlayer>` — clone it out before
            // wrapping the state, then set the callback on the
            // standalone handle.
            let player = state.player.clone();
            let state_for_resume = Arc::new(Mutex::new(state));
            let setup_handle = state_for_resume.clone();
            let callback_handle = state_for_resume.clone();
            let app_handle = app.handle().clone();
            // `app_handle` is moved into the player event callback
            // below; clone once for the bootstrap path so both owners
            // can co-exist.
            let setup_app_handle = app_handle.clone();

            // Wire the AudioPlayer's events to Tauri. The player
            // only fires `PlayerEvent::TrackEnded` from the poller
            // thread now — every other state change (play / pause /
            // resume / stop / seek / set_volume / set_muted) emits
            // `playback-state-changed` synchronously from the
            // command that triggered it via `emit_playback_state`.
            //
            // The hot path is therefore free of `app.emit` and
            // unaffected by WKWebView's event-channel quirks on
            // macOS, which were the root cause of the position bar
            // freezing after the first snapshot. The frontend polls
            // `get_playback_state` every 250 ms to redraw the seek
            // bar; that's an IPC round-trip, not an event push.
            let advance_handle = callback_handle.clone();
            player.set_event_callback(move |event| {
                if let sinfonic_playback::PlayerEvent::TrackEnded { .. } = event {
                    let handle = advance_handle.clone();
                    let app = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        commands::advance_queue_on_end(&handle, &app).await;
                    });
                }
            });

            // Resume a previously-persisted Last.fm session, if any,
            // and spawn the scrobble watcher. Both run on the tokio
            // runtime because the Tauri setup closure is sync; network
            // errors are swallowed (we just stay disconnected until the
            // user re-enters credentials).
            tauri::async_runtime::spawn(async move {
                // 0) Restore the last-active media provider, if any.
                //    Runs before Last.fm because the scrobble watcher
                //    needs a populated queue / identity to make sense
                //    of any pending track changes. Failures are logged
                //    and dropped so the app still boots into the
                //    setup view when the persisted pointer is stale.
                commands::try_restore_provider(&setup_handle, &setup_app_handle).await;
                // 1) Resume a persisted session (cheap — does not
                //    block startup if Last.fm is unreachable).
                {
                    let state_ref = setup_handle.lock().await;
                    commands::try_resume_lastfm(&state_ref).await;
                }
                // 1b) Restore persisted playback configuration
                //     (crossfade on/off + duration). Done here so
                //     the first track the user plays already honours
                //     the saved preference. Both prefs are
                //     best-effort: a missing or unparseable value
                //     falls back to the AudioPlayer defaults
                //     (off, 6 s).
                {
                    let state_ref = setup_handle.lock().await;
                    let enabled = state_ref
                        .library
                        .get_preference("playback.crossfade_enabled")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(false);
                    let seconds = state_ref
                        .library
                        .get_preference("playback.crossfade_seconds")
                        .ok()
                        .flatten()
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(6);
                    state_ref.player.set_crossfade(enabled, seconds);
                    tracing::debug!(
                        target: "sinfonic::app",
                        enabled,
                        seconds,
                        "restored crossfade config"
                    );
                }
                // 2) Take clones of the bits the watcher needs, then
                //    hand them off. This keeps the watcher's mutex
                //    pressure off the IPC lock.
                let (queue_clone, player_clone, lastfm_clone, provider_clone) = {
                    let state_ref = setup_handle.lock().await;
                    (
                        Arc::new(Mutex::new(state_ref.queue.clone())),
                        state_ref.player.clone(),
                        state_ref.lastfm.clone(),
                        state_ref.provider.clone(),
                    )
                };
                scrobble_watcher::run(queue_clone, player_clone, lastfm_clone, provider_clone).await;
            });

            app.manage(state_for_resume);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Library reads (Phase 2)
            commands::get_albums,
            commands::get_artists,
            commands::get_genres,
            commands::get_albums_by_genre,
            commands::get_tracks_by_genre,
            commands::get_tracks,
            commands::get_album,
            commands::get_album_detail,
            // Provider-direct reads (Phase 1 of feature/direct-fetch-providers).
            // Bypass the SQLite cache and hit the upstream server so
            // the UI doesn't wait for `provider_sync_library`.
            commands::provider_list_albums,
            commands::provider_list_artists,
            commands::provider_list_tracks,
            commands::provider_album_detail,
            commands::provider_artist_detail,
            commands::provider_playlist_detail,
            // Phase 3 of feature/direct-fetch-providers: spawn the
            // Subsonic album-tracks background sync. Idempotent —
            // re-entry while one is running is a no-op.
            commands::kick_subsonic_background_sync,
            // Phase 3 parity for Jellyfin: identical contract
            // (fire-and-forget, idempotent) over
            // `sync_library_data` instead of the album fan-out.
            commands::kick_jellyfin_background_sync,
            commands::play_album,
            commands::play_album_with_context,
            // Playback (Phase 1 + Phase 4 audio)
            commands::get_playback_state,
            commands::get_queue,
            commands::play_track,
            commands::play_track_with_context,
            commands::queue_play_now,
            commands::queue_play_next,
            commands::queue_add,
            commands::queue_remove,
            commands::queue_jump_to,
            commands::queue_move,
            commands::queue_clear,
            commands::queue_extend_more,
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
            // Crossfade (Phase 3)
            commands::set_crossfade,
            commands::get_crossfade_config,
            // Search (Phase 2)
            commands::search,
            // Provider (Phase 3 + Phase 5)
            commands::jellyfin_discover,
            commands::jellyfin_login,
            commands::subsonic_login,
            commands::provider_logout,
            commands::provider_delete,
            commands::provider_servers,
            commands::provider_active_server,
            commands::provider_set_active,
            commands::provider_sync_library,
            commands::bootstrap_state,
            // Album art (Phase 7)
            commands::provider_image_bytes,
            commands::provider_image_bytes_bulk,
            // Lyrics
            commands::get_lyrics,
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

/// Initialise the global tracing subscriber.
///
/// Honours `RUST_LOG` (e.g. `RUST_LOG=sinfonic::sync=debug`). If no
/// filter env var is set, defaults to `info` for `sinfonic::*` and
/// `warn` for everything else. Idempotent — repeated calls (e.g. in
/// integration tests) are a no-op via the `try_init` guard.
fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,sinfonic=debug")
    });

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).with_ansi(false))
        .try_init();
}
