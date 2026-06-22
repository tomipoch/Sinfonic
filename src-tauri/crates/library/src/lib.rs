//! SQLite library cache (synced data, FTS5 search, settings).
//!
//! # Phase status
//!
//! - Phase 0: skeleton — every method returned `Err("not
//!   implemented")`. Kept the surface to prove the workspace
//!   compiles.
//! - Phase 2: real implementation.
//!   - `Store::open(path)` / `Store::open_memory()` return a
//!     thread-safe handle backed by an r2d2 pool with
//!     `journal_mode=WAL` and `foreign_keys=ON` on every
//!     connection.
//!   - Schema v1 lives in `schema::MIGRATIONS`; new versions are
//!     appended (never edited) and applied idempotently on every
//!     `open`.
//!   - CRUD is server-scoped: every query takes a `&ServerId`
//!     because the same database can hold many providers.
//!   - `replace_*` diffs against the existing rows for that server
//!     + kind and applies the new set in a single transaction.
//!   - Full-text search lives in `search` and runs over the
//!     contentless FTS5 virtual table populated alongside every
//!     insert.

pub mod album_art;
pub mod error;
pub mod rows;
pub mod schema;
pub mod search;
pub mod smart_playlists;
pub mod store;

pub use album_art::{AlbumArtCache, CachedImage, CachedImageMeta, ImageCacheKey};
pub use error::{LibraryError, LibraryResult};
pub use store::Store;
