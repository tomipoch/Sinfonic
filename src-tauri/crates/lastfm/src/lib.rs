//! Last.fm / Libre.fm scrobble client.
//!
//! Implements the [audioscrobbler 2.0] REST API — the same protocol
//! Last.fm and Libre.fm speak. The crate is deliberately small
//! (~300 LOC + tests) so it can be shared across future Sinfonic
//! front-ends without dragging in a generic HTTP abstraction.
//!
//! ## Auth
//!
//! 1. The user enters `api_key`, `api_secret`, `username`, and a
//!    plaintext `password` in Settings.
//! 2. We md5-hash the password and POST
//!    `method=auth.getMobileSession` with the api signature.
//! 3. Last.fm returns a session key we persist in the OS keyring
//!    under `SecretKey::LastFmSession`. The api key + secret stay in
//!    `SecretKey::LastFmApiSecret` as a JSON blob.
//!
//! ## Scrobble rules
//!
//! - `now_playing` is sent the moment a track starts playing.
//! - `scrobble` is sent when the playhead crosses the 50 % mark
//!   OR the track ends (whichever fires first). The Tauri layer
//!   enforces the dedupe — this client will happily double-scrobble
//!   if asked to twice.
//!
//! [audioscrobbler 2.0]: https://www.last.fm/api/show/auth.getMobileSession

pub mod auth;
pub mod client;
pub mod error;
pub mod signature;
pub mod track;

pub use auth::LastFmCredentials;
pub use client::LastFmClient;
pub use error::{LastFmError, LastFmResult};
pub use track::{Scrobble, ScrobbleSource};
