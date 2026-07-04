//! LRCLIB lyrics lookup client.
//!
//! `LrclibClient` queries the public [LRCLIB] service — a free
//! aggregator of plain and LRC-synced lyrics — and returns whichever
//! variant the server has on file. The client is the fallback path
//! for `commands::get_lyrics` when no music provider (Subsonic,
//! Jellyfin, …) ships its own lyrics or when those lyrics are empty.
//!
//! ## Caching
//!
//! Every query hits an internal LRU keyed by
//! `(title, artist, duration)`. Both positive and "not found"
//! results are cached so the user can scrub back and forth in the
//! queue without re-issuing the same HTTP call. The cache does not
//! persist across launches.
//!
//! ## Politeness
//!
//! - `User-Agent: Sinfonic/<version> (https://github.com/tomipoch/Sinfonic)`
//! - 5 s request timeout
//! - No concurrency cap (LRCLIB's rate limit is generous for the
//!   normal "few lyrics per session" use case; we add one if abuse
//!   becomes a problem)
//!
//! [LRCLIB]: https://lrclib.net
//!
//! ## Example
//!
//! ```no_run
//! use sinfonic_lyrics::{LrclibClient, LyricsQuery};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = LrclibClient::new(
//!     "https://lrclib.net".parse()?,
//!     env!("CARGO_PKG_VERSION").to_string(),
//! )?;
//! let q = LyricsQuery {
//!     artist: "Eagles",
//!     title: "Hotel California",
//!     album: Some("Hotel California"),
//!     duration_seconds: Some(390),
//! };
//! if let Some(hit) = client.fetch(&q).await? {
//!     println!("synced={} plain={} instrumental={}",
//!         hit.synced.is_some(),
//!         hit.plain.is_some(),
//!         hit.instrumental);
//! }
//! # Ok(()) }
//! ```

pub mod client;
pub mod dto;
pub mod error;

pub use client::LrclibClient;
pub use dto::LyricsHit;
pub use error::{LyricsError, LyricsResult};

/// Inputs for a single LRCLIB query. All fields are borrowed; the
/// caller (typically `commands::get_lyrics`) owns the underlying
/// strings.
#[derive(Debug, Clone)]
pub struct LyricsQuery<'a> {
    pub artist: &'a str,
    pub title: &'a str,
    pub album: Option<&'a str>,
    pub duration_seconds: Option<u32>,
}
