//! Application settings (persisted as JSON to disk in `library`).
//!
//! This is the *type* of the settings blob; the persistence layer lives
//! in `sinfonic-library`. Keep this struct `Serialize`/`Deserialize` so
//! we can move to `toml` or `bincode` later without code changes.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppSettings {
    pub volume: f32,
    pub muted: bool,
    pub repeat: super::queue::RepeatMode,
    pub shuffle: bool,
    pub last_server_id: Option<super::ids::ServerId>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            muted: false,
            repeat: super::queue::RepeatMode::Off,
            shuffle: false,
            last_server_id: None,
        }
    }
}
