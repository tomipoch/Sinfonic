//! Subsonic authentication.
//!
//! Subsonic uses a session-less, request-based auth scheme: every call
//! to `?u=&t=&s=&v=&c=&f=json` is signed with a fresh
//! `token = md5(password + salt)`. There's no login/logout step on the
//! server — the credentials are the password itself. The async
//! `ping` we expose here is purely a *validity check* (it calls
//! `/rest/ping` and reports the result).
//!
//! We deliberately don't talk to the server in `login`. The flow is:
//!
//! 1. Caller has `{ base_url, username, password }` (password came
//!    from the user; we never store it long-term, the keyring does).
//! 2. Caller calls `connect(...)` to build a `SubsonicProvider`.
//!    That function does a `ping` to validate the credentials and
//!    discover the server name / type.
//! 3. Subsequent calls to provider methods sign every request with
//!    a fresh salt via `sign_request()`.
//!
//! The token is regenerated on every call so a leak of one request
//! body cannot replay against the server.

use md5::Context;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sinfonic_domain::ServerId;
use sinfonic_source::{ProviderError, ProviderResult};
use url::Url;

use super::client::{SubsonicClient, SUBSONIC_CLIENT_NAME};
use super::dto::PingResponse;

const SUBSONIC_LOGIN_PATH: &str = "rest/ping";
const SUBSONIC_LOGIN_VIEW: &str = "rest/ping.view";
// The default Subsonic JSON envelope version. Newer servers also
// support `1.16.1`; we pick `1.16.1` because Navidrome, Funkwhale
// and modern Airsonic-derivatives all understand it and it adds
// `serverType` in `getServerInfo`.
pub(super) const SUBSONIC_API_VERSION_PING: &str = "1.16.1";

/// Salt + token pair sent on every authenticated request. Always
/// freshly generated — never reused across calls.
#[derive(Clone, Debug)]
pub struct AuthParams {
    pub username: String,
    pub salt: String,
    pub token: String,
}

/// Login request — what the Tauri command receives from the frontend.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

/// Result of a successful handshake. Carries the data needed to
/// build a `SubsonicProvider` and the `ServerId` the library cache
/// uses to scope subsequent reads.
#[derive(Clone, Debug)]
pub struct LoginSuccess {
    pub session: super::SubsonicSession,
    pub server_id: ServerId,
    pub server_name: String,
    pub server_type: String,
}

/// Cheap-to-clone session: the password is `String` (small) and the
/// rest is metadata.
#[derive(Clone, Debug)]
pub struct SubsonicSession {
    pub server_id: ServerId,
    pub base_url: String,
    pub username: String,
    pub password: String,
}

impl SubsonicSession {
    /// Build the auth params for a single request. Salt is a fresh
    /// 16-char random alphanumeric string; token is `md5(p + s)`
    /// rendered as lowercase hex.
    pub fn sign(&self) -> AuthParams {
        let salt: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let token = md5_hex(format!("{}{}", self.password, salt).as_bytes());
        AuthParams {
            username: self.username.clone(),
            salt,
            token,
        }
    }
}

/// Hex-encoded MD5 of the input bytes. Used for the Subsonic auth
/// token. Lowercase to match the convention the protocol expects.
pub fn md5_hex(input: &[u8]) -> String {
    let mut hasher = Context::new();
    hasher.consume(input);
    let digest = hasher.compute();
    hex::encode(digest.0)
}

/// Validate the credentials by pinging the server. The ping response
/// includes `subsonic-response.serverName` and
/// `subsonic-response.type` ("navidrome", "opensubsonic", "airsonic",
/// …) which the library cache uses to display a friendly name.
///
/// `version` is the Subsonic API version we advertise in the
/// request — default `1.16.1` for parity with Navidrome; older
/// servers may need `1.15.0`.
pub async fn ping(
    request: &LoginRequest,
    version: Option<&str>,
) -> ProviderResult<PingResponse> {
    let base_url = parse_base_url(&request.base_url)?;
    let client = SubsonicClient::new(base_url)?;
    let session = SubsonicSession {
        server_id: ServerId::new("server-pending"),
        base_url: request.base_url.trim_end_matches('/').to_string(),
        username: request.username.clone(),
        password: request.password.clone(),
    };
    let auth = session.sign();
    let version = version.unwrap_or(SUBSONIC_API_VERSION_PING);
    let response: PingResponse = client
        .get_json(
            SUBSONIC_LOGIN_PATH,
            &auth,
            version,
            [
                ("f", "json".to_string()),
                ("c", SUBSONIC_CLIENT_NAME.to_string()),
                ("v", version.to_string()),
            ],
        )
        .await?;
    Ok(response)
}

pub async fn login(request: LoginRequest) -> ProviderResult<LoginSuccess> {
    let response = ping(&request, None).await?;
    // server_id is derived from the server name + address so the
    // library cache can scope its rows. We re-derive it later from
    // the actual server id (e.g. UUID) once the user has confirmed
    // they're connecting to the right host. The placeholder used
    // here is overwritten by the provider.
    let server_id = ServerId::new(format!("server-subsonic-{}", slugify(&response.server_name)));
    Ok(LoginSuccess {
        session: SubsonicSession {
            server_id: server_id.clone(),
            base_url: request.base_url.trim_end_matches('/').to_string(),
            username: request.username,
            password: request.password,
        },
        server_id,
        server_name: response.server_name,
        server_type: response.server_type,
    })
}

fn parse_base_url(raw: &str) -> ProviderResult<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::Auth("base_url is empty".into()));
    }
    Url::parse(trimmed).map_err(|e| ProviderError::Auth(format!("invalid base_url: {e}")))
}

/// Lowercase, alphanumeric-only derivation used to make a stable
/// `server-id` from a server name. Delegates to
/// `sinfonic_source::slugify` so the slug rules stay in lockstep
/// with the ones used in `lib.rs::tracks()` for genre ids.
fn slugify(name: &str) -> String {
    sinfonic_source::slugify(name)
}

// Kept referenced so the import isn't flagged as unused when the
// path is changed in the future.
#[allow(dead_code)]
const _UNUSED: (&str, &str) = (SUBSONIC_LOGIN_PATH, SUBSONIC_LOGIN_VIEW);
