//! Tauri event names and their payload types.
//!
//! Centralising event names avoids the silent-typo failure mode where the
//! frontend listens for `"playback_state_changed"` and the backend
//! emits `"playback-state-changed"`. One enum, one `as_str()`.

use serde::{Deserialize, Serialize};
use sinfonic_domain::RepeatMode;

/// Names of every event the backend can emit.
///
/// The frontend subscribes to these via `listen<T>(name, handler)`; the
/// backend emits them via `app.emit(name.as_str(), payload)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventName {
    PlaybackStateChanged,
    QueueChanged,
    TrackChanged,
    LibrarySyncStatus,
    ServerDiscovered,
}

impl EventName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlaybackStateChanged => "playback-state-changed",
            Self::QueueChanged => "queue-changed",
            Self::TrackChanged => "track-changed",
            Self::LibrarySyncStatus => "library-sync-status",
            Self::ServerDiscovered => "server-discovered",
        }
    }
}

/// Payload for `playback-state-changed`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatePayload {
    pub is_playing: bool,
    pub position_seconds: u32,
    pub duration_seconds: u32,
    pub volume: f32,
    pub muted: bool,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

impl PlaybackStatePayload {
    pub fn playing() -> Self {
        Self {
            is_playing: true,
            ..Self::default()
        }
    }
    pub fn paused() -> Self {
        Self {
            is_playing: false,
            ..Self::default()
        }
    }
}

/// Payload for `queue-changed`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshotPayload {
    pub entries: Vec<QueueEntryView>,
    pub current_index: Option<usize>,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntryView {
    pub id: String,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u32,
}

/// Payload for `track-changed`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackChangedPayload {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
}

/// Payload for `library-sync-status`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySyncStatusPayload {
    pub server_id: Option<String>,
    pub state: String,
    pub progress: f32,
}
