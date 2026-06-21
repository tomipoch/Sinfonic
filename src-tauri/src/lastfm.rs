//! Last.fm / Libre.fm credentials + session key persistence.
//!
//! Three keys live in the OS keyring:
//! - `LastFmApiSecret`  → JSON `{ "api_key": "...", "api_secret": "..." }`
//! - `LastFmSession`    → the session key returned by
//!   `auth.getMobileSession`
//!
//! The plaintext password is NEVER persisted — it lives in JS for the
//! duration of the login flow only, then is hashed here and sent
//! straight to Last.fm.

use serde::{Deserialize, Serialize};

use md5::Context;
use sinfonic_lastfm::{LastFmClient, LastFmCredentials};
use sinfonic_secrets::{SecretKey, SecretStore};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub api_key: String,
    pub api_secret: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LastFmStatus {
    pub configured: bool,
    pub authenticated: bool,
    pub username: Option<String>,
}

/// MD5-hash a UTF-8 string and return lower-case hex. Last.fm's mobile
/// handshake takes the hash directly, so we never persist the
/// plaintext.
pub fn md5_hex(input: &str) -> String {
    let mut ctx = Context::new();
    ctx.consume(input.as_bytes());
    let digest = ctx.compute();
    hex::encode(digest.0)
}

/// Persist the api key + secret pair under `LastFmApiSecret` as a
/// JSON blob. Returns `Ok(true)` when an existing entry was
/// replaced.
#[allow(dead_code)]
pub async fn store_credentials<S: SecretStore + ?Sized>(
    secrets: &S,
    creds: &StoredCredentials,
) -> Result<bool, String> {
    let blob = serde_json::to_string(creds).map_err(|e| e.to_string())?;
    secrets
        .save_secret(SecretKey::LastFmApiSecret, blob)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

pub async fn load_credentials<S: SecretStore + ?Sized>(
    secrets: &S,
) -> Result<Option<StoredCredentials>, String> {
    let raw = secrets
        .load_secret(SecretKey::LastFmApiSecret)
        .await
        .map_err(|e| e.to_string())?;
    match raw {
        None => Ok(None),
        Some(blob) => serde_json::from_str(&blob)
            .map(Some)
            .map_err(|e| format!("invalid stored credentials: {e}")),
    }
}

pub async fn load_session<S: SecretStore + ?Sized>(
    secrets: &S,
) -> Result<Option<String>, String> {
    secrets
        .load_secret(SecretKey::LastFmSession)
        .await
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn store_session<S: SecretStore + ?Sized>(
    secrets: &S,
    session_key: &str,
) -> Result<(), String> {
    secrets
        .save_secret(SecretKey::LastFmSession, session_key.to_string())
        .await
        .map_err(|e| e.to_string())
}

pub async fn clear_secrets<S: SecretStore + ?Sized>(secrets: &S) -> Result<(), String> {
    secrets
        .delete_secret(SecretKey::LastFmApiSecret)
        .await
        .map_err(|e| e.to_string())?;
    secrets
        .delete_secret(SecretKey::LastFmSession)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Build the in-memory client + persist the session. Used by both
/// the `lastfm_connect` Tauri command and the startup-time resume
/// path.
#[allow(dead_code)]
pub async fn build_client_with_session(
    creds: &StoredCredentials,
    session_key: String,
) -> Result<LastFmClient, String> {
    let mut client = LastFmClient::new(creds.api_key.clone(), creds.api_secret.clone())
        .map_err(|e| format!("lastfm client init: {e}"))?;
    client
        .resume(session_key)
        .await
        .map_err(|e| format!("lastfm resume: {e}"))?;
    Ok(client)
}

pub async fn authenticate_and_store(
    secrets: &dyn SecretStore,
    creds: &StoredCredentials,
    username: &str,
    password: &str,
    lastfm_slot: &Mutex<Option<LastFmClient>>,
) -> Result<String, String> {
    let password_md5 = md5_hex(password);
    let auth_creds = LastFmCredentials {
        api_key: creds.api_key.clone(),
        api_secret: creds.api_secret.clone(),
        username: username.to_string(),
        password_md5,
    };
    let mut client = LastFmClient::new(creds.api_key.clone(), creds.api_secret.clone())
        .map_err(|e| format!("lastfm client init: {e}"))?;
    let session = client
        .authenticate(&auth_creds)
        .await
        .map_err(|e| format!("lastfm auth: {e}"))?;

    secrets
        .save_secret(
            SecretKey::LastFmApiSecret,
            serde_json::to_string(creds).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
    secrets
        .save_secret(SecretKey::LastFmSession, session.clone())
        .await
        .map_err(|e| e.to_string())?;

    *lastfm_slot.lock().await = Some(client);
    Ok(session)
}

pub async fn try_resume(
    secrets: &dyn SecretStore,
    lastfm_slot: &Mutex<Option<LastFmClient>>,
) -> Option<String> {
    let creds = match load_credentials(secrets).await.ok().flatten() {
        Some(c) => c,
        None => return None,
    };
    let session = match load_session(secrets).await.ok().flatten() {
        Some(s) => s,
        None => return None,
    };
    match build_client_with_session(&creds, session.clone()).await {
        Ok(client) => {
            *lastfm_slot.lock().await = Some(client);
            Some(session)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// In-memory `SecretStore` for tests. Keeps each secret in a
    /// `Mutex<HashMap>` keyed by `format!("{}:{}", namespace, kind)`.
    struct InMemoryStore {
        inner: parking_lot::Mutex<std::collections::HashMap<String, String>>,
    }

    impl InMemoryStore {
        fn new() -> Self {
            Self {
                inner: parking_lot::Mutex::new(Default::default()),
            }
        }
    }

    #[async_trait::async_trait]
    impl SecretStore for InMemoryStore {
        async fn save_secret(
            &self,
            key: SecretKey,
            secret: String,
        ) -> sinfonic_secrets::SecretResult<()> {
            self.inner.lock().insert(
                format!("{}:{}", key.namespace(), key.kind()),
                secret,
            );
            Ok(())
        }
        async fn load_secret(
            &self,
            key: SecretKey,
        ) -> sinfonic_secrets::SecretResult<Option<String>> {
            Ok(self
                .inner
                .lock()
                .get(&format!("{}:{}", key.namespace(), key.kind()))
                .cloned())
        }
        async fn delete_secret(
            &self,
            key: SecretKey,
        ) -> sinfonic_secrets::SecretResult<()> {
            self.inner
                .lock()
                .remove(&format!("{}:{}", key.namespace(), key.kind()));
            Ok(())
        }
    }

    #[test]
    fn md5_hex_matches_known_vector() {
        // md5("password") = 5f4dcc3b5aa765d61d8327deb882cf99
        assert_eq!(
            md5_hex("password"),
            "5f4dcc3b5aa765d61d8327deb882cf99"
        );
    }

    #[test]
    fn store_and_load_credentials_round_trip() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemoryStore::new());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let creds = StoredCredentials {
                api_key: "k".into(),
                api_secret: "s".into(),
            };
            store_credentials(store.as_ref(), &creds).await.unwrap();
            let loaded = load_credentials(store.as_ref()).await.unwrap().unwrap();
            assert_eq!(loaded.api_key, "k");
            assert_eq!(loaded.api_secret, "s");
        });
    }

    #[test]
    fn clear_secrets_removes_both_keys() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemoryStore::new());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            store_credentials(
                store.as_ref(),
                &StoredCredentials {
                    api_key: "k".into(),
                    api_secret: "s".into(),
                },
            )
            .await
            .unwrap();
            store_session(store.as_ref(), "SK").await.unwrap();
            clear_secrets(store.as_ref()).await.unwrap();
            assert!(load_credentials(store.as_ref()).await.unwrap().is_none());
            assert!(load_session(store.as_ref()).await.unwrap().is_none());
        });
    }
}
