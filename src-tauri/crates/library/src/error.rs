//! Errors surfaced by the library cache.
//!
//! Kept in this crate (not the domain) because they describe SQLite
//! and migration failures, which are storage concerns, not music
//! domain concerns. The Tauri command boundary converts to `String`
//! via `Display`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("migration failed: {0}")]
    Migration(String),

    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(String),
}

pub type LibraryResult<T> = Result<T, LibraryError>;
