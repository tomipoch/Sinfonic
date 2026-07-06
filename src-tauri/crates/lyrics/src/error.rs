//! Error types for the LRCLIB client.

use thiserror::Error;

/// Result alias for the LRCLIB client.
pub type LyricsResult<T> = Result<T, LyricsError>;

#[derive(Debug, Error)]
pub enum LyricsError {
    /// HTTP layer failure (DNS, TLS, timeout, connection reset).
    #[error("lrclib: network error: {0}")]
    Network(#[from] reqwest::Error),

    /// LRCLIB returned a body that didn't parse into the expected
    /// shape. Almost certainly means the upstream schema changed.
    #[error("lrclib: malformed response: {0}")]
    Decode(String),
}
