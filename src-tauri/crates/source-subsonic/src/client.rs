//! HTTP client for Subsonic.
//!
//! Wraps `reqwest::Client` with:
//!
//! - Auth params (`?u=…&t=…&s=…&v=…&c=…&f=json`) injected on every
//!   request. Salt and token are passed per call (regenerated each
//!   time by `SubsonicSession::sign`).
//! - Conservative connect / request / read timeouts so a slow server
//!   can't pin a Tauri command forever.
//! - JSON request/response helpers that translate network and HTTP
//!   failures into the shared `ProviderError` so callers never see a
//!   raw `reqwest::Error`.
//! - Status-code check that handles Subsonic's two-layer error
//!   protocol: HTTP 200 with `subsonic-response.status == "failed"`
//!   (the `error.code` field is the protocol code, e.g. 10 for
//!   bad credentials). HTTP non-200 means the server is broken at
//!   the transport level.
//!
//! The client is cheap to clone (reqwest pools internally) and is
//! shared by every `MusicProvider` method via `SubsonicProvider`.

use std::time::Duration;

use reqwest::{Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use url::Url;

use sinfonic_source::ProviderError;

use super::auth::AuthParams;
use super::dto::SubsonicEnvelope;

pub(super) const SUBSONIC_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const SUBSONIC_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub(super) const SUBSONIC_JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(super) const SUBSONIC_IMAGE_MAX_BYTES: usize = 32 * 1024 * 1024;
pub(super) const SUBSONIC_API_VERSION: &str = "1.16.1";
pub(super) const SUBSONIC_CLIENT_NAME: &str = "Sinfonic";

/// The HTTP client. Cloneable (reqwest is `Arc`-backed) and stored
/// once per `SubsonicProvider`.
#[derive(Clone)]
pub(super) struct SubsonicClient {
    http: Client,
    base_url: Url,
}

impl SubsonicClient {
    /// Build a client pointing at `base_url`. Fails if the URL is
    /// invalid or `reqwest` cannot be initialised (no TLS backend,
    /// missing system proxy, …).
    pub fn new(base_url: Url) -> Result<Self, ProviderError> {
        let http = Client::builder()
            .connect_timeout(SUBSONIC_CONNECT_TIMEOUT)
            .timeout(SUBSONIC_REQUEST_TIMEOUT)
            .user_agent(format!("{SUBSONIC_CLIENT_NAME}/0.1.0"))
            .pool_max_idle_per_host(16)
            .tcp_nodelay(true)
            .build()
            .map_err(|e| ProviderError::Network(format!("reqwest build failed: {e}")))?;
        Ok(Self { http, base_url })
    }

    /// Borrow the base URL the client was built for (used by tests).
    #[allow(dead_code)]
    pub(super) fn base_url(client: &SubsonicClient) -> &Url {
        &client.base_url
    }

    /// Run a request to `{path}` and decode the JSON body into `T`.
    /// The request is signed with the auth params and the Subsonic
    /// `?f=json` envelope is unwrapped automatically.
    pub async fn get_json<T, P>(
        &self,
        path: &str,
        auth: &AuthParams,
        version: &str,
        extra: P,
    ) -> Result<T, ProviderError>
    where
        T: DeserializeOwned,
        P: IntoIterator<Item = (&'static str, String)>,
    {
        let url = self.url(path, auth, version, extra)?;
        let resp = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = check_status(resp, path).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ProviderError::Network(format!("read body: {e}")))?;
        if bytes.len() > SUBSONIC_JSON_MAX_BYTES {
            return Err(ProviderError::Other(format!(
                "response too large: {} bytes",
                bytes.len()
            )));
        }
        let envelope: SubsonicEnvelope<T> = serde_json::from_slice(&bytes).map_err(|e| {
            ProviderError::Other(format!(
                "decode response: {e} (body: {})",
                preview(&bytes)
            ))
        })?;
        envelope.into_result(path)
    }

    /// POST a Subsonic request with an empty body. Subsonic uses
    /// query params, not request bodies, for almost every mutating
    /// endpoint. This is a thin wrapper that mirrors the GET helper
    /// but issues a `POST` so the server knows the request is
    /// mutating.
    pub async fn post_json<T, P>(
        &self,
        path: &str,
        auth: &AuthParams,
        version: &str,
        extra: P,
    ) -> Result<T, ProviderError>
    where
        T: DeserializeOwned,
        P: IntoIterator<Item = (&'static str, String)>,
    {
        let url = self.url(path, auth, version, extra)?;
        let resp = self
            .http
            .request(Method::POST, url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = check_status(resp, path).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ProviderError::Network(format!("read body: {e}")))?;
        if bytes.len() > SUBSONIC_JSON_MAX_BYTES {
            return Err(ProviderError::Other(format!(
                "response too large: {} bytes",
                bytes.len()
            )));
        }
        let envelope: SubsonicEnvelope<T> = serde_json::from_slice(&bytes).map_err(|e| {
            ProviderError::Other(format!(
                "decode response: {e} (body: {})",
                preview(&bytes)
            ))
        })?;
        envelope.into_result(path)
    }

    /// GET a binary payload (for `getCoverArt` and image fetches).
    /// Enforces `SUBSONIC_IMAGE_MAX_BYTES` so a malicious server
    /// can't exhaust memory.
    pub async fn get_bytes(
        &self,
        path: &str,
        auth: &AuthParams,
        version: &str,
    ) -> Result<(Vec<u8>, Option<String>), ProviderError> {
        let url = self.url(path, auth, version, std::iter::empty::<(&'static str, String)>())?;
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());
        let resp = check_status(resp, path).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ProviderError::Network(format!("read body: {e}")))?;
        if bytes.len() > SUBSONIC_IMAGE_MAX_BYTES {
            return Err(ProviderError::Other(format!(
                "image too large: {} bytes",
                bytes.len()
            )));
        }
        Ok((bytes.to_vec(), content_type))
    }

    /// Build a signed query string. The path is appended to the
    /// base URL, and the auth + version + `f=json` params are
    /// injected. Extra params (e.g. `id=42`, `count=10`) are
    /// appended verbatim.
    fn url<P>(
        &self,
        path: &str,
        auth: &AuthParams,
        version: &str,
        extra: P,
    ) -> Result<Url, ProviderError>
    where
        P: IntoIterator<Item = (&'static str, String)>,
    {
        let path = path.trim_start_matches('/');
        let mut url = self
            .base_url
            .join(path)
            .map_err(|e| ProviderError::Other(format!("invalid url {path}: {e}")))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("u", &auth.username)
                .append_pair("t", &auth.token)
                .append_pair("s", &auth.salt)
                .append_pair("v", version)
                .append_pair("c", SUBSONIC_CLIENT_NAME)
                .append_pair("f", "json");
            for (k, v) in extra {
                qp.append_pair(k, v.as_ref());
            }
        }
        Ok(url)
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
