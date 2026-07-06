//! Background task that forwards playback events to Last.fm and to
//! the active music provider.
//!
//! Spawned once at startup. Holds a cheap clone of the shared
//! `AudioPlayer`, the `QueueEngine`, the `LastFmClient` mutex, and
//! a handle to the active `MusicProvider` slot. On every poll
//! (default 1 s) it:
//!
//! 1. Reads `player.cached_state()` and `queue.current()`.
//! 2. **Provider playback report (always fires when a track is
//!    active):** sends `Started` on track change, throttled
//!    `Progress` updates every 30 s while the same track is
//!    playing. Subsonic implements this as `POST /rest/scrobble`,
//!    Jellyfin as `POST /Sessions/Playing`. Local returns
//!    `Unsupported` and is silently dropped — the watcher keeps
//!    ticking unaffected.
//! 3. **Last.fm (opt-in):** if the user has authenticated, fires
//!    `track.updateNowPlaying` on track change and
//!    `track.scrobble` at 50 %.
//!
//! Errors are logged at warn-level and never bubble up — a flaky
//! Last.fm or upstream provider must never disrupt playback.
//!
//! # Resilience
//!
//! Each iteration runs inside [`std::panic::AssertUnwindSafe`] +
//! [`futures::FutureExt::catch_unwind`]. A panic in any inner future
//! is logged but does **not** terminate the watcher — the next tick
//! runs normally. This guarantees scrobbling continues even after a
//! future bug in the HTTP client or domain mapping.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::FutureExt;
use sinfonic_domain::{QueueEngine, TrackId};
use sinfonic_lastfm::{LastFmClient, Scrobble, ScrobbleSource};
use sinfonic_playback::AudioPlayer;
use sinfonic_source::{MusicProvider, PlaybackReport, PlaybackReportKind, ProviderError};
use tokio::sync::Mutex;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Throttle for the per-track `Progress` report so we don't hammer
/// the upstream `/Sessions/Playing` (Jellyfin) or `/rest/scrobble`
/// (Subsonic) once per second. The first `Started` on track change
/// fires immediately, then `Progress` every `PROGRESS_THROTTLE`.
const PROGRESS_THROTTLE: Duration = Duration::from_secs(30);

/// State held between iterations so we can detect track changes and
/// avoid double-scrobbling.
struct WatcherState {
    last_track_id: Option<TrackId>,
    scrobbled_track_ids: std::collections::HashSet<TrackId>,
    last_progress_at: Option<Instant>,
}

impl WatcherState {
    fn new() -> Self {
        Self {
            last_track_id: None,
            scrobbled_track_ids: std::collections::HashSet::new(),
            last_progress_at: None,
        }
    }
}

/// Long-running watcher task. The future never completes under
/// normal operation; cancel by dropping the supplied handles. The
/// provider report path runs independently of Last.fm, so the
/// watcher ticks even when the user has not configured Last.fm.
pub async fn run(
    queue: Arc<Mutex<QueueEngine>>,
    player: Arc<AudioPlayer>,
    lastfm_slot: Arc<Mutex<Option<LastFmClient>>>,
    provider_slot: Arc<Mutex<Option<Arc<dyn MusicProvider>>>>,
) {
    let mut state = WatcherState::new();
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        // Catch panics from inner logic so a transient bug cannot kill
        // the whole watcher. Recover by logging and continuing on the
        // next tick.
        let tick_future = tick(&queue, &player, &lastfm_slot, &provider_slot, &mut state);
        let outcome = AssertUnwindSafe(tick_future).catch_unwind().await;

        if let Err(payload) = outcome {
            let message = panic_message(&payload);
            tracing::error!(
                target: "sinfonic::scrobble_watcher",
                panic = %message,
                "scrobble watcher tick panicked; recovering on next interval"
            );
            // Reset transient state on panic so we re-detect changes.
            state.last_track_id = None;
            state.scrobbled_track_ids.clear();
            state.last_progress_at = None;
        }
    }
}

/// Extracts a printable message from a `catch_unwind` payload.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "<non-string panic payload>"
    }
}

async fn tick(
    queue: &Arc<Mutex<QueueEngine>>,
    player: &Arc<AudioPlayer>,
    lastfm_slot: &Arc<Mutex<Option<LastFmClient>>>,
    provider_slot: &Arc<Mutex<Option<Arc<dyn MusicProvider>>>>,
    state: &mut WatcherState,
) {
    let cached = player.cached_state();
    let current_track_id = player.current_track_id();
    let current_entry = {
        let queue_guard = queue.lock().await;
        queue_guard.current().cloned()
    };

    // Detect a new track.
    let track_changed = current_track_id != state.last_track_id;
    let position_crossed_half = current_entry
        .as_ref()
        .zip(Some(cached.duration_seconds))
        .is_some_and(|(_entry, duration)| {
            duration > 0 && cached.position_seconds >= duration / 2
        });

    // 1) Provider playback report. Always runs (independent of
    //    Last.fm); Subsonic/Jellyfin forward playback to their own
    //    scrobble / "now playing" panels, LocalProvider returns
    //    `Unsupported` and is silently dropped.
    if track_changed {
        state.last_track_id = current_track_id.clone();
        state.scrobbled_track_ids.clear();
        // The throttling timer resets on track change so the next
        // 30s tick always re-fires `Progress` after `Started`.
        state.last_progress_at = None;
        if current_track_id.is_some() && current_entry.is_some() {
            send_provider_report(
                provider_slot,
                build_report(
                    PlaybackReportKind::Started,
                    &cached,
                    current_track_id.as_ref().unwrap(),
                ),
            )
            .await;
        }
    } else if current_track_id.is_some()
        && state
            .last_progress_at
            .map(|t| t.elapsed() >= PROGRESS_THROTTLE)
            .unwrap_or(true)
    {
        send_provider_report(
            provider_slot,
            build_report(
                PlaybackReportKind::Progress,
                &cached,
                current_track_id.as_ref().unwrap(),
            ),
        )
        .await;
        state.last_progress_at = Some(Instant::now());
    }

    // 2) Last.fm path (opt-in).
    let client = {
        let guard = lastfm_slot.lock().await;
        guard.as_ref().cloned()
    };
    if let Some(client) = client {
        // 2a) Track changed → fire now-playing.
        if track_changed {
            if let (Some(track_id), Some(entry)) = (&current_track_id, &current_entry) {
                if entry.track_id == *track_id {
                    tracing::info!(
                        target: "sinfonic::scrobble_watcher",
                        artist = %entry.artist,
                        title = %entry.title,
                        "track changed; sending now-playing"
                    );
                    let scrobble = Scrobble {
                        artist: entry.artist.clone(),
                        track: entry.title.clone(),
                        album: Some(entry.album.clone()),
                        duration_seconds: Some(cached.duration_seconds),
                        timestamp_unix: now_unix(),
                        mbid: None,
                    };
                    match client.now_playing(&scrobble, ScrobbleSource::User).await {
                        Ok(()) => tracing::debug!(
                            target: "sinfonic::scrobble_watcher",
                            "now-playing accepted by Last.fm"
                        ),
                        Err(err) => tracing::warn!(
                            target: "sinfonic::scrobble_watcher",
                            error = %err,
                            "now-playing request failed"
                        ),
                    }
                }
            }
        }

        // 2b) Position crossed 50 % → fire scrobble (once per track).
        if position_crossed_half {
            if let (Some(track_id), Some(entry)) = (&current_track_id, &current_entry) {
                if entry.track_id == *track_id
                    && !state.scrobbled_track_ids.contains(track_id)
                {
                    tracing::info!(
                        target: "sinfonic::scrobble_watcher",
                        artist = %entry.artist,
                        title = %entry.title,
                        "position crossed 50%; submitting scrobble"
                    );
                    let scrobble = Scrobble {
                        artist: entry.artist.clone(),
                        track: entry.title.clone(),
                        album: Some(entry.album.clone()),
                        duration_seconds: Some(cached.duration_seconds),
                        timestamp_unix: now_unix(),
                        mbid: None,
                    };
                    match client.scrobble(&scrobble, ScrobbleSource::User).await {
                        Ok(accepted) => {
                            tracing::info!(
                                target: "sinfonic::scrobble_watcher",
                                accepted,
                                "scrobble submitted"
                            );
                            if accepted {
                                state.scrobbled_track_ids.insert(track_id.clone());
                            }
                        }
                        Err(err) => tracing::warn!(
                            target: "sinfonic::scrobble_watcher",
                            error = %err,
                            "scrobble submission failed"
                        ),
                    }
                }
            }
        }
    }
}

/// Build a `PlaybackReport` from the audio player's cached state.
/// `kind` discriminates `Started` (track-change tick) from
/// `Progress` (regular polled updates). The implementation fills
/// every field the provider trait's `report_playback` reads;
/// `shuffle` / `repeat_one` / `repeat_all` default to `false`
/// here because the audio player doesn't surface them — a
/// future refactor can plumb them through the player state.
fn build_report(
    kind: PlaybackReportKind,
    cached: &sinfonic_domain::PlaybackState,
    track_id: &TrackId,
) -> PlaybackReport {
    PlaybackReport {
        kind,
        track_id: track_id.clone(),
        position_seconds: cached.position_seconds,
        paused: !cached.is_playing,
        muted: cached.muted,
        volume_percent: (cached.volume.clamp(0.0, 1.0) * 100.0 + 0.5) as u8,
        shuffle: false,
        repeat_one: false,
        repeat_all: false,
        failed: false,
    }
}

/// Fire-and-forget `provider.report_playback` against the active
/// provider. `Unsupported` (LocalProvider) is silently dropped;
/// transient network errors are logged at warn and never bubble
/// up — the watcher keeps ticking.
async fn send_provider_report(
    provider_slot: &Arc<Mutex<Option<Arc<dyn MusicProvider>>>>,
    report: PlaybackReport,
) {
    let provider = {
        let guard = provider_slot.lock().await;
        guard.as_ref().cloned()
    };
    let Some(provider) = provider else {
        return;
    };
    if let Err(e) = provider.report_playback(report).await {
        match e {
            ProviderError::Unsupported(_) => {}
            other => tracing::warn!(
                target: "sinfonic::scrobble_watcher",
                error = %other,
                "provider report_playback failed (continuing)"
            ),
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_message_handles_str_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("oops");
        assert_eq!(panic_message(&payload), "oops");
    }

    #[test]
    fn panic_message_handles_string_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from("owned"));
        assert_eq!(panic_message(&payload), "owned");
    }

    #[test]
    fn panic_message_handles_unknown_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(42_i32);
        assert_eq!(panic_message(&payload), "<non-string panic payload>");
    }
}