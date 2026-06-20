//! `Store` — the only public type other crates depend on.
//!
//! Phase 0: struct + builder. Phase 2 implements `open` (runs migrations),
//! `begin_sync`/`complete_sync`, and the upsert-with-delta API.

use std::path::Path;

pub struct Store;

impl Store {
    /// Open or create the database at `path`, running all migrations.
    pub fn open(_path: impl AsRef<Path>) -> Result<Self, String> {
        Err("library store not implemented in skeleton".into())
    }

    /// In-memory store for tests.
    pub fn open_memory() -> Result<Self, String> {
        Err("library store not implemented in skeleton".into())
    }
}
