//! AudioPlayer — owns the rodio output stream and the queue integration.
//!
//! Phase 0: stub. Phase 4 implements the rodio pipeline + dsyneq EQ +
//! symphonia decoder.

use sinfonic_domain::TrackId;

#[derive(Default)]
pub struct AudioPlayer;

impl AudioPlayer {
    pub fn new() -> Self {
        Self
    }

    /// Play a track given its resolved stream URI.
    pub async fn play(&self, _track_id: TrackId, _stream_uri: String) -> Result<(), String> {
        Err("playback not implemented in skeleton".into())
    }

    pub async fn pause(&self) -> Result<(), String> {
        Err("playback not implemented in skeleton".into())
    }

    pub async fn resume(&self) -> Result<(), String> {
        Err("playback not implemented in skeleton".into())
    }

    pub async fn stop(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn seek(&self, _position_seconds: u32) -> Result<(), String> {
        Err("playback not implemented in skeleton".into())
    }

    pub fn set_volume(&self, _volume: f32) {}

    pub fn set_muted(&self, _muted: bool) {}
}
