//! HTTP client for Jellyfin.
//!
//! Wraps `reqwest::Client` with:
//!
//! - The fixed `X-Emby-Authorization: MediaBrowser Client=...` header
//!   that Jellyfin expects on every authenticated request.
//! - Conservative connect / request / read timeouts so a slow server
//!   can't pin a Tauri command forever.
//! - Strict body-size limits so a misconfigured server can't blow our
//!   memory.
//! - JSON request/response helpers that convert network and HTTP
//!   failures into the shared `ProviderError` so callers never see a
//!   raw `reqwest::Error`.
//!
//! The client is cheap to clone (reqwest pools internally) and is
//! shared by every `MusicProvider` method via `JellyfinProvider`.

use std::time::Duration;

use reqwest::{Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use sinfonic_source::ProviderError;

pub(super) const JELLYFIN_DISCOVERY_PORT: u16 = 7359;
pub(super) const JELLYFIN_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const JELLYFIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub(super) const JELLYFIN_JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(super) const JELLYFIN_IMAGE_MAX_BYTES: usize = 32 * 1024 * 1024;

/// Stable client identity used in the `X-Emby-Authorization` header.
/// Jellyfin tracks devices by these four values; rotating them logs
/// the user out everywhere else, so they're hardcoded for v0.1.
pub(super) const CLIENT_NAME: &str = "Sinfonic";
pub(super) const CLIENT_VERSION: &str = "0.1.0";
pub(super) const DEVICE_NAME: &str = "Sinfonic Desktop";

/// Per-request auth context. Cheap to clone; passed to helpers.
#[derive(Clone, Debug)]
pub(super) struct AuthContext {
    pub device_id: String,
    pub access_token: Option<String>,
}

impl AuthContext {
    /// Build the value of the `X-Emby-Authorization` header for the
    /// current request. Jellyfin expects a single comma-separated
    /// line of `key="value"` pairs; reqwest requires a string.
    pub fn header_value(&self) -> String {
        let mut parts = format!(
            "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
            CLIENT_NAME, DEVICE_NAME, self.device_id, CLIENT_VERSION,
        );
        if let Some(token) = &self.access_token {
            parts.push_str(&format!(", Token=\"{token}\""));
        }
        parts
    }
}

/// The HTTP client. Cloneable (reqwest is `Arc`-backed) and stored
/// once per `JellyfinProvider`.
#[derive(Clone)]
pub(super) struct JellyfinClient {
    http: Client,
    base_url: Url,
}

impl JellyfinClient {
    /// Build a client pointing at `base_url`. Fails if the URL is
    /// invalid or `reqwest` cannot be initialised (no TLS backend,
    /// missing system proxy, …).
    pub fn new(base_url: Url) -> Result<Self, ProviderError> {
        let http = Client::builder()
            .connect_timeout(JELLYFIN_CONNECT_TIMEOUT)
            .timeout(JELLYFIN_REQUEST_TIMEOUT)
            .user_agent(format!("{CLIENT_NAME}/{CLIENT_VERSION}"))
            .build()
            .map_err(|e| ProviderError::Network(format!("reqwest build failed: {e}")))?;
        Ok(Self { http, base_url })
    }

    /// Borrow the base URL the client was built for (exposed for tests).
#[allow(dead_code)]
pub(super) fn base_url(client: &JellyfinClient) -> &Url {
    &client.base_url
}

    /// Run `GET {path}` and decode the JSON body into `T`.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        auth: &AuthContext,
    ) -> Result<T, ProviderError> {
        let body: Option<&()> = None;
        self.send_json::<(), T>(Method::GET, path, auth, body).await
    }

    /// Run `POST {path}` with a JSON body and decode the response
    /// into `T`.
    pub async fn post_json<B, T>(
        &self,
        path: &str,
        auth: &AuthContext,
        body: &B,
    ) -> Result<T, ProviderError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        self.send_json(Method::POST, path, auth, Some(body)).await
    }

    /// Run `DELETE {path}` and ignore the body. Jellyfin returns 204
    /// for most mutating endpoints; we tolerate any 2xx.
    pub async fn delete(&self, path: &str, auth: &AuthContext) -> Result<(), ProviderError> {
        let url = self.url(path)?;
        let resp = self
            .http
            .request(Method::DELETE, url)
            .header("X-Emby-Authorization", auth.header_value())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = check_status(resp, path).await?;
        // Drain so the connection can be reused.
        let _ = resp.bytes().await;
        Ok(())
    }

    /// `DELETE` with a JSON body — Jellyfin's playlist-entry removal
    /// requires this (`DELETE /Playlists/{id}/Items` with
    /// `{ "EntryIds": [...] }`).
    pub async fn delete_with_body<B: Serialize>(
        &self,
        path: &str,
        auth: &AuthContext,
        body: &B,
    ) -> Result<(), ProviderError> {
        let url = self.url(path)?;
        let resp = self
            .http
            .request(Method::DELETE, url)
            .header("X-Emby-Authorization", auth.header_value())
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = check_status(resp, path).await?;
        let _ = resp.bytes().await;
        Ok(())
    }

    /// Run `GET {path}` and return raw bytes (for image fetches).
    /// Enforces `JELLYFIN_IMAGE_MAX_BYTES` so a malicious server can't
    /// exhaust memory.
    pub async fn get_bytes(
        &self,
        path: &str,
        auth: &AuthContext,
    ) -> Result<Vec<u8>, ProviderError> {
        let url = self.url(path)?;
        let resp = self
            .http
            .get(url)
            .header("X-Emby-Authorization", auth.header_value())
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = check_status(resp, path).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ProviderError::Network(format!("read body: {e}")))?;
        if bytes.len() > JELLYFIN_IMAGE_MAX_BYTES {
            return Err(ProviderError::Other(format!(
                "image too large: {} bytes",
                bytes.len()
            )));
        }
        Ok(bytes.to_vec())
    }

    async fn send_json<B, T>(
        &self,
        method: Method,
        path: &str,
        auth: &AuthContext,
        body: Option<&B>,
    ) -> Result<T, ProviderError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let url = self.url(path)?;
        let mut req = self
            .http
            .request(method, url)
            .header("X-Emby-Authorization", auth.header_value())
            .header("Accept", "application/json");
        if let Some(b) = body {
            req = req.header("Content-Type", "application/json").json(b);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = check_status(resp, path).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ProviderError::Network(format!("read body: {e}")))?;
        if bytes.len() > JELLYFIN_JSON_MAX_BYTES {
            return Err(ProviderError::Other(format!(
                "response too large: {} bytes",
                bytes.len()
            )));
        }
        serde_json::from_slice(&bytes).map_err(|e| {
            ProviderError::Other(format!("decode response: {e} (body: {})", preview(&bytes)))
        })
    }

    fn url(&self, path: &str) -> Result<Url, ProviderError> {
        let path = path.trim_start_matches('/');
        self.base_url
            .join(path)
            .map_err(|e| ProviderError::Other(format!("invalid url {path}: {e}")))
    }
}

async fn check_status(resp: Response, path: &str) -> Result<Response, ProviderError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let message = resp
        .text()
        .await
        .unwrap_or_else(|_| String::from("(no body)"));
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            Err(ProviderError::Auth(format!("{status} on {path}: {message}")))
        }
        StatusCode::NOT_FOUND => Err(ProviderError::NotFound),
        _ => Err(ProviderError::Server {
            status: status.as_u16(),
            message: format!("{path}: {message}"),
        }),
    }
}

fn preview(bytes: &[u8]) -> String {
    let take = bytes.len().min(200);
    String::from_utf8_lossy(&bytes[..take]).into_owned()
}