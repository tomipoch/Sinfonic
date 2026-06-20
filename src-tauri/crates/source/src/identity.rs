//! `ProviderIdentity` — human-readable info about the server/source.

use serde::{Deserialize, Serialize};

use sinfonic_domain::ServerId;

/// Alias used by provider impls (`Identity` reads cleaner than
/// `ProviderIdentity` in trait bounds).
pub type Identity = ProviderIdentity;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderIdentity {
    pub provider_id: String,
    pub server_id: ServerId,
    pub server_name: String,
    pub user_id: String,
    pub username: String,
}
