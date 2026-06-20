//! Sinfonic Tauri app entry point.
//!
//! All logic lives here (not in `main.rs`) so mobile builds can hook
//! the `mobile_entry_point` attribute. `main.rs` is a thin passthrough.
//!
//! Phase 0 (Fase 0): every Tauri command is a stub returning
//! `Result<T, String>` so the IPC surface compiles and the frontend can
//! start wiring. The bodies land in subsequent phases (Jellyfin auth,
//! playback, library cache, etc.).

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
            // Initialise the shared application state. Phase 0 keeps it
            // empty; later phases plug in the store, provider registry,
            // playback engine, queue, etc.
            let state = AppState::new();
            app.manage(Arc::new(Mutex::new(state)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_albums,
            commands::get_artists,
            commands::get_tracks,
            commands::get_playback_state,
            commands::get_queue,
            commands::search,
            commands::play_track,
            commands::pause,
            commands::resume,
            commands::stop,
            commands::next,
            commands::previous,
            commands::seek,
            commands::set_volume,
            commands::set_muted,
            commands::jellyfin_discover,
            commands::jellyfin_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
