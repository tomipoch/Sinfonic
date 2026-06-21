//! Auth flow for Jellyfin.
//!
//! `POST /Users/AuthenticateByName` returns the access token and the
//! user id we use as the `user_id` in the provider identity. The
//! `server_id` we persist in the SQLite cache is the Jellyfin `Id`
//! from the auth response (a stable UUID) so the row survives the
//! server changing its display name.
//!
//! The token never leaves this crate: callers receive a
//! `JellyfinSession` and are expected to feed it into a
//! `JellyfinProvider`. Token persistence (keyring, file, …) is the
//! caller's responsibility.

use serde::{Deserialize, Serialize};
use sinfonic_domain::ServerId;
use sinfonic_source::{ProviderError, ProviderResult};
use url::Url;

use super::client::{AuthContext, JellyfinClient};
use super::dto::AuthResult;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub device_id: String,
}

/// Result of a successful login. The session carries the data needed
/// to build a `JellyfinProvider`; the `ServerId` is the value the
/// library cache uses to scope all subsequent reads.
#[derive(Clone, Debug)]
pub struct LoginSuccess {
    pub session: super::JellyfinSession,
    pub server_id: ServerId,
}

/// Authenticate against `request.base_url`. Returns the populated
/// `JellyfinSession` on success, or a `ProviderError::Auth` /
/// `Network` / `Server` describing the failure.
pub async fn login(request: LoginRequest) -> ProviderResult<LoginSuccess> {
    let base_url = parse_base_url(&request.base_url)?;
    let client = JellyfinClient::new(base_url.clone())?;

    // The auth header is sent on the login call too; Jellyfin returns
    // an anonymous token otherwise and rejects the device id check.
    let auth = AuthContext {
        device_id: request.device_id.clone(),
        access_token: None,
    };

    #[derive(Serialize)]
    struct Body<'a> {
        #[serde(rename = "Username")]
        username: &'a str,
        #[serde(rename = "Pw")]
        password: &'a str,
    }

    let body = Body {
        username: &request.username,
        password: &request.password,
    };

    let result: AuthResult = client
        .post_json("Users/AuthenticateByName", &auth, &body)
        .await?;

    Ok(LoginSuccess {
        session: super::JellyfinSession {
            server_id: ServerId::new(format!("server-{}", result.server_id)),
            base_url: request.base_url.trim_end_matches('/').to_string(),
            access_token: result.access_token,
            user_id: result.user.id,
            device_id: request.device_id,
        },
        server_id: ServerId::new(format!("server-{}", result.server_id)),
    })
}

fn parse_base_url(raw: &str) -> ProviderResult<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::Auth("base_url is empty".into()));
    }
    Url::parse(trimmed).map_err(|e| ProviderError::Auth(format!("invalid base_url: {e}")))
}