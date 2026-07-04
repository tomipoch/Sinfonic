//! Response shape from LRCLIB's `/api/get` endpoint.
//!
//! All fields are optional — different LRCLIB responses ship
//! different subsets. `plainLyrics` and `syncedLyrics` are mutually
//! inclusive in practice but the parser doesn't enforce it; an
//! entry with neither is treated as "not found" by the caller.

use serde::{Deserialize, Serialize};

/// A single positive LRCLIB match. Empty when the upstream said
/// "no result for this query".
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LrclibResponse {
    /// LRCLIB's internal id. We surface this in `LyricsHit::lrclib_id`
    /// for future "report a correction" links but don't use it for
    /// anything yet.
    #[serde(default)]
    pub id: u64,

    /// Plain-text lyrics, no timestamps. Present on older / less
    /// curated entries.
    #[serde(default)]
    pub plain_lyrics: Option<String>,

    /// LRC-flavoured synced lyrics (`[mm:ss.xx]line`). When present
    /// the frontend's `parseLrc` can drive the line-by-line
    /// highlight.
    #[serde(default)]
    pub synced_lyrics: Option<String>,

    /// LRCLIB flags some tracks as instrumental (no sung lyrics).
    /// The frontend uses this to render an "instrumental" placeholder
    /// instead of the generic "no lyrics" empty state.
    #[serde(default)]
    pub instrumental: bool,

    /// Upstream track name — useful for diagnostics but not used
    /// downstream.
    #[serde(default)]
    pub track_name: Option<String>,
    #[serde(default)]
    pub artist_name: Option<String>,
    #[serde(default)]
    pub album_name: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
}

/// What `LrclibClient::fetch` returns to its callers — a normalised
/// view over `LrclibResponse` that drops LRCLIB-specific fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricsHit {
    /// Plain-text lyrics, if LRCLIB had any.
    pub plain: Option<String>,
    /// Synced LRC lyrics, if LRCLIB had any.
    pub synced: Option<String>,
    /// True when LRCLIB flagged the track as instrumental.
    pub instrumental: bool,
    /// LRCLIB's id for this entry. `None` when the response was
    /// empty.
    pub lrclib_id: Option<u64>,
}

impl LrclibResponse {
    /// Reduce a parsed response to the `LyricsHit` the rest of the
    /// app consumes. Returns `None` for an empty response (no
    /// lyrics at all and not flagged instrumental).
    pub fn into_hit(self) -> Option<LyricsHit> {
        if self.plain_lyrics.is_none()
            && self.synced_lyrics.is_none()
            && !self.instrumental
        {
            return None;
        }
        Some(LyricsHit {
            plain: self.plain_lyrics.filter(|s| !s.trim().is_empty()),
            synced: self.synced_lyrics.filter(|s| !s.trim().is_empty()),
            instrumental: self.instrumental,
            lrclib_id: if self.id == 0 {
                None
            } else {
                Some(self.id)
            },
        })
    }
}
