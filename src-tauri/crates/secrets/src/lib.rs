//! Cross-platform credential storage.
//!
//! Backends:
//! - macOS:   Keychain via `keyring`
//! - Windows: Credential Manager via `keyring`
//! - Linux:   Secret Service via `keyring`
//!
//! Phase 0: trait + KeyringStore impl. Tests cover the basic round-trip
//! (save / load / delete). Caching wrapper lands in Phase 1.

#![allow(dead_code)]

use async_trait::async_trait;
use sinfonic_domain::ServerId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("keyring backend failed: {0}")]
    Backend(String),
    #[error("secret not found")]
    NotFound,
}

pub type SecretResult<T> = Result<T, SecretError>;

/// What kind of secret we're storing. Re-export of the variant set used
/// across crates so we have a single source of truth.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SecretKey {
    ProviderToken(ServerId),
    LastFmApiSecret,
    LastFmSession,
    LibreFmSession,
    ListenBrainzToken,
}

impl SecretKey {
    pub fn namespace(&self) -> &'static str {
        match self {
            Self::ProviderToken(_) => "provider",
            Self::LastFmApiSecret | Self::LastFmSession => "lastfm",
            Self::LibreFmSession => "librefm",
            Self::ListenBrainzToken => "listenbrainz",
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::ProviderToken(_) => "provider-token",
            Self::LastFmApiSecret => "lastfm-api-secret",
            Self::LastFmSession => "lastfm-session",
            Self::LibreFmSession => "librefm-session",
            Self::ListenBrainzToken => "listenbrainz-token",
        }
    }
}

#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn save_secret(&self, key: SecretKey, secret: String) -> SecretResult<()>;
    async fn load_secret(&self, key: SecretKey) -> SecretResult<Option<String>>;
    async fn delete_secret(&self, key: SecretKey) -> SecretResult<()>;

    async fn save_token(&self, server_id: ServerId, token: String) -> SecretResult<()> {
        self.save_secret(SecretKey::ProviderToken(server_id), token)
            .await
    }

    async fn load_token(&self, server_id: ServerId) -> SecretResult<Option<String>> {
        self.load_secret(SecretKey::ProviderToken(server_id)).await
    }

    async fn delete_token(&self, server_id: ServerId) -> SecretResult<()> {
        self.delete_secret(SecretKey::ProviderToken(server_id)).await
    }
}

/// `keyring`-backed implementation. Tries to talk to the OS keychain /
/// Credential Manager / Secret Service. Returns
/// `SecretError::Backend` if the backend is unavailable.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

#[async_trait]
impl SecretStore for KeyringStore {
    async fn save_secret(&self, key: SecretKey, secret: String) -> SecretResult<()> {
        let entry = keyring::Entry::new(&self.service, &format!("{}:{}", key.namespace(), key.kind()))
            .map_err(|e| SecretError::Backend(e.to_string()))?;
        entry
            .set_password(&secret)
            .map_err(|e| SecretError::Backend(e.to_string()))
    }

    async fn load_secret(&self, key: SecretKey) -> SecretResult<Option<String>> {
        let entry = keyring::Entry::new(&self.service, &format!("{}:{}", key.namespace(), key.kind()))
            .map_err(|e| SecretError::Backend(e.to_string()))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }

    async fn delete_secret(&self, key: SecretKey) -> SecretResult<()> {
        let entry = keyring::Entry::new(&self.service, &format!("{}:{}", key.namespace(), key.kind()))
            .map_err(|e| SecretError::Backend(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }
}
