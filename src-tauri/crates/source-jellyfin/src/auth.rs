//! Auth flow for Jellyfin.
//!
//! Phase 0: stub. Real impl in Phase 3 uses `POST /Users/AuthenticateByName`.

use serde::{Deserialize, Serialize};
use sinfonic_source::ProviderResult;

use super::JellyfinSession;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub device_id: String,
    pub device_name: String,
}

pub async fn login(_request: LoginRequest) -> ProviderResult<JellyfinSession> {
    Err(sinfonic_source::ProviderError::Other(
        "auth flow not implemented in skeleton".into(),
    ))
}
