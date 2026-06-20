//! In-memory playback state.
//!
//! This is the *runtime* state: is audio actually playing, where the
//! playhead is, volume, mute. It is intentionally separate from the
//! `QueueEngine` (which owns *what* plays) and from the IPC payload
//! (which is the wire format).
//!
//! # Layering
//!
//! ```text
//!   ┌──────────────────────────────┐
//!   │ events::PlaybackStatePayload │   (IPC wire format, in src-tauri)
//!   └──────────────┬───────────────┘
//!                  │ built from
//!   ┌──────────────┴───────────────┐
//!   │ domain::PlaybackState        │   (this module)
//!   └──────────────┬───────────────┘
//!                  │ owned by
//!   ┌──────────────┴───────────────┐
//!   │ AppState                     │   (in src-tauri/src/state.rs)
//!   └──────────────────────────────┘
//! ```
//!
//! Repeat and shuffle are queue concerns, not playback concerns, so
//! they live in `QueueEngine`. The payload combines the two when
//! emitting events.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_seconds: u32,
    pub duration_seconds: u32,
    pub volume: f32,
    pub muted: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            position_seconds: 0,
            duration_seconds: 0,
            volume: 0.8,
            muted: false,
        }
    }
}

impl PlaybackState {
    /// Clamps volume to `[0.0, 1.0]` to defend against bad inputs.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn start(&mut self, duration_seconds: u32) {
        self.is_playing = true;
        self.position_seconds = 0;
        self.duration_seconds = duration_seconds;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn resume(&mut self) {
        if self.duration_seconds > 0 {
            self.is_playing = true;
        }
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.position_seconds = 0;
        self.duration_seconds = 0;
    }

    pub fn seek(&mut self, position_seconds: u32) {
        self.position_seconds = position_seconds.min(self.duration_seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_paused_at_zero() {
        let s = PlaybackState::default();
        assert!(!s.is_playing);
        assert_eq!(s.position_seconds, 0);
        assert_eq!(s.duration_seconds, 0);
        assert!((s.volume - 0.8).abs() < f32::EPSILON);
        assert!(!s.muted);
    }

    #[test]
    fn set_volume_clamps() {
        let mut s = PlaybackState::default();
        s.set_volume(1.5);
        assert!((s.volume - 1.0).abs() < f32::EPSILON);
        s.set_volume(-0.5);
        assert_eq!(s.volume, 0.0);
        s.set_volume(0.42);
        assert!((s.volume - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn start_sets_position_zero_and_duration() {
        let mut s = PlaybackState::default();
        s.start(180);
        assert!(s.is_playing);
        assert_eq!(s.position_seconds, 0);
        assert_eq!(s.duration_seconds, 180);
    }

    #[test]
    fn resume_keeps_stopped_when_no_track() {
        let mut s = PlaybackState::default();
        s.resume();
        assert!(!s.is_playing);
    }

    #[test]
    fn seek_clamps_to_duration() {
        let mut s = PlaybackState::default();
        s.start(180);
        s.seek(500);
        assert_eq!(s.position_seconds, 180);
        s.seek(60);
        assert_eq!(s.position_seconds, 60);
    }
}
