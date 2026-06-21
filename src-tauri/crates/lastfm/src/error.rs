//! Errors surfaced by the Last.fm client.
//!
//! Variants map the Last.fm `error` codes that matter for Sinfonic:
//! authentication failures, invalid method/parameter, and rate
//! limiting. Everything else falls under `Protocol` (the server
//! responded but with an unexpected shape) or `Network` (transport
//! failure).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LastFmError {
    #[error("last.fm network error: {0}")]
    Network(String),

    #[error("last.fm authentication failed: {0}")]
    Auth(String),

    #[error("last.fm protocol error: {0}")]
    Protocol(String),

    #[error("last.fm rate-limited; retry later")]
    RateLimited,

    #[error("last.fm invalid request: {0}")]
    InvalidRequest(String),

    #[error("last.fm: not authenticated (call authenticate first)")]
    NotAuthenticated,
}

pub type LastFmResult<T> = Result<T, LastFmError>;
