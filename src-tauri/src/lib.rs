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
//!   in-memory `AppState` and emit real events. Audio playback is still
//!   stubbed (Phase 4 wires rodio).
//! - Phases 2–3: library cache (rusqlite) and Jellyfin provider land.

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

mod commands;
mod events;
mod state;

pub use events::{EventName, PlaybackStatePayload, QueueSnapshotPayload, TrackChangedPayload};
pub use state::AppState;

/// Entrypoint invoked by `main.rs` (and the mobile target on iOS/Android).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::new();
            app.manage(Arc::new(Mutex::new(state)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            // Library reads (Phase 2)
            commands::get_albums,
            commands::get_artists,
            commands::get_tracks,
            // Playback (Phase 1, in-memory)
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
            // Search (Phase 2)
            commands::search,
            // Jellyfin (Phase 3)
            commands::jellyfin_discover,
            commands::jellyfin_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
