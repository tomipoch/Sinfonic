//! Last.fm / Libre.fm REST client.
//!
//! Stateless beyond the session key: each call re-builds the api
//! signature from scratch. The endpoint defaults to Last.fm but
//! can be pointed at any audioscrobbler-compatible service (e.g.
//! `https://libre.fm/2.0/`) via `with_endpoint`.
//!
//! All API methods are async because the underlying `reqwest::Client`
//! is async; tests use `tokio::runtime::Runtime` or `wiremock` to
//! drive them.

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use url::Url;

use crate::auth::LastFmCredentials;
use crate::error::{LastFmError, LastFmResult};
use crate::signature::sign;
use crate::track::{Scrobble, ScrobbleSource};

const LASTFM_ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Known error codes that need a richer mapping than "Protocol".
///
/// Reference: <https://www.last.fm/api/errorcodes>.
const ERROR_AUTH_TOKEN: i64 = 9;
const ERROR_AUTH_INVALID: i64 = 4;
const ERROR_AUTH_UNAUTHORISED: i64 = 14;
const ERROR_INVALID_PARAMS: i64 = 13;
const ERROR_INVALID_METHOD: i64 = 3;
const ERROR_RATE_LIMITED: i64 = 29;

#[derive(Clone)]
pub struct LastFmClient {
    endpoint: Url,
    api_key: String,
    api_secret: String,
    session_key: Option<String>,
    http: Client,
}

impl LastFmClient {
    /// Build a client with the Last.fm endpoint. Call `authenticate`
    /// before any authenticated method.
    pub fn new(api_key: String, api_secret: String) -> LastFmResult<Self> {
        Self::with_endpoint(LASTFM_ENDPOINT, api_key, api_secret)
    }

    /// Build a client pointed at a custom audioscrobbler endpoint
    /// (e.g. Libre.fm).
    pub fn with_endpoint(
        endpoint: &str,
        api_key: String,
        api_secret: String,
    ) -> LastFmResult<Self> {
        let endpoint = Url::parse(endpoint)
            .map_err(|e| LastFmError::InvalidRequest(format!("endpoint: {e}")))?;
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent("Sinfonic/0.1 (+https://github.com/tomipoch/Sinfonic)")
            .build()
            .map_err(|e| LastFmError::Network(e.to_string()))?;
        Ok(Self {
            endpoint,
            api_key,
            api_secret,
            session_key: None,
            http,
        })
    }

    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    pub fn is_authenticated(&self) -> bool {
        self.session_key.is_some()
    }

    /// Exchange credentials for a session key via
    /// `auth.getMobileSession`. On success the key is cached in
    /// memory; callers should persist it to the OS keyring for next
    /// launch.
    pub async fn authenticate(
        &mut self,
        creds: &LastFmCredentials,
    ) -> LastFmResult<String> {
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("method", "auth.getMobileSession".into());
        params.insert("api_key", creds.api_key.clone());
        params.insert("username", creds.username.clone());
        params.insert("password", creds.password_md5.clone());
        let sig = sign(params.iter().map(|(k, v)| (*k, v.as_str())), &creds.api_secret);

        let mut form: Vec<(&str, &str)> = Vec::with_capacity(params.len() + 2);
        for (k, v) in &params {
            form.push((*k, v.as_str()));
        }
        form.push(("api_sig", sig.as_str()));
        form.push(("format", "json"));

        let response: AuthResponse = self.post_form(&form).await?;
        let session = response.session.key;
        self.session_key = Some(session.clone());
        Ok(session)
    }

    /// Re-attach a previously-issued session key (loaded from the
    /// keyring on startup). Returns an error if the server rejects
    /// it — in that case the caller should re-prompt the user.
    pub async fn resume(&mut self, session_key: String) -> LastFmResult<()> {
        // A cheap `user.getInfo` proves the session is still alive.
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("method", "user.getInfo".into());
        params.insert("api_key", self.api_key.clone());
        params.insert("sk", session_key.clone());
        let sig = sign(
            params.iter().map(|(k, v)| (*k, v.as_str())),
            &self.api_secret,
        );
        let mut form: Vec<(&str, &str)> = Vec::with_capacity(params.len() + 2);
        for (k, v) in &params {
            form.push((*k, v.as_str()));
        }
        form.push(("api_sig", sig.as_str()));
        form.push(("format", "json"));

        let _: serde_json::Value = self.post_form(&form).await?;
        self.session_key = Some(session_key);
        Ok(())
    }

    pub fn logout(&mut self) {
        self.session_key = None;
    }

    /// Test-only constructor that pre-attaches a session key,
    /// bypassing the auth handshake. Used by integration tests that
    /// only want to exercise `scrobble` / `now_playing`.
    #[doc(hidden)]
    pub fn with_session_for_tests(
        endpoint: &str,
        api_key: String,
        api_secret: String,
        session_key: String,
    ) -> LastFmResult<Self> {
        let mut client = Self::with_endpoint(endpoint, api_key, api_secret)?;
        client.session_key = Some(session_key);
        Ok(client)
    }

    /// `track.updateNowPlaying` — fire-and-forget signal that the
    /// given track is currently playing. Last.fm uses it to power
    /// the "Friends" tab.
    pub async fn now_playing(
        &self,
        scrobble: &Scrobble,
        source: ScrobbleSource,
    ) -> LastFmResult<()> {
        let session = self
            .session_key
            .as_ref()
            .ok_or(LastFmError::NotAuthenticated)?;
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("method", "track.updateNowPlaying".into());
        params.insert("api_key", self.api_key.clone());
        params.insert("sk", session.clone());
        params.insert("artist", scrobble.artist.clone());
        params.insert("track", scrobble.track.clone());
        if let Some(album) = &scrobble.album {
            params.insert("album", album.clone());
        }
        if let Some(d) = scrobble.duration_seconds {
            params.insert("duration", d.to_string());
        }
        if let Some(mbid) = &scrobble.mbid {
            params.insert("mbid", mbid.clone());
        }
        params.insert("source", source.as_str().to_string());

        let sig = sign(params.iter().map(|(k, v)| (*k, v.as_str())), &self.api_secret);
        let mut form: Vec<(&str, &str)> = Vec::with_capacity(params.len() + 2);
        for (k, v) in &params {
            form.push((*k, v.as_str()));
        }
        form.push(("api_sig", sig.as_str()));
        form.push(("format", "json"));

        let _: serde_json::Value = self.post_form(&form).await?;
        Ok(())
    }

    /// `track.scrobble` — log a single scrobble. Returns the
    /// `accepted` flag from the server response.
    pub async fn scrobble(
        &self,
        scrobble: &Scrobble,
        source: ScrobbleSource,
    ) -> LastFmResult<bool> {
        let session = self
            .session_key
            .as_ref()
            .ok_or(LastFmError::NotAuthenticated)?;
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("method", "track.scrobble".into());
        params.insert("api_key", self.api_key.clone());
        params.insert("sk", session.clone());
        params.insert("artist", scrobble.artist.clone());
        params.insert("track", scrobble.track.clone());
        params.insert("timestamp", scrobble.timestamp_unix.to_string());
        if let Some(album) = &scrobble.album {
            params.insert("album", album.clone());
        }
        if let Some(d) = scrobble.duration_seconds {
            params.insert("duration", d.to_string());
        }
        if let Some(mbid) = &scrobble.mbid {
            params.insert("mbid", mbid.clone());
        }
        params.insert("source", source.as_str().to_string());

        let sig = sign(params.iter().map(|(k, v)| (*k, v.as_str())), &self.api_secret);
        let mut form: Vec<(&str, &str)> = Vec::with_capacity(params.len() + 2);
        for (k, v) in &params {
            form.push((*k, v.as_str()));
        }
        form.push(("api_sig", sig.as_str()));
        form.push(("format", "json"));

        let resp: ScrobbleResponse = self.post_form(&form).await?;
        Ok(resp.scrobbles.attr.accepted > 0)
    }

    async fn post_form<T: for<'de> Deserialize<'de>>(
        &self,
        form: &[(&str, &str)],
    ) -> LastFmResult<T> {
        let resp = self
            .http
            .post(self.endpoint.clone())
            .form(form)
            .send()
            .await
            .map_err(|e| LastFmError::Network(e.to_string()))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| LastFmError::Network(format!("read body: {e}")))?;
        if !status.is_success() {
            return Err(LastFmError::Protocol(format!(
                "HTTP {status}: {}",
                truncate(&body, 200)
            )));
        }
        // Last.fm error responses look like `{"error": 14, "message": "..."}`
        // (top-level integer code, NOT an object). Detect that first so a
        // generic success-shaped body never gets fed to the error mapper.
        if let Some(err_num) = peek_error_code(&body) {
            let message = peek_error_message(&body).unwrap_or_default();
            return Err(map_error_code(err_num, message));
        }
        serde_json::from_str(&body).map_err(|e| {
            LastFmError::Protocol(format!(
                "invalid json: {e}; body={}",
                truncate(&body, 200)
            ))
        })
    }
}

fn peek_error_code(body: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("error").and_then(|x| x.as_i64())
}

fn peek_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("message")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    session: AuthSession,
}

#[derive(Debug, Deserialize)]
struct AuthSession {
    key: String,
}

#[derive(Debug, Deserialize)]
struct ScrobbleResponse {
    scrobbles: ScrobblesAttrs,
}

#[derive(Debug, Deserialize)]
struct ScrobblesAttrs {
    #[serde(rename = "@attr")]
    attr: ScrobbleAttr,
}

#[derive(Debug, Deserialize)]
struct ScrobbleAttr {
    accepted: u32,
}

fn map_error_code(code: i64, message: String) -> LastFmError {
    match code {
        ERROR_AUTH_TOKEN | ERROR_AUTH_INVALID | ERROR_AUTH_UNAUTHORISED => {
            LastFmError::Auth(message)
        }
        ERROR_INVALID_METHOD | ERROR_INVALID_PARAMS => LastFmError::InvalidRequest(message),
        ERROR_RATE_LIMITED => LastFmError::RateLimited,
        _ => LastFmError::Protocol(format!("code {code}: {message}")),
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Find the largest char boundary at or before `max`.
        let mut idx = max;
        while idx > 0 && !s.is_char_boundary(idx) {
            idx -= 1;
        }
        &s[..idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "hel");
        // Multi-byte boundary — shouldn't panic.
        assert_eq!(truncate("héllo", 3), "hé");
    }

    #[test]
    fn map_error_code_routes_known_codes() {
        assert!(matches!(
            map_error_code(ERROR_AUTH_TOKEN, "x".into()),
            LastFmError::Auth(_)
        ));
        assert!(matches!(
            map_error_code(ERROR_RATE_LIMITED, "x".into()),
            LastFmError::RateLimited
        ));
        assert!(matches!(
            map_error_code(9999, "x".into()),
            LastFmError::Protocol(_)
        ));
    }
}
