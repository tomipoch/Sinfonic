//! Error types for the LRCLIB client.

use thiserror::Error;

/// Result alias for the LRCLIB client.
pub type LyricsResult<T> = Result<T, LyricsError>;

#[derive(Debug, Error)]
pub enum LyricsError {
    /// LRCLIB returned 404 (or otherwise signalled "no match").
    /// Surfaced as `Ok(None)` from `LrclibClient::fetch`, not
    /// bubbled — this variant exists for internal use only.
    #[error("lrclib: no lyrics for this track")]
    NotFound,

    /// HTTP layer failure (DNS, TLS, timeout, connection reset).
    #[error("lrclib: network error: {0}")]
    Network(#[from] reqwest::Error),

    /// LRCLIB returned a body that didn't parse into the expected
    /// shape. Almost certainly means the upstream schema changed.
    #[error("lrclib: malformed response: {0}")]
    Decode(String),
}
