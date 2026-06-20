//! Queue state model.
//!
//! `QueueEngine` is the in-memory authoritative state for playback order.
//! Snapshot/restore methods let the UI hydrate from disk and the player
//! emit a serialisable view for the frontend.
//!
//! For v0.1 the engine stores entries linearly with a simple
//! repeat/shuffle policy. Full Rufin-style shuffle keys and origins are
//! tracked in the `origin` field; richer behaviour lands in later phases.

use serde::{Deserialize, Serialize};

use super::ids::{QueueEntryId, ServerId, TrackId};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub track_id: TrackId,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u32,
    pub origin: QueueEntryOrigin,
}

/// Why a track was added to the queue.
///
/// `shuffle_key` lets the engine reorder without losing the user's intent
/// when shuffle is toggled.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum QueueEntryOrigin {
    /// Added from a source view (album, playlist).
    Source { shuffle_key: u64 },
    /// Added by the user explicitly.
    Manual { shuffle_key: u64 },
    /// Restored from a snapshot whose origin we no longer know.
    RestoredUnknown { shuffle_key: u64 },
}

/// A batch replacement used by "Play album" / "Play playlist" actions.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QueueReplacement {
    pub track_ids: Vec<TrackId>,
    pub origin: QueueEntryOrigin,
}

/// Serializable view of the queue for the UI.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct QueueSnapshot {
    pub server_id: Option<ServerId>,
    pub entries: Vec<QueueEntry>,
    pub current_index: Option<usize>,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub shuffle_seed: u64,
}

/// In-memory queue state.
///
/// The engine is `Clone` because every Tauri command gets a snapshot via
/// `state.engine.clone()` — no `Mutex` needed in this skeleton; that
/// changes when playback mutates it concurrently.
#[derive(Clone, Debug, Default)]
pub struct QueueEngine {
    server_id: Option<ServerId>,
    entries: Vec<QueueEntry>,
    current_index: Option<usize>,
    repeat: RepeatMode,
    shuffle: bool,
    shuffle_seed: u64,
    next_entry_seq: u64,
}

impl QueueEngine {
    pub fn new(server_id: ServerId) -> Self {
        Self {
            server_id: Some(server_id),
            ..Self::default()
        }
    }

    pub fn from_snapshot(snapshot: QueueSnapshot) -> Self {
        let next_entry_seq = snapshot
            .entries
            .len()
            .try_into()
            .unwrap_or(u64::MAX);
        Self {
            server_id: snapshot.server_id,
            entries: snapshot.entries,
            current_index: snapshot.current_index,
            repeat: snapshot.repeat,
            shuffle: snapshot.shuffle,
            shuffle_seed: snapshot.shuffle_seed,
            next_entry_seq,
        }
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            server_id: self.server_id.clone(),
            entries: self.entries.clone(),
            current_index: self.current_index,
            repeat: self.repeat,
            shuffle: self.shuffle,
            shuffle_seed: self.shuffle_seed,
        }
    }

    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }

    pub fn current(&self) -> Option<&QueueEntry> {
        self.current_index.and_then(|i| self.entries.get(i))
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_engine_has_no_entries() {
        let engine = QueueEngine::new(ServerId::fake(1));
        assert!(engine.entries().is_empty());
        assert!(engine.current().is_none());
        assert_eq!(engine.repeat(), RepeatMode::Off);
        assert!(!engine.shuffle_enabled());
    }

    #[test]
    fn roundtrip_snapshot_preserves_state() {
        let mut engine = QueueEngine::new(ServerId::fake(1));
        engine.set_repeat(RepeatMode::All);
        engine.set_shuffle(true);
        let snapshot = engine.snapshot();
        let restored = QueueEngine::from_snapshot(snapshot.clone());
        assert_eq!(restored.repeat(), RepeatMode::All);
        assert!(restored.shuffle_enabled());
        assert_eq!(restored.snapshot().server_id, snapshot.server_id);
    }
}
