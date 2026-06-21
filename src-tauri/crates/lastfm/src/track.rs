//! Scrobble payload + helper builders.

use serde::{Deserialize, Serialize};

/// "Source" parameter sent on every scrobble/now-playing call.
/// Last.fm uses this for analytics; `P` means "chosen by the user".
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScrobbleSource {
    /// `P` — chosen by the user (Sinfonic qualifies).
    #[default]
    User,
    /// `R` — non-personalised broadcast (e.g. a radio station).
    NonPersonalised,
    /// `E` — last.fm-recommended.
    Recommended,
}

impl ScrobbleSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ScrobbleSource::User => "P",
            ScrobbleSource::NonPersonalised => "R",
            ScrobbleSource::Recommended => "E",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Scrobble {
    pub artist: String,
    pub track: String,
    /// Album name (optional but recommended — improves stats).
    pub album: Option<String>,
    /// Track duration in seconds (optional — improves stats).
    pub duration_seconds: Option<u32>,
    /// Unix timestamp (seconds) of when the user started listening.
    pub timestamp_unix: u64,
    /// Optional MusicBrainz ID for higher scrobble accuracy.
    pub mbid: Option<String>,
}

impl Scrobble {
    pub fn now_playing(artist: String, track: String, album: Option<String>) -> Self {
        Self {
            artist,
            track,
            album,
            duration_seconds: None,
            timestamp_unix: now_unix(),
            mbid: None,
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
