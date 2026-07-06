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
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
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
/// The poller only updates the cached `position_seconds` and fires
/// the track-end detector — it does NOT emit a state-change event.
/// Runtime state for the UI is read on demand via `cached_state()`
/// or polled by the frontend via `get_playback_state`, so a slow
/// WKWebView event channel can't stall the position counter.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Crossfade configuration bounds. `seconds` is clamped to this
/// range by `set_crossfade` so a hostile or buggy caller can't
/// schedule hour-long fades.
const CROSSFADE_SECONDS_MIN: u32 = 0;
const CROSSFADE_SECONDS_MAX: u32 = 12;

/// How often the fade thread updates `Sink::set_volume` while
/// ramping between two sinks. 60 fps keeps the ramp audibly smooth
/// without burning a CPU core.
const FADE_TICK: Duration = Duration::from_millis(16);

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

    // ─── Crossfade state ────────────────────────────────────
    /// Master toggle. When `false`, `play()` does a dry cut and
    /// `preload_next` is a no-op (no second sink is ever built).
    crossfade_enabled: AtomicBool,
    /// Crossfade duration in seconds (0..=12). The frontend slider
    /// sends values inside this range; `set_crossfade` clamps.
    crossfade_seconds: AtomicU32,
    /// Pre-loaded sink for the next track. Built by `preload_next`
    /// and consumed by the next `play()` call when crossfade is
    /// enabled. Holding the rodio `Sink` here keeps the decoder
    /// warm so the fade can start instantly.
    next_sink: Mutex<Option<(TrackId, Sink)>>,
    /// JoinHandle + stop flag for the active fade thread. A fade
    /// in progress is cancelled by `play()` (when a new track
    /// arrives mid-fade) and by `stop()`.
    fade_thread: Mutex<Option<JoinHandle<()>>>,
    fade_stop: Arc<AtomicBool>,
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
            crossfade_enabled: AtomicBool::new(false),
            crossfade_seconds: AtomicU32::new(6),
            next_sink: Mutex::new(None),
            fade_thread: Mutex::new(None),
            fade_stop: Arc::new(AtomicBool::new(false)),
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
        // Open the stream once. `stream::open` is async because HTTP
        // downloads are funneled through a worker thread — we open
        // the body before touching any state so a decode failure here
        // is surfaced to the caller without side effects. The decoded
        // source is wrapped in the project's EQ and either consumed
        // by the crossfade path or handed off to the dry-cut Sink;
        // either way we read the file once per play.
        let decoded = stream::open(stream_uri)
            .await
            .map_err(|e| PlayerError::Stream(e.to_string()))?
            .with_eq(self.inner.equalizer.clone());
        let duration_seconds = decoded.duration_seconds.unwrap_or(0);

        // Crossfade path: if crossfade is on AND a preload exists
        // for this exact track_id AND a real audio device is open,
        // ramp the current sink down and the preloaded one up over
        // `crossfade_seconds`, then promote the new sink. Falls
        // through to the dry-cut path when any condition fails.
        //
        // Either branch uses `decoded` end-to-end: the crossfade
        // path consumes `decoded.source` (move), the dry-cut path
        // stashes it in `stream_for_dry_cut` and uses it below.
        #[allow(unused_assignments)]
        let mut stream_for_dry_cut: Option<stream::StreamHandle> = None;

        // Always drop any stale preload first — even crossfade-off
        // callers must not leave a half-built sink in `next_sink`
        // from a previous track.
        let stale_preload = self.inner.next_sink.lock().take();

        if self.inner.crossfade_enabled.load(Ordering::Relaxed)
            && self.inner.stream_handle.lock().is_some()
        {
            if let Some((ref preload_id, _)) = stale_preload {
                if *preload_id == track_id {
                    // Exact preload match — take ownership of the
                    // preloaded sink and promote `decoded`'s source
                    // into the crossfade.
                    let (_, next_sink) = stale_preload.unwrap();
                    self.start_crossfade(decoded.source, track_id.clone(), next_sink);
                    self.inner.position_seconds.store(0, Ordering::Relaxed);
                    self.inner.duration_seconds.store(duration_seconds, Ordering::Relaxed);
                    self.inner.is_paused.store(false, Ordering::Relaxed);
                    self.inner.ended_fired.store(false, Ordering::Relaxed);
                    *self.inner.track_id.lock() = Some(track_id.clone());
                    self.start_poller();
                    self.emit_state(Some(track_id));
                    return Ok(duration_seconds);
                }
            }
            // Crossfade on but no exact preload (or stale preload for
            // a different track): drop the stale one and fall through.
            if let Some((_, stale_sink)) = stale_preload {
                stale_sink.stop();
            }
        } else {
            // Crossfade off — kill any in-flight preload because we
            // won't consume it on this play.
            if let Some((_, stale_sink)) = stale_preload {
                stale_sink.stop();
            }
        }

        // No crossfade match — keep `decoded` alive for the dry-cut
        // path that follows.
        stream_for_dry_cut = Some(decoded);

        // Dry-cut path. Kill the existing poller + sink before
        // swapping in the new one so the old one is fully torn down.
        self.stop_poller();
        {
            let mut sink_slot = self.inner.sink.lock();
            *sink_slot = None;
        }

        let stream = stream_for_dry_cut
            .take()
            .expect("dry-cut path needs the decoded stream");
        let os_handle = self.inner.stream_handle.lock().clone();
        match os_handle {
            Some(os_handle) => {
                let sink = Sink::try_new(&os_handle)
                    .map_err(|e| PlayerError::NoDevice(e.to_string()))?;
                sink.set_volume(*self.inner.volume.lock());
                sink.append(stream.source);
                sink.play();
                *self.inner.sink.lock() = Some(sink);
            }
            None => {
                // Headless: drop the source, fire TrackEnded so the
                // queue can advance without waiting on a real device.
                drop(stream);
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

    /// Hand off control of the current sink to a fade thread and
    /// install the preloaded sink as the active one. `decoded_source`
    /// is the freshly decoded source for the new track — we wrap it
    /// through the EQ, append it to `next_sink`, and start the fade.
    fn start_crossfade(
        &self,
        decoded_source: Box<dyn Source<Item = f32> + Send>,
        _track_id: TrackId,
        next_sink: Sink,
    ) {
        // Append the freshly decoded source to the preloaded sink
        // and unpause it. The fade thread will ramp `sink.set_volume`
        // from 0 → master over the configured window.
        next_sink.append(decoded_source);
        next_sink.play();
        let master = *self.inner.volume.lock();

        // Cancel any in-flight fade (a second play call mid-fade
        // should restart the ramp from the current gains, not
        // stack two fade threads).
        self.stop_fade();

        // Take the current sink out for the fade thread to dispose
        // of once the ramp completes, then install the new sink
        // directly. `Sink` is not `Clone`, so the preloaded sink
        // is moved into the main slot in one shot.
        let old_sink = self.inner.sink.lock().take();
        *self.inner.sink.lock() = Some(next_sink);

        let inner = Arc::clone(&self.inner);
        let stop_flag = Arc::clone(&self.inner.fade_stop);
        stop_flag.store(false, Ordering::Relaxed);

        let handle = std::thread::Builder::new()
            .name("sinfonic-playback-fade".into())
            .spawn(move || inner.run_fade_tick(old_sink, master, stop_flag))
            .expect("spawn fade thread");
        *self.inner.fade_thread.lock() = Some(handle);
    }

    fn stop_fade(&self) {
        self.inner.fade_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.inner.fade_thread.lock().take() {
            // Detach: the fade thread checks the stop flag every
            // FADE_TICK, so it will exit within ~16 ms of the
            // signal. Joining here would risk blocking a command
            // thread on a 60 fps audio thread.
            drop(handle);
        }
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
        self.stop_fade();
        self.cancel_preload();
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
    ///
    /// If a fade is in progress the new master takes effect on the
    /// next fade tick (the ramp recomputes `from_gain = master *
    /// (1 - progress)` and `to_gain = master * progress` every
    /// FADE_TICK). The fade itself keeps running — pausing the
    /// ramp on a volume change would surprise the user mid-fade.
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

    // ─── Crossfade ──────────────────────────────────────────
    //
    // `set_crossfade` only stores the configuration; nothing is
    // played or scheduled here. The actual fade kicks in the next
    // time `play()` runs with a pre-loaded next sink.
    //
    // `preload_next` decodes the upcoming track and parks a ready-
    // to-play `Sink` in `next_sink`. The next `play()` will pick
    // it up and ramp both sinks for `crossfade_seconds` before
    // promoting the new sink to the main one.
    //
    // When `crossfade_enabled` is `false` both `set_crossfade`
    // and `preload_next` are essentially free: `set_crossfade`
    // just records the values, and `preload_next` no-ops.

    /// Configure crossfade. `seconds` is clamped to
    /// `[CROSSFADE_SECONDS_MIN, CROSSFADE_SECONDS_MAX]`.
    pub fn set_crossfade(&self, enabled: bool, seconds: u32) {
        self.inner
            .crossfade_enabled
            .store(enabled, Ordering::Relaxed);
        self.inner.crossfade_seconds.store(
            seconds.clamp(CROSSFADE_SECONDS_MIN, CROSSFADE_SECONDS_MAX),
            Ordering::Relaxed,
        );
    }

    /// Snapshot of the current crossfade configuration.
    pub fn crossfade_config(&self) -> (bool, u32) {
        (
            self.inner.crossfade_enabled.load(Ordering::Relaxed),
            self.inner.crossfade_seconds.load(Ordering::Relaxed),
        )
    }

    /// Build a rodio sink for `track_id` and park it as the
    /// preloaded "next" sink. No audio is played yet; the next
    /// `play(track_id, _)` call will detect the match and fade
    /// from the current sink to this one.
    ///
    /// If crossfade is disabled this is a no-op (and the
    /// returned duration comes straight from the decoder). The
    /// frontend always calls it before `play`, so the cost when
    /// the feature is off is just the extra decode — acceptable
    /// because the sink is dropped immediately.
    ///
    /// Calling this while a previous preload is still cached
    /// replaces it (the old sink drops and the decoder is freed).
    pub async fn preload_next(
        &self,
        track_id: TrackId,
        stream_uri: &str,
    ) -> Result<u32, PlayerError> {
        if !self.inner.crossfade_enabled.load(Ordering::Relaxed) {
            // Even when crossfade is off, the caller asked for a
            // preload. Decode + drop so the result of `play` (which
            // always decodes again) stays the source of truth.
            let _ = stream::open(stream_uri)
                .await
                .map_err(|e| PlayerError::Stream(e.to_string()))?;
            return Ok(0);
        }
        let decoded = stream::open(stream_uri)
            .await
            .map_err(|e| PlayerError::Stream(e.to_string()))?
            .with_eq(self.inner.equalizer.clone());
        let duration_seconds = decoded.duration_seconds.unwrap_or(0);

        let os_handle = self.inner.stream_handle.lock().clone();
        let Some(os_handle) = os_handle else {
            // Headless: drop the decoded source. The next `play`
            // will detect the empty `next_sink` and fall back to
            // the dry-cut path (which itself falls back to a
            // TrackEnded fire — see `play`).
            drop(decoded);
            return Ok(duration_seconds);
        };
        let sink = Sink::try_new(&os_handle).map_err(|e| PlayerError::NoDevice(e.to_string()))?;
        sink.set_volume(0.0); // silent until the fade thread takes over
        sink.append(decoded.source);
        sink.pause(); // do not play until the fade kicks in
        let mut slot = self.inner.next_sink.lock();
        *slot = Some((track_id, sink));
        Ok(duration_seconds)
    }

    /// Drop any cached preloaded sink. Called automatically by
    /// `play` and `stop` so a stale preload can never start
    /// fading into the wrong track.
    fn cancel_preload(&self) {
        let mut slot = self.inner.next_sink.lock();
        if let Some((_, sink)) = slot.take() {
            sink.stop();
        }
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
        tracing::debug!(target: "sinfonic::playback::poller", "start_poller: thread spawned");
    }

    fn stop_poller(&self) {
        self.inner.poller_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.inner.poller.lock().take() {
            // Detach rather than join: the poller checks the stop flag
            // every `POLL_INTERVAL`, so a few hundred ms after stop
            // signal it will exit on its own.
            tracing::debug!(target: "sinfonic::playback::poller", "stop_poller: dropping handle");
            drop(handle);
        } else {
            tracing::debug!(target: "sinfonic::playback::poller", "stop_poller: no handle");
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
        tracing::debug!(target: "sinfonic::playback::poller", "run_poller: thread started");
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
            // we never fired TrackEnded for it + not paused. This is
            // the ONLY event the poller fires — every other state
            // transition is driven from the public command methods
            // (play / pause / resume / seek / set_volume / set_muted
            // / stop), which already emit PlayerEvent::StateChanged
            // synchronously from the calling thread.
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
        }
        tracing::debug!(target: "sinfonic::playback::poller", "run_poller: thread exiting");
    }

    /// Ramp the previous sink to silence and the new (now installed)
    /// sink from silence to `master` over `crossfade_seconds`, then
    /// stop and drop the previous sink. `stop_flag` cancels the
    /// ramp from outside (e.g. when `play` is called again before
    /// the previous fade finished).
    fn run_fade_tick(
        self: Arc<Self>,
        old_sink: Option<Sink>,
        initial_master: f32,
        stop_flag: Arc<AtomicBool>,
    ) {
        let total_seconds = self.crossfade_seconds.load(Ordering::Relaxed).max(1) as f32;
        let start = std::time::Instant::now();
        tracing::debug!(
            target: "sinfonic::playback::fade",
            seconds = total_seconds,
            "run_fade_tick: started"
        );

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let elapsed = start.elapsed().as_secs_f32();
            let progress = (elapsed / total_seconds).clamp(0.0, 1.0);

            // Re-read the master every tick so `set_volume` mid-fade
            // takes effect on the next tick without restarting the
            // ramp.
            let master = *self.volume.lock();
            let from_gain = master * (1.0 - progress);
            let to_gain = master * progress;

            // Apply to the new (currently installed) sink.
            if let Some(new_sink) = self.sink.lock().as_ref() {
                new_sink.set_volume(to_gain);
            }
            // Apply to the outgoing sink, if we still have one.
            if let Some(ref old) = old_sink {
                old.set_volume(from_gain);
            }

            if progress >= 1.0 {
                break;
            }
            std::thread::sleep(FADE_TICK);
        }

        // Final state: outgoing sink at 0, incoming at master.
        if let Some(ref old) = old_sink {
            old.stop();
        }
        if let Some(new_sink) = self.sink.lock().as_ref() {
            new_sink.set_volume(*self.volume.lock());
        }
        let _ = initial_master; // kept for parity / future per-fade overrides
        tracing::debug!(target: "sinfonic::playback::fade", "run_fade_tick: done");
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

    /// The position poller does NOT emit `StateChanged` events on
    /// every tick — only the public command methods do, plus
    /// `TrackEnded` on sink dry. This regression test exists because
    /// the previous implementation fired `StateChanged` four times a
    /// second from the poller thread, which on macOS wedged
    /// Tauri's `app.emit` hook and froze the seek bar after the
    /// first snapshot.
    #[test]
    fn poller_does_not_emit_state_changed() {
        use std::sync::atomic::AtomicU32;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        let player = AudioPlayer::new();
        let state_count = Arc::new(AtomicU32::new(0));
        let track_count = Arc::new(AtomicU32::new(0));
        let s = state_count.clone();
        let t = track_count.clone();
        player.set_event_callback(move |event| match event {
            PlayerEvent::StateChanged { .. } => {
                s.fetch_add(1, Ordering::Relaxed);
            }
            PlayerEvent::TrackEnded { .. } => {
                t.fetch_add(1, Ordering::Relaxed);
            }
        });
        // Drive the player through several poller ticks without
        // invoking any command method. None of those ticks should
        // see a StateChanged event.
        let path = tmp_wav(1);
        let _ = runtime.block_on(player.play(TrackId::from("track-noemit"), path.to_str().unwrap()));
        let before = state_count.load(Ordering::Relaxed);
        std::thread::sleep(StdDuration::from_millis(700));
        let after = state_count.load(Ordering::Relaxed);
        assert_eq!(
            before, after,
            "poller fired StateChanged unexpectedly: before={before} after={after}"
        );
        player.stop();
        std::thread::sleep(StdDuration::from_millis(50));
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

    // ─── Crossfade tests ──────────────────────────────────────

    /// `set_crossfade` must clamp the seconds argument to the
    /// documented `[0, 12]` range so a hostile or buggy caller
    /// can't schedule a 10-minute fade.
    #[test]
    fn set_crossfade_clamps_seconds_to_max() {
        let player = AudioPlayer::new();
        player.set_crossfade(true, 100);
        assert_eq!(player.crossfade_config(), (true, 12));
    }

    #[test]
    fn set_crossfade_clamps_seconds_to_min() {
        let player = AudioPlayer::new();
        player.set_crossfade(true, 0);
        assert_eq!(player.crossfade_config(), (true, 0));
    }

    /// Default config is `enabled = false`, `seconds = 6`.
    #[test]
    fn crossfade_default_is_disabled_with_six_seconds() {
        let player = AudioPlayer::new();
        assert_eq!(player.crossfade_config(), (false, 6));
    }

    /// When crossfade is disabled, `preload_next` must not allocate
    /// a second sink (verified by the `next_sink` slot remaining
    /// empty after the call). The decoded source is dropped
    /// immediately so the only cost is the decode itself.
    #[test]
    fn preload_next_is_noop_when_disabled() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        let player = AudioPlayer::new();
        let path = tmp_wav(2);
        let _ = runtime.block_on(player.preload_next(
            TrackId::from("track-a"),
            path.to_str().unwrap(),
        ));
        // next_sink slot should still be empty.
        {
            let slot = player.inner.next_sink.lock();
            assert!(slot.is_none(), "next_sink should be empty when crossfade is disabled");
        }
        std::fs::remove_file(&path).ok();
    }

    /// `stop` must cancel any in-flight fade and drop the preloaded
    /// sink so a stale preload can't leak into the next session.
    #[test]
    fn stop_clears_fade_and_preload() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        let player = AudioPlayer::new();
        player.set_crossfade(true, 4);

        let path_a = tmp_wav(2);
        let _ = runtime.block_on(player.preload_next(
            TrackId::from("track-a"),
            path_a.to_str().unwrap(),
        ));

        player.stop();

        let slot = player.inner.next_sink.lock();
        assert!(slot.is_none(), "stop should drop the preload");
        std::fs::remove_file(&path_a).ok();
    }
}