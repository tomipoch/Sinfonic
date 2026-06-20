//! Integration tests for `AppState` + `QueueEngine` + `PlaybackState`.
//!
//! Lives in the app crate (not in `sinfonic-domain`) because it
//! exercises the wiring of the three layers without going through
//! Tauri's IPC plumbing.

use sinfonic_domain::{AlbumId, ArtistId, PlaybackState, RepeatMode, ServerId, Track, TrackId};
use sinfonic_lib::AppState;

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

#[test]
fn app_state_default_is_empty_and_paused() {
    let s = AppState::new();
    assert!(s.queue.is_empty());
    assert!(s.queue.current().is_none());
    assert_eq!(s.playback, PlaybackState::default());
}

#[test]
fn app_state_with_server_starts_with_that_server() {
    let s = AppState::with_server(ServerId::fake(42));
    assert_eq!(s.queue.server_id().map(|s| s.as_str()), Some("server-42"));
}

#[test]
fn play_now_populates_queue_and_starts_playback() {
    let mut s = AppState::new();
    let tracks = vec![track("a", "A", 180), track("b", "B", 200)];
    s.queue.play_now(&tracks);

    assert_eq!(s.queue.len(), 2);
    assert_eq!(s.queue.current().unwrap().title, "A");
    assert!(!s.playback.is_playing, "playback is not auto-started by play_now");

    s.playback.start(s.queue.current().unwrap().duration_seconds);
    assert!(s.playback.is_playing);
    assert_eq!(s.playback.duration_seconds, 180);
}

#[test]
fn repeat_and_shuffle_are_queue_concerns_not_playback() {
    let mut s = AppState::new();
    s.queue.set_repeat(RepeatMode::All);
    s.queue.set_shuffle(true);
    assert_eq!(s.queue.repeat(), RepeatMode::All);
    assert!(s.queue.shuffle_enabled());
    // Playback state holds the runtime fields only; mode state lives on the engine.
    assert!(!s.playback.is_playing);
    assert_eq!(s.playback, PlaybackState::default());
}

#[test]
fn add_then_next_then_previous_walks_the_queue() {
    let mut s = AppState::new();
    s.queue.play_now(&[track("a", "A", 1), track("b", "B", 1), track("c", "C", 1)]);

    let first = s.queue.current().unwrap().id.clone();
    s.queue.next_track();
    s.queue.next_track();
    assert_eq!(s.queue.current().unwrap().title, "C");
    s.queue.previous_track();
    assert_eq!(s.queue.current().unwrap().title, "B");
    s.queue.previous_track();
    assert_eq!(s.queue.current().unwrap().id, first);
}
