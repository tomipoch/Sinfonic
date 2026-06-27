//! Tauri event names and their payload types.
//!
//! Centralising event names avoids the silent-typo failure mode where the
//! frontend listens for `"playback_state_changed"` and the backend
//! emits `"playback-state-changed"`. One enum, one `as_str()`.

use serde::{Deserialize, Serialize};
use sinfonic_domain::{PlaybackState, QueueEngine, RepeatMode};

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
    /// Per-batch progress emitted by long-running sync phases. The
    /// frontend may surface this as a progress bar; missing listeners
    /// are harmless (the emit is fire-and-forget).
    SyncProgress,
}

impl EventName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlaybackStateChanged => "playback-state-changed",
            Self::QueueChanged => "queue-changed",
            Self::TrackChanged => "track-changed",
            Self::LibrarySyncStatus => "library-sync-status",
            Self::SyncProgress => "sync-progress",
        }
    }
}

/// Payload for `playback-state-changed`.
///
/// Merges the runtime `PlaybackState` with the queue's repeat/shuffle
/// mode so the UI gets a single snapshot without juggling two event
/// sources.
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

    /// Builds the payload from the live state + queue. Use this in
    /// commands so the wire format has one constructor.
    pub fn from_state(playback: &PlaybackState, queue: &QueueEngine) -> Self {
        Self {
            is_playing: playback.is_playing,
            position_seconds: playback.position_seconds,
            duration_seconds: playback.duration_seconds,
            volume: playback.volume,
            muted: playback.muted,
            repeat: queue.repeat(),
            shuffle: queue.shuffle_enabled(),
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

/// Payload for `sync-progress`.
///
/// `phase` is a free-form label (e.g. `"tracks"`, `"albums"`) so the
/// frontend can distinguish phases without hard-coding a number per
/// provider. `done` / `total` are counts of completed work units
/// within the current phase.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgressPayload {
    pub phase: String,
    pub done: usize,
    pub total: usize,
}
