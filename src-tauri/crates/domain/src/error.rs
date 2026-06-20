//! Error type for the domain layer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid ID: {0}")]
    InvalidId(String),

    #[error("validation failed: {0}")]
    Validation(String),
}

pub type DomainResult<T> = Result<T, DomainError>;
