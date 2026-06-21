//! Last.fm credentials handed to `LastFmClient::authenticate`.
//!
//! `password_md5` is the hex MD5 of the user's plaintext password
//! — Last.fm's mobile handshake wants the hash, not the cleartext.
//! The frontend should hash it in Rust (via the `md5` crate) before
//! passing it across the IPC boundary so the cleartext never leaves
//! the JS process unless the user types it there.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LastFmCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub username: String,
    /// Lower-case hex MD5 of the plaintext password.
    pub password_md5: String,
}
