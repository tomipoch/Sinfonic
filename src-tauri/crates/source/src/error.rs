//! `ProviderError` — single error type every provider returns.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Deserialize, Serialize)]
pub enum ProviderError {
    #[error("provider authentication failed: {0}")]
    Auth(String),

    #[error("provider TLS validation failed: {0}")]
    Tls(String),

    #[error("provider network failed: {0}")]
    Network(String),

    #[error("provider server failed with status {status}: {message}")]
    Server { status: u16, message: String },

    #[error("provider item was not found")]
    NotFound,

    #[error("provider capability is not supported: {0}")]
    Unsupported(&'static str),

    #[error("provider failed: {0}")]
    Other(String),
}

pub type ProviderResult<T> = Result<T, ProviderError>;
