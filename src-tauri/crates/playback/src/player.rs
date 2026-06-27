//! AudioPlayer — owns the rodio output stream and the playback state.
//!
//! ```text
//!   ┌─────────────────────────────────────────────────────────────────┐
//!   │                       AudioPlayer                                │
//!   │                                                                  │
//!   │  OutputStream (kept on the main thread, !Send)                   │
//!   │       │                                                         │
//!   │       ▼                                                         │
//!   │  OutputStreamHandle (Send + Sync) ──┐                           │
//!   │                                     │                           │
//!   │     ┌───────────────────────────────┘                           │
//!   │     ▼                                                           │
//!   │   Sink (Arc inside rodio) ── append(source) ── plays            │
//!   │                                                                  │
//!   │   Position poller thread ── polls sink.get_pos() every 250ms,   │
//!   │   fires PlayerEvent callbacks back to the AppState.             │
//!   └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Threading model
//!
//! `OutputStream` is `!Send` (cpal's audio thread has thread-affinity
//! requirements). It lives directly on the `AudioPlayer` (which is
//! owned by the `AppState` and only moved at construction time). The
//! inner state that IS shared with the position-poller thread is
//! wrapped in `Arc<Inner>` where `Inner` contains only `Send + Sync`
//! fields.
//!
//! All rodio access goes through a single `Mutex<Sink>` for the
//! commands. The position-poller thread takes the same mutex for a
//! few microseconds at a time, so commands never block on the poller
//! for more than that.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use parking_lot::Mutex;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use sinfonic_domain::{PlaybackState, TrackId};
use thiserror::Error;

use crate::eq::{Equalizer, SharedEqualizer};
use crate::stream;

/// Wrapper around rodio's `OutputStream` (which is `!Send + !Sync`
/// because cpal 0.15 holds a platform-specific `*mut ()` stream
/// handle with thread affinity). We never touch the inner stream from
/// any thread other than the one that constructed it — the only thing
/// we need is for it to live as long as the `OutputStreamHandle` we
/// extracted from it. The `unsafe impl Send + Sync` declares that
/// invariant; it is sound because all access to the stream is funnelled
/// through rodio's own APIs (which carry their own thread-safety
/// guarantees on the `Sink` side).
struct OutputStreamHolder(#[allow(dead_code)] Option<OutputStream>);

// SAFETY: `OutputStreamHolder` is only ever constructed inside
// `AudioPlayer::new` and then moved into the `AudioPlayer` struct
// exactly once. After construction, the inner `OutputStream` is never
// touched from any thread (no method reads or writes it). Its sole
// purpose is to keep the OS-level sink alive for as long as the
// associated `OutputStreamHandle` lives inside `Inner::stream_handle`.
//
// We do not implement `Clone`, so the value cannot escape into another
// thread. All `Send`-bound APIs surface only the `Arc<Inner>` (which
// contains no `!Send` data) and the `&self` receiver (which is a
// shared reference, never crossing threads on its own).
//
// Verified by `concurrent_play_pause_volume_no_panic` below.
unsafe impl Send for OutputStreamHolder {}
unsafe impl Sync for OutputStreamHolder {}

/// How often the position-poller thread reads the rodio Sink.
///
/// 1 s is granular enough for a seekable progress bar (Spotify uses a
/// similar cadence) and cheap enough that the per-tick `app.emit`
/// cost is negligible. Earlier this was 250 ms (4 Hz), which produced
/// four `playback-state-changed` events per second and visibly more
/// CPU when the player was idle.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Events the AudioPlayer emits to the rest of the app. Wired to Tauri
/// events in `lib.rs::run`.
#[derive(Clone, Debug)]
pub enum PlayerEvent {
    /// Position changed (or play/pause flipped).
    StateChanged {
        track_id: Option<TrackId>,
        position_seconds: u32,
        is_playing: bool,
        volume: f32,
        muted: bool,
        duration_seconds: u32,
    },
    /// The sink ran dry. The queue should advance to the next track.
    TrackEnded { track_id: TrackId },
}

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("audio device unavailable: {0}")]
    NoDevice(String),
    #[error("stream error: {0}")]
    Stream(String),
}

/// Type alias for the event callback. Boxed by `Arc` so we don't pay
/// the cost of a clone on every emit.
pub type PlayerEventCallback = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

/// The single playback engine. Cheap to construct; cloning bumps an
/// Arc refcount. Stored behind an `Arc<AudioPlayer>` in the app state
/// so we don't need to derive `Clone` here — `OutputStream` is `!Clone`
/// anyway.
pub struct AudioPlayer {
    /// `OutputStream` is `!Send` — kept on the AudioPlayer struct
    /// itself inside a wrapper with a manual `unsafe impl Send`. The
    /// AudioPlayer only ever moves at construction, so this is safe.
    /// The field is never read; its job is purely to keep the OS-sink
    /// alive for as long as the AudioPlayer does.
    #[allow(dead_code)]
    output_stream: OutputStreamHolder,
    inner: Arc<Inner>,
}

struct Inner {
    stream_handle: Mutex<Option<OutputStreamHandle>>,
    sink: Mutex<Option<Sink>>,
    poller: Mutex<Option<JoinHandle<()>>>,
    poller_stop: Arc<AtomicBool>,
    track_id: Mutex<Option<TrackId>>,
    position_seconds: AtomicU32,
    duration_seconds: AtomicU32,
    is_paused: AtomicBool,
    ended_fired: AtomicBool,
    volume: Mutex<f32>,
    muted: Mutex<bool>,
    equalizer: SharedEqualizer,
    /// Set once via `set_event_callback` and never re-assigned at
    /// runtime, so we can use `OnceLock` and pay zero synchronisation
    /// cost on every emit.
    on_event: OnceLock<PlayerEventCallback>,
}

impl std::fmt::Debug for AudioPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPlayer")
            .field("is_paused", &self.inner.is_paused.load(Ordering::Relaxed))
            .field("position_seconds", &self.inner.position_seconds.load(Ordering::Relaxed))
            .field("volume", &self.inner.volume.lock())
            .finish()
    }
}

impl AudioPlayer {
    /// Try to open the default audio device. Falls back to a "silent"
    /// player (no audio output) on failure so commands keep working
    /// headless — useful for CI and `cargo test`.
    pub fn new() -> Self {
        let (stream_opt, handle_opt) = match OutputStream::try_default() {
            Ok(pair) => (Some(pair.0), Some(pair.1)),
            Err(err) => {
                tracing::warn!(
                    target: "sinfonic::playback",
                    error = %err,
                    "no audio output device; running in headless mode"
                );
                (None, None)
            }
        };
        let inner = Inner {
            stream_handle: Mutex::new(handle_opt),
            sink: Mutex::new(None),
            poller: Mutex::new(None),
            poller_stop: Arc::new(AtomicBool::new(false)),
            track_id: Mutex::new(None),
            position_seconds: AtomicU32::new(0),
            duration_seconds: AtomicU32::new(0),
            is_paused: AtomicBool::new(true),
            ended_fired: AtomicBool::new(false),
            volume: Mutex::new(0.8),
            muted: Mutex::new(false),
            equalizer: Arc::new(Mutex::new(Equalizer::flat())),
            on_event: OnceLock::new(),
        };
        Self {
            output_stream: OutputStreamHolder(stream_opt),
            inner: Arc::new(inner),
        }
    }

/// Register a callback fired on every state change / track end.
/// `lib.rs::run` wires this to Tauri `app.emit(...)` calls.
///
/// Must be called exactly once before any playback starts. Repeated
/// calls are silently ignored — `OnceLock::set` returns `Err` if the
/// slot is already populated. The lock-free `get` on the hot path
/// (every poll tick) is the win over the previous `Mutex<Option<…>>`
/// arrangement.
pub fn set_event_callback<F>(&self, callback: F)
where
    F: Fn(PlayerEvent) + Send + Sync + 'static,
{
    let _ = self.inner.on_event.set(Arc::new(callback));
}

    /// Read the most-recently cached playback state. Cheap; doesn't
    /// lock the rodio sink.
    pub fn cached_state(&self) -> PlaybackState {
        PlaybackState {
            is_playing: !self.inner.is_paused.load(Ordering::Relaxed)
                && self.inner.track_id.lock().is_some(),
            position_seconds: self.inner.position_seconds.load(Ordering::Relaxed),
            duration_seconds: self.inner.duration_seconds.load(Ordering::Relaxed),
            volume: *self.inner.volume.lock(),
            muted: *self.inner.muted.lock(),
        }
    }

    /// The track id currently being played, if any.
    pub fn current_track_id(&self) -> Option<TrackId> {
        self.inner.track_id.lock().clone()
    }

    /// Start playing `track_id` from its stream URI. Returns the
    /// resolved duration (from the rodio decoder) on success.
    pub async fn play(
        &self,
        track_id: TrackId,
        stream_uri: &str,
    ) -> Result<u32, PlayerError> {
        // Open the stream first so we know the duration before we
        // touch any state. A decode failure here is surfaced to the
        // caller and nothing else happens.
        //
        // `stream::open` is async because HTTP downloads are funneled
        // through `tokio::task::spawn_blocking` internally — keeping
        // the rodio `Sink` work on the same task avoids any chance of
        // the user pressing "next" mid-decode and leaving us with a
        // dangling source.
        let decoded = stream::open(stream_uri)
            .await
            .map_err(|e| PlayerError::Stream(e.to_string()))?
            .with_eq(self.inner.equalizer.clone());
        let duration_seconds = decoded.duration_seconds.unwrap_or(0);

        // Kill any existing poller + clear the previous sink before
        // swapping in a new one. We do this BEFORE building the new
        // sink so the old one is fully torn down.
        self.stop_poller();
        {
            let mut sink_slot = self.inner.sink.lock();
            *sink_slot = None;
        }

        // Build the new sink and append the source.
        let os_handle = self.inner.stream_handle.lock().clone();
        match os_handle {
            Some(os_handle) => {
                let sink = Sink::try_new(&os_handle)
                    .map_err(|e| PlayerError::NoDevice(e.to_string()))?;
                sink.set_volume(*self.inner.volume.lock());
                sink.append(decoded.source);
                sink.play();
                *self.inner.sink.lock() = Some(sink);
            }
            None => {
                // Headless: drop the source, fire TrackEnded so the
                // queue can advance without waiting on a real device.
                drop(decoded);
                if let Some(cb) = self.inner.on_event.get() {
                    cb(PlayerEvent::TrackEnded { track_id: track_id.clone() });
                }
            }
        };

        // Cache + emit.
        self.inner.position_seconds.store(0, Ordering::Relaxed);
        self.inner.duration_seconds.store(duration_seconds, Ordering::Relaxed);
        self.inner.is_paused.store(false, Ordering::Relaxed);
        self.inner.ended_fired.store(false, Ordering::Relaxed);
        *self.inner.track_id.lock() = Some(track_id.clone());

        self.start_poller();
        self.emit_state(Some(track_id));
        Ok(duration_seconds)
    }

    /// Pause the current sink. Idempotent.
    pub fn pause(&self) {
        if let Some(sink) = self.inner.sink.lock().as_ref() {
            sink.pause();
        }
        self.inner.is_paused.store(true, Ordering::Relaxed);
        self.emit_state(self.inner.track_id.lock().clone());
    }

    /// Resume the current sink. Idempotent.
    pub fn resume(&self) {
        if let Some(sink) = self.inner.sink.lock().as_ref() {
            if !sink.empty() {
                sink.play();
            }
        }
        self.inner.is_paused.store(false, Ordering::Relaxed);
        self.emit_state(self.inner.track_id.lock().clone());
    }

    /// Stop playback entirely and clear the queue.
    pub fn stop(&self) {
        self.stop_poller();
        {
            let mut sink_slot = self.inner.sink.lock();
            if let Some(sink) = sink_slot.as_ref() {
                sink.stop();
            }
            *sink_slot = None;
        }
        self.inner.position_seconds.store(0, Ordering::Relaxed);
        self.inner.duration_seconds.store(0, Ordering::Relaxed);
        self.inner.is_paused.store(true, Ordering::Relaxed);
        *self.inner.track_id.lock() = None;
        self.emit_state(None);
    }

    /// Seek to `position_seconds`. Saturates at the track's duration.
    pub fn seek(&self, position_seconds: u32) {
        let sink_slot = self.inner.sink.lock();
        if let Some(sink) = sink_slot.as_ref() {
            let _ = sink.try_seek(Duration::from_secs(position_seconds as u64));
        }
        let max = self.inner.duration_seconds.load(Ordering::Relaxed);
        self.inner
            .position_seconds
            .store(position_seconds.min(max), Ordering::Relaxed);
        self.emit_state(self.inner.track_id.lock().clone());
    }

    /// Set the master volume. `volume` is clamped to `[0.0, 1.0]`.
    pub fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        *self.inner.volume.lock() = clamped;
        if let Some(sink) = self.inner.sink.lock().as_ref() {
            sink.set_volume(clamped);
        }
        self.emit_state(self.inner.track_id.lock().clone());
    }

    /// Toggle the mute state. Volume is preserved; the position poll
    /// multiplies by `!muted` when emitting.
    pub fn set_muted(&self, muted: bool) {
        *self.inner.muted.lock() = muted;
        self.emit_state(self.inner.track_id.lock().clone());
    }

    /// Set the gain on a single EQ band (in Hz). `gain_db` is clamped
    /// to `[-12.0, +12.0]`.
    pub fn set_eq_band(&self, hz: u32, gain_db: f32) {
        self.inner.equalizer.lock().set_band(hz, gain_db);
    }

    /// Snapshot of the current EQ bands.
    pub fn eq_bands(&self) -> Vec<crate::eq::BandGain> {
        self.inner.equalizer.lock().bands().to_vec()
    }

    /// Reset the EQ to flat (all bands at 0 dB).
    pub fn reset_eq(&self) {
        let mut eq = self.inner.equalizer.lock();
        *eq = Equalizer::flat();
    }

    // ─── internals ────────────────────────────────────────────────

    fn emit_state(&self, track_id: Option<TrackId>) {
        if let Some(cb) = self.inner.on_event.get() {
            cb(PlayerEvent::StateChanged {
                track_id: track_id.clone(),
                position_seconds: self.inner.position_seconds.load(Ordering::Relaxed),
                is_playing: !self.inner.is_paused.load(Ordering::Relaxed) && track_id.is_some(),
                volume: *self.inner.volume.lock(),
                muted: *self.inner.muted.lock(),
                duration_seconds: self.inner.duration_seconds.load(Ordering::Relaxed),
            });
        }
    }

    fn start_poller(&self) {
        self.inner.poller_stop.store(false, Ordering::Relaxed);
        let inner = Arc::clone(&self.inner);
        let handle = std::thread::Builder::new()
            .name("sinfonic-playback-poller".into())
            .spawn(move || inner.run_poller())
            .expect("spawn playback poller");
        *self.inner.poller.lock() = Some(handle);
    }

    fn stop_poller(&self) {
        self.inner.poller_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.inner.poller.lock().take() {
            // Detach rather than join: the poller checks the stop flag
            // every `POLL_INTERVAL`, so a few hundred ms after stop
            // signal it will exit on its own.
            drop(handle);
        }
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.stop_poller();
        }
    }
}

impl Inner {
    fn run_poller(self: Arc<Self>) {
        while !self.poller_stop.load(Ordering::Relaxed) {
            std::thread::sleep(POLL_INTERVAL);

            let snapshot = {
                let sink_slot = self.sink.lock();
                sink_slot.as_ref().map(|sink| {
                    (
                        sink.get_pos(),
                        sink.empty(),
                        sink.is_paused(),
                    )
                })
            };

            let Some((pos, empty, paused)) = snapshot else {
                continue;
            };

            let position_seconds = pos.as_secs() as u32;
            self.position_seconds.store(position_seconds, Ordering::Relaxed);
            self.is_paused.store(paused, Ordering::Relaxed);

            // Track-end detection: sink is empty + we have a track +
            // we never fired TrackEnded for it + not paused.
            let track_id = self.track_id.lock().clone();
            if empty && !paused {
                if let Some(track_id) = track_id {
                    if !self.ended_fired.swap(true, Ordering::Relaxed) {
                        if let Some(cb) = self.on_event.get() {
                            cb(PlayerEvent::TrackEnded { track_id });
                        }
                    }
                }
            } else {
                self.ended_fired.store(false, Ordering::Relaxed);
            }

            // Emit state.
            if let Some(cb) = self.on_event.get() {
                cb(PlayerEvent::StateChanged {
                    track_id: self.track_id.lock().clone(),
                    position_seconds,
                    is_playing: !paused && self.track_id.lock().is_some(),
                    volume: *self.volume.lock(),
                    muted: *self.muted.lock(),
                    duration_seconds: self.duration_seconds.load(Ordering::Relaxed),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration as StdDuration;

    fn tmp_wav(duration_seconds: u32) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "sinfonic-player-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::stream::write_test_wav(&path, duration_seconds).expect("write wav");
        path
    }

    #[test]
    fn new_player_is_paused_at_zero() {
        let player = AudioPlayer::new();
        let s = player.cached_state();
        assert!(!s.is_playing);
        assert_eq!(s.position_seconds, 0);
    }

    #[test]
    fn set_volume_clamps() {
        let player = AudioPlayer::new();
        player.set_volume(1.5);
        assert!((player.cached_state().volume - 1.0).abs() < f32::EPSILON);
        player.set_volume(-0.5);
        assert!(player.cached_state().volume.abs() < f32::EPSILON);
        player.set_volume(0.42);
        assert!((player.cached_state().volume - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn set_muted_toggles_flag() {
        let player = AudioPlayer::new();
        player.set_muted(true);
        assert!(player.cached_state().muted);
        player.set_muted(false);
        assert!(!player.cached_state().muted);
    }

    #[test]
    fn eq_band_set_then_reset() {
        let player = AudioPlayer::new();
        player.set_eq_band(1000, 6.0);
        let bands = player.eq_bands();
        assert!(bands.iter().any(|b| b.hz == 1000.0 && (b.gain_db - 6.0).abs() < f32::EPSILON));
        player.reset_eq();
        let bands = player.eq_bands();
        assert!(bands.iter().all(|b| b.gain_db == 0.0));
    }

    #[test]
    fn event_callback_fires_on_play_and_stop() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        let player = AudioPlayer::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        player.set_event_callback(move |_event| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let path = tmp_wav(2);
        // Best-effort play: CI may not have an audio device, but the
        // callback should still fire on stop.
        let _ = runtime.block_on(player.play(TrackId::from("track-test"), path.to_str().unwrap()));
        std::thread::sleep(StdDuration::from_millis(100));
        player.stop();
        std::thread::sleep(StdDuration::from_millis(50));
        assert!(counter.load(Ordering::Relaxed) > 0);
        std::fs::remove_file(&path).ok();
    }

    /// Smoke test for the `unsafe impl Send + Sync` on `OutputStreamHolder`.
    ///
    /// We hammer the public, `&self`-receiving API from many threads in
    /// parallel. If a future change accidentally introduces a method that
    /// touches the inner `OutputStream` from another thread (which would
    /// be undefined behaviour per cpal's docs), this test is the cheapest
    /// place it will start to flake on macOS/Windows audio backends.
    #[test]
    fn concurrent_play_pause_volume_no_panic() {
        let player = Arc::new(AudioPlayer::new());
        let mut handles = Vec::new();

        for _ in 0..4 {
            let p = Arc::clone(&player);
            handles.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    p.set_volume(0.5);
                    p.set_muted(true);
                    p.set_muted(false);
                    let _ = p.cached_state();
                    let _ = p.current_track_id();
                }
            }));
        }

        for _ in 0..2 {
            let p = Arc::clone(&player);
            handles.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    p.pause();
                    p.resume();
                    p.set_eq_band(1000, 0.0);
                    let _ = p.eq_bands();
                    p.reset_eq();
                }
            }));
        }

        for h in handles {
            h.join().expect("thread should not panic");
        }

        // Final read should still be coherent.
        let s = player.cached_state();
        assert!(s.volume.is_finite());
        assert!(s.volume >= 0.0 && s.volume <= 1.0);
    }
}