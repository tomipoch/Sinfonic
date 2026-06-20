//! SQLite library cache (synced data, FTS5 search, settings).
//!
//! Phase 0: skeleton. Phase 2 implements the schema v1 + migrations,
//! delta sync, and FTS5 queries. `rusqlite` is intentionally absent
//! from Fase 0 to keep the first `cargo check` fast.

#![allow(dead_code)]

pub mod schema;
pub mod search;
pub mod store;

pub use store::Store;
