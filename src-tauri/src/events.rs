//! Re-exports of the Tauri event vocabulary.
//!
//! The single source of truth for event names and payloads is now
//! `sinfonic_domain::events` so that library crates (e.g.
//! `sinfonic-source-subsonic`) can emit the same wire strings the
//! frontend listens for, without depending on the app crate.

pub use sinfonic_domain::events::{
    EventName, LibrarySyncStatusPayload, PlaybackConfigPayload, PlaybackStatePayload,
    QueueEntryView, QueueSnapshotPayload, SyncProgressPayload, TrackChangedPayload,
};