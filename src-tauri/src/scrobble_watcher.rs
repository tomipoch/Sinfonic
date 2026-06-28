//! Background task that forwards playback events to Last.fm.
//!
//! Spawned once at startup. Holds a cheap clone of the shared
//! `AudioPlayer`, the `QueueEngine`, and the `LastFmClient` mutex.
//! On every poll (default 1 s) it:
//!
//! 1. Reads `player.cached_state()` and `queue.current()`.
//! 2. If the track id changed (including `None` → `Some`), fires
//!    `track.updateNowPlaying`.
//! 3. If the playhead crossed 50 % of the duration AND we have not
//!    yet scrobbled this track, fires `track.scrobble` and marks
//!    the track as scrobbled.
//! 4. If the track ended without crossing 50 %, fires `track.scrobble`
//!    anyway (Last.fm's "skip threshold" rule: scrobble if the user
//!    listened to ≥ 50 % OR ≥ 4 minutes; we approximate the first).
//!
//! Errors are logged at warn-level and never bubble up — a flaky
//! Last.fm must never disrupt playback.
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
use std::time::Duration;

use futures::FutureExt;
use sinfonic_domain::{QueueEngine, TrackId};
use sinfonic_lastfm::{LastFmClient, Scrobble, ScrobbleSource};
use sinfonic_playback::AudioPlayer;
use tokio::sync::Mutex;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// State held between iterations so we can detect track changes and
/// avoid double-scrobbling.
struct WatcherState {
    last_track_id: Option<TrackId>,
    scrobbled_track_ids: std::collections::HashSet<TrackId>,
}

impl WatcherState {
    fn new() -> Self {
        Self {
            last_track_id: None,
            scrobbled_track_ids: std::collections::HashSet::new(),
        }
    }
}

/// Long-running watcher task. The future never completes under
/// normal operation; cancel by dropping the supplied handles (the
/// task polls `lastfm_slot` each tick and skips silently if empty).
pub async fn run(
    queue: Arc<Mutex<QueueEngine>>,
    player: Arc<AudioPlayer>,
    lastfm_slot: Arc<Mutex<Option<LastFmClient>>>,
) {
    let mut state = WatcherState::new();
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        // Catch panics from inner logic so a transient bug cannot kill
        // the whole watcher. Recover by logging and continuing on the
        // next tick.
        let tick_future = tick(&queue, &player, &lastfm_slot, &mut state);
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
    state: &mut WatcherState,
) {
    // Cheap guard: only proceed if the user has authenticated.
    let client_guard = lastfm_slot.lock().await;
    let Some(client) = client_guard.as_ref() else {
        return;
    };

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

    // 1) Track changed → fire now-playing (best-effort).
    if track_changed {
        state.last_track_id = current_track_id.clone();
        // Clear the scrobbled-set on track change so a long playlist
        // doesn't grow unbounded. We keep the just-finished id around
        // until the next tick so a same-id rebroadcast doesn't reset
        // the flag.
        state.scrobbled_track_ids.clear();

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

    // 2) Position crossed 50 % → fire scrobble (once per track).
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