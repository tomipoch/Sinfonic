//! Queue state model.
//!
//! `QueueEngine` is the in-memory authoritative state for playback order.
//! Snapshot/restore methods let the UI hydrate from disk and the player
//! emit a serialisable view for the frontend.
//!
//! # Layout
//!
//! Every entry carries an immutable `entry_seq` (insertion order) and a
//! mutable `shuffle_key` (display order, lives inside `origin`). When
//! shuffle is **off**, entries are sorted by `entry_seq`. When shuffle
//! is **on**, they are sorted by `shuffle_key`, which is reassigned
//! deterministically from the engine's `shuffle_seed`.
//!
//! `current_index` always points into the *currently displayed* ordering,
//! so navigation ("next", "previous") works the same in both modes.
//!
//! # v0.1 Scope
//!
//! - `play_now`, `play_next`, `add_to_queue`, `remove`, `jump_to`,
//!   `move_entry`, `clear` — full CRUD on the queue
//! - `next_track` / `previous_track` — navigation with repeat logic
//! - `set_repeat` / `set_shuffle` — mode switches
//!
//! Deferred to later phases: per-track playback positions, "restart
//! current track" on `previous`, Auto DJ, Random origin, smart
//! playlists.

use serde::{Deserialize, Serialize};

use super::entities::Track;
use super::error::DomainError;
use super::ids::{QueueEntryId, ServerId, TrackId};

// ─── Repeat / Origin ────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

// ─── Entry / Origin ─────────────────────────────────────────────

/// Why a track was added to the queue.
///
/// `shuffle_key` is the *display order* key, owned by the engine. The
/// engine rewrites it when the user toggles shuffle; consumers should
/// treat it as opaque.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum QueueEntryOrigin {
    /// Added as part of a bulk action (play album, play playlist).
    Source { shuffle_key: u64 },
    /// Added explicitly by the user (play next, add to queue).
    Manual { shuffle_key: u64 },
    /// Loaded from a snapshot whose original origin we can no longer infer.
    RestoredUnknown { shuffle_key: u64 },
}

impl QueueEntryOrigin {
    pub fn shuffle_key(&self) -> u64 {
        match self {
            Self::Source { shuffle_key }
            | Self::Manual { shuffle_key }
            | Self::RestoredUnknown { shuffle_key } => *shuffle_key,
        }
    }

    pub fn set_shuffle_key(&mut self, new_key: u64) {
        match self {
            Self::Source { shuffle_key }
            | Self::Manual { shuffle_key }
            | Self::RestoredUnknown { shuffle_key } => *shuffle_key = new_key,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub track_id: TrackId,
    /// Insertion order. Monotonically increasing; set once at creation.
    /// Used as the sort key when shuffle is off.
    pub entry_seq: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u32,
    pub origin: QueueEntryOrigin,
}

impl QueueEntry {
    /// Builds an entry from a `Track`. The caller supplies the `id` and
    /// `entry_seq`; the `shuffle_key` starts equal to `entry_seq` so the
    /// unshuffled order matches insertion order.
    pub fn new(id: QueueEntryId, entry_seq: u64, track: &Track, origin: QueueEntryOrigin) -> Self {
        Self {
            id,
            track_id: track.id.clone(),
            entry_seq,
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration_seconds: track.duration_seconds,
            origin,
        }
    }
}

// ─── Snapshot / Replacement ─────────────────────────────────────

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

// ─── Engine ─────────────────────────────────────────────────────

/// In-memory queue state.
///
/// The engine is `Clone` so every Tauri command can take a snapshot of
/// the relevant view without holding the lock; mutations go through
/// `&mut self` under the `AppState` mutex.
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
        let max_entry_seq = snapshot
            .entries
            .iter()
            .map(|e| e.entry_seq)
            .max()
            .unwrap_or(0);
        let next_entry_seq = max_entry_seq.saturating_add(1);
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

    pub fn server_id(&self) -> Option<&ServerId> {
        self.server_id.as_ref()
    }

    pub fn set_server_id(&mut self, server_id: ServerId) {
        self.server_id = Some(server_id);
    }

    // ── Accessors ──────────────────────────────────────────────

    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn current(&self) -> Option<&QueueEntry> {
        self.current_index.and_then(|i| self.entries.get(i))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn find(&self, entry_id: &QueueEntryId) -> Option<&QueueEntry> {
        self.entries.iter().find(|e| &e.id == entry_id)
    }

    pub fn position_of(&self, entry_id: &QueueEntryId) -> Option<usize> {
        self.entries.iter().position(|e| &e.id == entry_id)
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle
    }

    pub fn shuffle_seed(&self) -> u64 {
        self.shuffle_seed
    }

    // ── Mutations: bulk ────────────────────────────────────────

    /// Replaces the entire queue with the given tracks and starts
    /// playback at the first one. Returns the `QueueEntryId`s of every
    /// entry created (in the order they end up in the queue, which may
    /// differ from `tracks` when shuffle is on). Empty input returns
    /// an empty vec.
    pub fn play_now(&mut self, tracks: &[Track]) -> Vec<QueueEntryId> {
        self.entries.clear();
        self.next_entry_seq = 0;
        self.current_index = None;

        for track in tracks {
            let id = self.alloc_entry_id();
            let entry_seq = self.next_entry_seq;
            self.next_entry_seq += 1;
            let origin = QueueEntryOrigin::Source { shuffle_key: entry_seq };
            self.entries.push(QueueEntry::new(id, entry_seq, track, origin));
        }

        if self.shuffle && !self.entries.is_empty() {
            self.assign_shuffle_keys();
            self.sort_by_shuffle_key();
        }

        if !self.entries.is_empty() {
            self.current_index = Some(0);
        }

        self.entries.iter().map(|e| e.id.clone()).collect()
    }

    /// Replaces the queue with a list of track IDs resolved by the
    /// caller. The resolver is invoked lazily; if it returns `None` the
    /// track is skipped. Mirrors `play_now` but for the case where the
    /// UI only has IDs (e.g., queue persistence).
    pub fn play_now_ids<F>(&mut self, track_ids: &[TrackId], resolve: F) -> Vec<QueueEntryId>
    where
        F: Fn(&TrackId) -> Option<Track>,
    {
        self.entries.clear();
        self.next_entry_seq = 0;
        self.current_index = None;

        let mut new_ids = Vec::with_capacity(track_ids.len());
        for tid in track_ids {
            let Some(track) = resolve(tid) else { continue };
            let id = self.alloc_entry_id();
            let entry_seq = self.next_entry_seq;
            self.next_entry_seq += 1;
            let origin = QueueEntryOrigin::Source { shuffle_key: entry_seq };
            new_ids.push(id.clone());
            self.entries.push(QueueEntry::new(id, entry_seq, &track, origin));
        }

        if self.shuffle {
            self.assign_shuffle_keys();
            self.sort_by_shuffle_key();
        }

        if !self.entries.is_empty() {
            self.current_index = Some(0);
        }

        new_ids
    }

    // ── Mutations: single ──────────────────────────────────────

    /// Inserts `track` immediately after the current entry. If no
    /// track is current, appends to the end.
    pub fn play_next(&mut self, track: &Track) -> QueueEntryId {
        let id = self.alloc_entry_id();
        let entry_seq = self.next_entry_seq;
        self.next_entry_seq += 1;
        let origin = QueueEntryOrigin::Manual { shuffle_key: entry_seq };
        let entry = QueueEntry::new(id.clone(), entry_seq, track, origin);

        let insert_at = match self.current_index {
            Some(i) => (i + 1).min(self.entries.len()),
            None => self.entries.len(),
        };
        self.entries.insert(insert_at, entry);
        self.shift_current_after_insert(insert_at);
        id
    }

    /// Appends `track` to the end of the queue.
    pub fn add_to_queue(&mut self, track: &Track) -> QueueEntryId {
        let id = self.alloc_entry_id();
        let entry_seq = self.next_entry_seq;
        self.next_entry_seq += 1;
        let origin = QueueEntryOrigin::Manual { shuffle_key: entry_seq };
        self.entries.push(QueueEntry::new(id.clone(), entry_seq, track, origin));
        id
    }

    /// Appends `tracks` to the end of the queue in order.
    /// Returns the `QueueEntryId`s of every entry created.
    pub fn add_many(&mut self, tracks: &[Track]) -> Vec<QueueEntryId> {
        let base_seq = self.next_entry_seq;
        let mut ids = Vec::with_capacity(tracks.len());
        for (i, track) in tracks.iter().enumerate() {
            let id = self.alloc_entry_id();
            let entry_seq = base_seq + i as u64;
            self.next_entry_seq = entry_seq + 1;
            let origin = QueueEntryOrigin::Manual { shuffle_key: entry_seq };
            self.entries.push(QueueEntry::new(id.clone(), entry_seq, track, origin));
            ids.push(id);
        }
        ids
    }

    /// Inserts `tracks` immediately after the current entry, preserving
    /// order. If no track is current, appends to the end.
    /// Returns the `QueueEntryId`s of every entry created.
    pub fn play_next_many(&mut self, tracks: &[Track]) -> Vec<QueueEntryId> {
        let insert_at = match self.current_index {
            Some(i) => (i + 1).min(self.entries.len()),
            None => self.entries.len(),
        };
        let base_seq = self.next_entry_seq;
        let mut ids = Vec::with_capacity(tracks.len());
        for (i, track) in tracks.iter().enumerate() {
            let id = self.alloc_entry_id();
            let entry_seq = base_seq + i as u64;
            self.next_entry_seq = entry_seq + 1;
            let origin = QueueEntryOrigin::Manual { shuffle_key: entry_seq };
            let entry = QueueEntry::new(id.clone(), entry_seq, track, origin);
            self.entries.insert(insert_at + i, entry);
            ids.push(id);
        }
        if insert_at <= self.current_index.unwrap_or(0) {
            if let Some(idx) = self.current_index {
                self.current_index = Some(idx + tracks.len());
            }
        }
        ids
    }

    /// Removes the entry with the given id. Returns `true` if found.
    /// Adjusts `current_index` to remain on the same entry when
    /// possible.
    pub fn remove_entry(&mut self, entry_id: &QueueEntryId) -> bool {
        let Some(pos) = self.position_of(entry_id) else {
            return false;
        };
        self.entries.remove(pos);
        match self.current_index {
            Some(i) if i == pos => {
                if self.entries.is_empty() {
                    self.current_index = None;
                } else if i >= self.entries.len() {
                    self.current_index = Some(self.entries.len() - 1);
                }
                // else: keep same index, which now points to the next entry
            }
            Some(i) if i > pos => {
                self.current_index = Some(i - 1);
            }
            _ => {}
        }
        true
    }

    /// Sets the entry with the given id as the current one. Returns
    /// `true` if found.
    pub fn jump_to(&mut self, entry_id: &QueueEntryId) -> bool {
        match self.position_of(entry_id) {
            Some(i) => {
                self.current_index = Some(i);
                true
            }
            None => false,
        }
    }

    /// Moves the entry with `entry_id` to `target_index` in the current
    /// ordering. The engine clamps `target_index` to the valid range
    /// and adjusts `current_index` so the same entry remains current.
    pub fn move_entry(
        &mut self,
        entry_id: &QueueEntryId,
        target_index: usize,
    ) -> Result<(), DomainError> {
        let from = self
            .position_of(entry_id)
            .ok_or_else(|| DomainError::Validation(format!("entry not found: {entry_id}")))?;
        if from == target_index {
            return Ok(());
        }
        let entry = self.entries.remove(from);
        let target = target_index.min(self.entries.len());
        self.entries.insert(target, entry);

        // Re-anchor current_index to the moved entry.
        self.current_index = Some(target);
        Ok(())
    }

    /// Empties the queue. Server id, repeat and shuffle settings are
    /// preserved so the user doesn't lose their mode after a clear.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
        self.next_entry_seq = 0;
    }

    // ── Mode switches ──────────────────────────────────────────

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Toggles shuffle. When enabling, the engine re-derives a
    /// deterministic `shuffle_key` for every entry from
    /// `shuffle_seed` and sorts by it. When disabling, the keys are
    /// reset to `entry_seq` (insertion order).
    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
        if enabled {
            // Refresh the seed so two consecutive toggles produce a
            // different order, while a single toggle is reproducible.
            self.shuffle_seed = self.shuffle_seed.wrapping_add(1).max(1);
        }
        self.recompute_ordering();
    }

    /// Replaces the shuffle seed and re-derives the ordering. Useful
    /// for "shuffle by" actions that re-roll the seed on demand.
    pub fn reshuffle(&mut self, seed: u64) {
        self.shuffle_seed = seed.max(1);
        self.shuffle = true;
        self.recompute_ordering();
    }

    // ── Navigation ─────────────────────────────────────────────

    /// Advances to the next track according to the current repeat
    /// mode. Returns the new current entry, or `None` if the queue
    /// ended (no repeat) or is empty.
    pub fn next_track(&mut self) -> Option<&QueueEntry> {
        if self.entries.is_empty() {
            self.current_index = None;
            return None;
        }
        match self.repeat {
            RepeatMode::One => {} // stay
            RepeatMode::All => {
                let next = match self.current_index {
                    Some(i) => (i + 1) % self.entries.len(),
                    None => 0,
                };
                self.current_index = Some(next);
            }
            RepeatMode::Off => match self.current_index {
                Some(i) if i + 1 < self.entries.len() => {
                    self.current_index = Some(i + 1);
                }
                Some(_) => {
                    self.current_index = None;
                    return None;
                }
                None => {
                    self.current_index = Some(0);
                }
            },
        }
        self.current()
    }

    /// Goes back one entry. With repeat `One` or `All`, wraps to the
    /// last entry. With repeat `Off` and at index 0, returns the
    /// current entry unchanged. The "restart current track" behavior
    /// lives in the player; the queue just handles index movement.
    pub fn previous_track(&mut self) -> Option<&QueueEntry> {
        if self.entries.is_empty() {
            self.current_index = None;
            return None;
        }
        match self.current_index {
            Some(0) => {
                // Already at the start; leave it.
            }
            Some(i) => {
                self.current_index = Some(i - 1);
            }
            None => {
                self.current_index = Some(self.entries.len() - 1);
            }
        }
        self.current()
    }

    /// Re-anchors the current pointer to the first entry (or clears
    /// it if the queue is empty). Useful after a snapshot restore when
    /// the previous `current_index` is no longer valid.
    pub fn reset_to_first(&mut self) {
        self.current_index = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    // ── Internals ──────────────────────────────────────────────

    fn alloc_entry_id(&self) -> QueueEntryId {
        QueueEntryId::new(format!("queue-{}", self.next_entry_seq))
    }

    fn shift_current_after_insert(&mut self, inserted_at: usize) {
        match self.current_index {
            Some(i) if i >= inserted_at => {
                self.current_index = Some(i + 1);
            }
            _ => {}
        }
    }

    fn assign_shuffle_keys(&mut self) {
        for entry in &mut self.entries {
            let key = splitmix64_mix(self.shuffle_seed, entry.entry_seq);
            entry.origin.set_shuffle_key(key);
        }
    }

    fn reset_shuffle_keys_to_entry_seq(&mut self) {
        for entry in &mut self.entries {
            entry.origin.set_shuffle_key(entry.entry_seq);
        }
    }

    fn sort_by_shuffle_key(&mut self) {
        self.entries.sort_by_key(|e| e.origin.shuffle_key());
    }

    fn sort_by_entry_seq(&mut self) {
        self.entries.sort_by_key(|e| e.entry_seq);
    }

    fn recompute_ordering(&mut self) {
        let current_id = self.current().map(|e| e.id.clone());
        if self.shuffle {
            self.assign_shuffle_keys();
            self.sort_by_shuffle_key();
        } else {
            self.reset_shuffle_keys_to_entry_seq();
            self.sort_by_entry_seq();
        }
        self.current_index = current_id
            .as_ref()
            .and_then(|id| self.position_of(id))
            .or({
                if self.entries.is_empty() {
                    None
                } else {
                    Some(0)
                }
            });
    }
}

/// Tiny deterministic PRNG mix (splitmix64). Not cryptographic; just
/// well-distributed enough to make a queue look shuffled while
/// remaining reproducible from `(seed, entry_seq)`.
fn splitmix64_mix(seed: u64, seq: u64) -> u64 {
    let mut z = seed.wrapping_add(seq.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AlbumId, ArtistId};

    fn track(id: &str, title: &str, dur: u32) -> Track {
        Track {
            id: TrackId::new(id),
            album_id: AlbumId::fake(1),
            title: title.into(),
            artist: "Artist".into(),
            artist_id: Some(ArtistId::fake(1)),
            album: "Album".into(),
            duration_seconds: dur,
            track_number: 1,
            disc_number: 1,
            favorite: false,
            image_ref: None,
        }
    }

    fn engine() -> QueueEngine {
        QueueEngine::new(ServerId::fake(1))
    }

    fn filled_engine(n: usize) -> QueueEngine {
        let mut e = engine();
        let tracks: Vec<Track> = (0..n).map(|i| track(&format!("t-{i}"), &format!("T{i}"), 180)).collect();
        e.play_now(&tracks);
        e
    }

    // ── Construction ───────────────────────────────────────────

    #[test]
    fn new_engine_is_empty() {
        let e = engine();
        assert!(e.is_empty());
        assert!(e.current().is_none());
        assert_eq!(e.repeat(), RepeatMode::Off);
        assert!(!e.shuffle_enabled());
    }

    #[test]
    fn snapshot_round_trip_preserves_entries_and_current() {
        let e = filled_engine(3);
        let snap = e.snapshot();
        let restored = QueueEngine::from_snapshot(snap.clone());
        assert_eq!(restored.entries().len(), 3);
        assert_eq!(restored.current_index(), snap.current_index);
        assert_eq!(restored.entries()[0].title, "T0");
    }

    #[test]
    fn from_snapshot_advances_seq_past_max() {
        let mut e = engine();
        e.play_now(&[track("a", "A", 1)]);
        e.play_next(&track("b", "B", 1));
        let snap = e.snapshot();
        let mut restored = QueueEngine::from_snapshot(snap);
        let id = restored.add_to_queue(&track("c", "C", 1));
        assert_eq!(id.as_str(), "queue-2");
    }

    // ── play_now ───────────────────────────────────────────────

    #[test]
    fn play_now_clears_and_sets_current_to_first() {
        let mut e = filled_engine(2);
        e.add_to_queue(&track("z", "Z", 1));
        assert_eq!(e.len(), 3);
        let ids = e.play_now(&[track("a", "A", 1), track("b", "B", 1)]);
        assert_eq!(e.len(), 2);
        assert_eq!(e.current_index(), Some(0));
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_str(), "queue-0");
    }

    #[test]
    fn play_now_with_empty_input_clears_and_returns_empty() {
        let mut e = filled_engine(2);
        let ids = e.play_now(&[]);
        assert!(ids.is_empty());
        assert!(e.is_empty());
        assert!(e.current().is_none());
    }

    // ── play_next / add_to_queue ───────────────────────────────

    #[test]
    fn add_to_queue_appends() {
        let mut e = filled_engine(2);
        let id = e.add_to_queue(&track("x", "X", 1));
        assert_eq!(e.len(), 3);
        assert_eq!(e.entries().last().unwrap().id, id);
    }

    #[test]
    fn play_next_inserts_after_current() {
        let mut e = filled_engine(3);
        let first = e.entries()[0].id.clone();
        e.jump_to(&first);
        e.play_next(&track("x", "X", 1));
        assert_eq!(e.entries()[1].title, "X");
        assert_eq!(e.current_index(), Some(0));
    }

    #[test]
    fn play_next_shifts_current_when_inserted_before() {
        let mut e = filled_engine(3);
        let second = e.entries()[1].id.clone();
        e.jump_to(&second);
        e.play_next(&track("x", "X", 1));
        assert_eq!(e.entries()[2].title, "X");
        assert_eq!(e.current_index(), Some(1));
    }

    #[test]
    fn play_next_appends_when_no_current() {
        let mut e = engine();
        let id = e.play_next(&track("x", "X", 1));
        assert_eq!(e.len(), 1);
        assert_eq!(e.entries()[0].id, id);
    }

    // ── remove / jump_to ───────────────────────────────────────

    #[test]
    fn remove_entry_returns_false_when_missing() {
        let mut e = filled_engine(2);
        assert!(!e.remove_entry(&QueueEntryId::fake(99)));
    }

    #[test]
    fn remove_entry_before_current_shifts_index() {
        let mut e = filled_engine(3);
        let removed = e.entries()[0].id.clone();
        let current = e.entries()[2].id.clone();
        e.jump_to(&current);
        e.remove_entry(&removed);
        assert_eq!(e.current_index(), Some(1));
    }

    #[test]
    fn remove_current_at_end_drops_to_new_last() {
        let mut e = filled_engine(2);
        let current = e.entries()[1].id.clone();
        e.jump_to(&current);
        e.remove_entry(&current);
        assert_eq!(e.current_index(), Some(0));
    }

    #[test]
    fn remove_only_entry_clears_current() {
        let mut e = filled_engine(1);
        let only = e.entries()[0].id.clone();
        e.remove_entry(&only);
        assert!(e.is_empty());
        assert!(e.current().is_none());
    }

    #[test]
    fn jump_to_sets_index() {
        let mut e = filled_engine(3);
        let target = e.entries()[2].id.clone();
        e.jump_to(&target);
        assert_eq!(e.current_index(), Some(2));
    }

    // ── move_entry ─────────────────────────────────────────────

    #[test]
    fn move_entry_reorders() {
        let mut e = filled_engine(3);
        let moved = e.entries()[0].id.clone();
        e.move_entry(&moved, 2).unwrap();
        assert_eq!(e.entries()[2].id, moved);
        assert_eq!(e.current_index(), Some(2));
    }

    #[test]
    fn move_entry_clamps_target() {
        let mut e = filled_engine(2);
        let moved = e.entries()[0].id.clone();
        e.move_entry(&moved, 99).unwrap();
        assert_eq!(e.entries()[1].id, moved);
    }

    #[test]
    fn move_entry_missing_returns_validation_error() {
        let mut e = filled_engine(2);
        let err = e.move_entry(&QueueEntryId::fake(99), 0).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    // ── Navigation ─────────────────────────────────────────────

    #[test]
    fn next_advances_then_stops_at_end_without_repeat() {
        let mut e = filled_engine(3);
        assert_eq!(e.next_track().unwrap().title, "T1");
        assert_eq!(e.next_track().unwrap().title, "T2");
        assert!(e.next_track().is_none());
        assert!(e.current().is_none());
    }

    #[test]
    fn next_with_repeat_all_wraps() {
        let mut e = filled_engine(2);
        e.set_repeat(RepeatMode::All);
        assert_eq!(e.next_track().unwrap().title, "T1");
        assert_eq!(e.next_track().unwrap().title, "T0");
        assert_eq!(e.next_track().unwrap().title, "T1");
    }

    #[test]
    fn next_with_repeat_one_stays() {
        let mut e = filled_engine(3);
        e.set_repeat(RepeatMode::One);
        assert_eq!(e.next_track().unwrap().title, "T0");
        assert_eq!(e.next_track().unwrap().title, "T0");
        assert_eq!(e.next_track().unwrap().title, "T0");
    }

    #[test]
    fn previous_at_zero_stays() {
        let mut e = filled_engine(3);
        let first = e.entries()[0].id.clone();
        e.jump_to(&first);
        e.previous_track();
        assert_eq!(e.current_index(), Some(0));
    }

    #[test]
    fn previous_steps_back() {
        let mut e = filled_engine(3);
        let third = e.entries()[2].id.clone();
        e.jump_to(&third);
        e.previous_track();
        assert_eq!(e.current_index(), Some(1));
    }

    #[test]
    fn previous_after_exhausting_with_no_repeat_wraps_to_last() {
        // Exhaust the queue with repeat Off, then press previous: it
        // should land on the last entry (defensive path for current == None).
        let mut e = filled_engine(3);
        e.next_track();
        e.next_track();
        e.next_track(); // returns None, clears current
        assert!(e.current().is_none());
        e.previous_track();
        assert_eq!(e.current_index(), Some(2));
    }

    // ── Shuffle ────────────────────────────────────────────────

    #[test]
    fn set_shuffle_reorders_entries() {
        let mut e = filled_engine(10);
        let before: Vec<u64> = e.entries().iter().map(|x| x.entry_seq).collect();
        e.set_shuffle(true);
        assert!(e.shuffle_enabled());
        let after: Vec<u64> = e.entries().iter().map(|x| x.entry_seq).collect();
        // Statistically, with 10 entries the chance of a fixed point is negligible.
        assert_ne!(before, after, "shuffle should reorder");
    }

    #[test]
    fn unset_shuffle_restores_insertion_order() {
        let mut e = filled_engine(10);
        let before: Vec<u64> = e.entries().iter().map(|x| x.entry_seq).collect();
        e.set_shuffle(true);
        e.set_shuffle(false);
        let after: Vec<u64> = e.entries().iter().map(|x| x.entry_seq).collect();
        assert_eq!(before, after, "unshuffle should restore insertion order");
    }

    #[test]
    fn shuffle_preserves_current_entry() {
        let mut e = filled_engine(10);
        let target = e.entries()[3].id.clone();
        e.jump_to(&target);
        e.set_shuffle(true);
        assert_eq!(e.current().unwrap().id, target);
    }

    #[test]
    fn shuffle_is_deterministic_for_same_seed() {
        let mut a = filled_engine(20);
        a.set_shuffle(true);
        let order_a: Vec<String> = a.entries().iter().map(|x| x.id.as_str().to_string()).collect();

        let mut b = filled_engine(20);
        b.set_shuffle(true);
        let order_b: Vec<String> = b.entries().iter().map(|x| x.id.as_str().to_string()).collect();

        // The engine seeds itself with `next_entry_seq.max(1)` on first
        // toggle, so identical inputs give identical outputs.
        assert_eq!(order_a, order_b);
    }

    #[test]
    fn reshuffle_changes_seed_and_order() {
        let mut e = filled_engine(20);
        e.set_shuffle(true);
        let first_order: Vec<String> = e.entries().iter().map(|x| x.id.as_str().to_string()).collect();
        e.reshuffle(0xDEAD_BEEF);
        let second_order: Vec<String> = e.entries().iter().map(|x| x.id.as_str().to_string()).collect();
        assert_ne!(first_order, second_order);
    }

    // ── add_many / play_next_many ─────────────────────────────

    #[test]
    fn add_many_appends_all_in_order() {
        let mut e = filled_engine(2);
        let ids = e.add_many(&[track("a", "A", 1), track("b", "B", 1)]);
        assert_eq!(ids.len(), 2);
        assert_eq!(e.len(), 4);
        assert_eq!(e.entries()[2].title, "A");
        assert_eq!(e.entries()[3].title, "B");
        assert_eq!(ids[0].as_str(), "queue-2");
        assert_eq!(ids[1].as_str(), "queue-3");
    }

    #[test]
    fn add_many_with_empty_input_returns_empty() {
        let mut e = filled_engine(2);
        let ids = e.add_many(&[]);
        assert!(ids.is_empty());
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn add_many_preserves_current_index() {
        let mut e = filled_engine(3);
        let target = e.entries()[1].id.clone();
        e.jump_to(&target);
        assert_eq!(e.current_index(), Some(1));
        e.add_many(&[track("x", "X", 1), track("y", "Y", 1)]);
        assert_eq!(e.current_index(), Some(1));
    }

    #[test]
    fn play_next_many_inserts_after_current() {
        let mut e = filled_engine(3);
        let target = e.entries()[1].id.clone();
        e.jump_to(&target);
        let ids = e.play_next_many(&[track("x", "X", 1), track("y", "Y", 1)]);
        assert_eq!(ids.len(), 2);
        assert_eq!(e.entries()[2].title, "X");
        assert_eq!(e.entries()[3].title, "Y");
        assert_eq!(ids[0].as_str(), "queue-3");
        assert_eq!(ids[1].as_str(), "queue-4");
        assert_eq!(e.current_index(), Some(1));
    }

    #[test]
    fn play_next_many_appends_when_no_current() {
        let mut e = engine();
        let ids = e.play_next_many(&[track("a", "A", 1), track("b", "B", 1)]);
        assert_eq!(ids.len(), 2);
        assert_eq!(e.len(), 2);
        assert_eq!(e.entries()[0].title, "A");
        assert_eq!(e.entries()[1].title, "B");
    }

    #[test]
    fn play_next_many_does_not_shift_current_when_inserting_after() {
        let mut e = filled_engine(3);
        let target = e.entries()[0].id.clone();
        e.jump_to(&target);
        assert_eq!(e.current_index(), Some(0));
        e.play_next_many(&[track("x", "X", 1), track("y", "Y", 1)]);
        assert_eq!(e.current_index(), Some(0));
    }

    #[test]
    fn splitmix_produces_distinct_keys() {
        for s in 1..50u64 {
            let keys: std::collections::HashSet<u64> =
                (0..100).map(|i| splitmix64_mix(s, i)).collect();
            assert_eq!(keys.len(), 100, "seed {s} produced duplicate keys");
        }
    }
}
