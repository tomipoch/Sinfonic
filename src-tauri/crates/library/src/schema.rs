//! Schema v1 and the versioned migrations that build it.
//!
//! The migration system is intentionally tiny: a `const` slice of
//! `(version, name, sql)` triples, run inside a transaction, recorded
//! in `schema_migrations`. There is no DSL, no auto-generation, no
//! checksum registry — every migration is just a SQL string and a
//! human-readable name. When v2 ships, append to `MIGRATIONS`; never
//! edit a past entry.
//!
//! All tables are scoped by `server_id` so a single database can
//! cache data from multiple Jellyfin / Subsonic servers without
//! collisions on numeric IDs.

use rusqlite::Connection;

/// Current schema version. Always equal to the highest version number
/// present in [`MIGRATIONS`]; checked at runtime so callers don't have
/// to trust a hand-bumped constant.
pub fn current_schema_version() -> u32 {
    MIGRATIONS
        .iter()
        .map(|m| m.version)
        .max()
        .expect("MIGRATIONS must not be empty")
}

/// One versioned schema change.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Migrations in version order. Append-only.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: INITIAL_SCHEMA,
    },
    Migration {
        version: 2,
        name: "add_year_and_smart_playlists",
        sql: MIGRATION_V2,
    },
    Migration {
        version: 3,
        name: "servers_username",
        sql: MIGRATION_V3,
    },
    Migration {
        version: 4,
        name: "albums_fk_artist_id_only",
        sql: MIGRATION_V4,
    },
    Migration {
        version: 5,
        name: "playlists_cover",
        sql: MIGRATION_V5,
    },
];

const INITIAL_SCHEMA: &str = r#"
-- `schema_migrations` is created by `run_migrations` before any
-- migration runs, so it must not appear here.

CREATE TABLE servers (
    server_id   TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    name        TEXT NOT NULL,
    base_url    TEXT NOT NULL,
    last_sync_at INTEGER,
    last_sync_status TEXT
);

CREATE TABLE artists (
    server_id   TEXT NOT NULL,
    -- `artist_id` is globally unique (the Subsonic / Jellyfin
    -- mapping layer prepends `artist-`, so e.g. `artist-ar-1`
    -- cannot collide across servers). The `UNIQUE` constraint
    -- lets the `albums.artist_id` FK reference it directly —
    -- SQLite requires the FK target to be uniquely keyed, and
    -- the composite PK on `(server_id, artist_id)` is not enough
    -- on its own. See MIGRATION_V4 for the matching constraint
    -- added to pre-existing databases.
    artist_id   TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    album_count INTEGER NOT NULL DEFAULT 0,
    track_count INTEGER NOT NULL DEFAULT 0,
    favorite    INTEGER NOT NULL DEFAULT 0,
    image_kind  TEXT,
    image_tag   TEXT,
    PRIMARY KEY (server_id, artist_id)
);
CREATE INDEX idx_artists_server_name ON artists(server_id, name COLLATE NOCASE);

CREATE TABLE albums (
    server_id        TEXT NOT NULL,
    album_id         TEXT NOT NULL,
    title            TEXT NOT NULL,
    artist           TEXT NOT NULL,
    artist_id        TEXT,
    year             INTEGER,
    track_count      INTEGER NOT NULL DEFAULT 0,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    favorite         INTEGER NOT NULL DEFAULT 0,
    image_kind       TEXT,
    image_tag        TEXT,
    PRIMARY KEY (server_id, album_id),
    -- FK on `artist_id` alone, not `(server_id, artist_id)`. Artist
    -- IDs are globally unique (the Subsonic/Jellyfin mapping layer
    -- prepends `artist-`, so e.g. `artist-ar-1` cannot collide across
    -- servers), so referencing `artist_id` is sufficient. The
    -- earlier composite FK triggered `ON DELETE SET NULL` on both
    -- columns of the FK — including `server_id`, which is `NOT
    -- NULL` — and re-syncs that deleted stale artists crashed with
    -- `NOT NULL constraint failed: albums.server_id`. See
    -- MIGRATION_V4 below.
    FOREIGN KEY (artist_id) REFERENCES artists(artist_id)
        ON DELETE SET NULL
);
CREATE INDEX idx_albums_server_title ON albums(server_id, title COLLATE NOCASE);
CREATE INDEX idx_albums_server_artist ON albums(server_id, artist_id);

CREATE TABLE album_genres (
    server_id TEXT NOT NULL,
    album_id  TEXT NOT NULL,
    genre     TEXT NOT NULL,
    PRIMARY KEY (server_id, album_id, genre),
    FOREIGN KEY (server_id, album_id) REFERENCES albums(server_id, album_id)
        ON DELETE CASCADE
);
CREATE INDEX idx_album_genres_genre ON album_genres(server_id, genre COLLATE NOCASE);

CREATE TABLE tracks (
    server_id        TEXT NOT NULL,
    track_id         TEXT NOT NULL,
    album_id         TEXT NOT NULL,
    title            TEXT NOT NULL,
    artist           TEXT NOT NULL,
    artist_id        TEXT,
    album            TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    track_number     INTEGER NOT NULL DEFAULT 0,
    disc_number      INTEGER NOT NULL DEFAULT 1,
    favorite         INTEGER NOT NULL DEFAULT 0,
    image_kind       TEXT,
    image_tag        TEXT,
    PRIMARY KEY (server_id, track_id),
    FOREIGN KEY (server_id, album_id) REFERENCES albums(server_id, album_id)
        ON DELETE CASCADE
);
CREATE INDEX idx_tracks_server_album ON tracks(server_id, album_id, disc_number, track_number);
CREATE INDEX idx_tracks_server_title ON tracks(server_id, title COLLATE NOCASE);

CREATE TABLE playlists (
    server_id        TEXT NOT NULL,
    playlist_id      TEXT NOT NULL,
    name             TEXT NOT NULL,
    track_count      INTEGER NOT NULL DEFAULT 0,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    owner            TEXT,
    public           INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (server_id, playlist_id)
);

CREATE TABLE playlist_tracks (
    server_id   TEXT NOT NULL,
    playlist_id TEXT NOT NULL,
    position    INTEGER NOT NULL,
    track_id    TEXT NOT NULL,
    PRIMARY KEY (server_id, playlist_id, position),
    FOREIGN KEY (server_id, playlist_id) REFERENCES playlists(server_id, playlist_id)
        ON DELETE CASCADE
);
CREATE INDEX idx_playlist_tracks_lookup ON playlist_tracks(server_id, playlist_id, position);

CREATE TABLE genres (
    server_id   TEXT NOT NULL,
    genre_id    TEXT NOT NULL,
    name        TEXT NOT NULL,
    album_count INTEGER NOT NULL DEFAULT 0,
    track_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (server_id, genre_id)
);

CREATE TABLE library_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- FTS5 virtual table for full-text search across the library.
-- content=' ' makes it contentless (we manage the rowid mapping
-- via the explicit track_id/album_id/artist_id columns) so we can
-- index entities of different kinds in the same table.
CREATE VIRTUAL TABLE library_fts USING fts5(
    kind,
    server_id,
    entity_id,
    title,
    subtitle,
    tokenize = 'unicode61 remove_diacritics 2'
);
"#;

const MIGRATION_V2: &str = r#"
-- Add year column to tracks (denormalised from album for rule evaluation).
ALTER TABLE tracks ADD COLUMN year INTEGER;

-- Smart playlists (Phase 9): single-rule evaluation stored in SQLite.
CREATE TABLE smart_playlists (
    server_id   TEXT NOT NULL,
    sp_id       TEXT NOT NULL,
    name        TEXT NOT NULL,
    field       TEXT NOT NULL,
    operator    TEXT NOT NULL,
    value       TEXT NOT NULL,
    sort_field  TEXT NOT NULL DEFAULT 'title',
    sort_dir    TEXT NOT NULL DEFAULT 'asc',
    limit_n     INTEGER NOT NULL DEFAULT 50,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    PRIMARY KEY (server_id, sp_id)
);
CREATE INDEX idx_smart_playlists_server ON smart_playlists(server_id);
"#;

// Persist the username alongside each server row so the active
// provider can be reconstructed from keyring + SQLite alone
// (Subsonic needs the username on every request to sign the auth
// header; storing it on the row avoids a separate preferences
// table for one column).
const MIGRATION_V3: &str = r#"
ALTER TABLE servers ADD COLUMN username TEXT;
"#;

// Rebuild `albums` with an FK on `artist_id` only, instead of the
// composite `(server_id, artist_id)` we shipped in v1. The
// composite form is the textbook SQLite recipe for "scope an FK
// alongside its parent's PK", but it interacts badly with
// `ON DELETE SET NULL`: per the SQLite docs, `SET NULL` on a
// composite FK NULLs **every** column in the FK, including
// `server_id` — which is `NOT NULL` on `albums`. That made every
// resync crash with `NOT NULL constraint failed: albums.server_id`
// the moment `replace_artists` tried to evict a stale artist that
// still had albums referencing it.
//
// The rebuild follows SQLite's standard "rebuild table to change
// FK" recipe: build a new table with the desired schema, copy the
// data over, drop the old table, rename the new one, recreate the
// indexes. `album_genres` references `albums(server_id, album_id)`
// via `ON DELETE CASCADE`; the rename preserves the table name so
// the FK still resolves. `library_fts` has no FK so it is not
// affected.
//
// We rebuild `artists` first to mark `artist_id UNIQUE`. SQLite
// requires the target of an FK to have a `UNIQUE` or `PRIMARY KEY`
// constraint — the v1 composite PK on `(server_id, artist_id)` is
// not enough. Artist IDs are globally unique already (the
// Subsonic / Jellyfin mapping layer prepends `artist-` so e.g.
// `artist-ar-1` cannot collide across servers), so adding the
// constraint is a no-op for valid data and fails the migration
// loudly for corrupt data.
const MIGRATION_V4: &str = r#"
CREATE TABLE artists_new (
    server_id   TEXT NOT NULL,
    artist_id   TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    album_count INTEGER NOT NULL DEFAULT 0,
    track_count INTEGER NOT NULL DEFAULT 0,
    favorite    INTEGER NOT NULL DEFAULT 0,
    image_kind  TEXT,
    image_tag   TEXT,
    PRIMARY KEY (server_id, artist_id)
);
INSERT INTO artists_new (
    server_id, artist_id, name, album_count, track_count,
    favorite, image_kind, image_tag
)
SELECT
    server_id, artist_id, name, album_count, track_count,
    favorite, image_kind, image_tag
FROM artists;
DROP TABLE artists;
ALTER TABLE artists_new RENAME TO artists;
CREATE INDEX idx_artists_server_name ON artists(server_id, name COLLATE NOCASE);

CREATE TABLE albums_new (
    server_id        TEXT NOT NULL,
    album_id         TEXT NOT NULL,
    title            TEXT NOT NULL,
    artist           TEXT NOT NULL,
    artist_id        TEXT,
    year             INTEGER,
    track_count      INTEGER NOT NULL DEFAULT 0,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    favorite         INTEGER NOT NULL DEFAULT 0,
    image_kind       TEXT,
    image_tag        TEXT,
    PRIMARY KEY (server_id, album_id),
    FOREIGN KEY (artist_id) REFERENCES artists(artist_id)
        ON DELETE SET NULL
);
INSERT INTO albums_new (
    server_id, album_id, title, artist, artist_id,
    year, track_count, duration_seconds, favorite, image_kind, image_tag
)
SELECT
    server_id, album_id, title, artist, artist_id,
    year, track_count, duration_seconds, favorite, image_kind, image_tag
FROM albums;
DROP TABLE albums;
ALTER TABLE albums_new RENAME TO albums;
CREATE INDEX idx_albums_server_title  ON albums(server_id, title COLLATE NOCASE);
CREATE INDEX idx_albums_server_artist ON albums(server_id, artist_id);
"#;

/// V5: add cover-art columns to the `playlists` table so the cached
/// playlist metadata carries an `image_ref`. Existing rows get
/// `NULL` for both columns, which maps to `image_ref = None` in the
/// domain type.
///
/// V1's `INITIAL_SCHEMA` no longer creates these columns (so a fresh
/// install hits V5's ALTERs once). Existing databases created before
/// V5 will get the columns added here without any data loss.
const MIGRATION_V5: &str = r#"
ALTER TABLE playlists ADD COLUMN image_kind TEXT;
ALTER TABLE playlists ADD COLUMN image_tag  TEXT;
"#;

/// Apply every migration in `MIGRATIONS` that has not been applied
/// yet, recording each in `schema_migrations` inside a single
/// transaction. Idempotent: running twice does nothing on the second
/// call.
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("BEGIN")?;
    let result = (|| -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at INTEGER NOT NULL
            )",
        )?;

        let mut stmt = conn.prepare("SELECT version FROM schema_migrations")?;
        let mut applied: Vec<u32> = stmt
            .query_map([], |r| r.get::<_, u32>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        applied.sort_unstable();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        for migration in MIGRATIONS {
            if applied.binary_search(&migration.version).is_ok() {
                continue;
            }
            conn.execute_batch(migration.sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![migration.version, migration.name, now],
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    Ok(())
}

/// PRAGMAs applied to every new connection in the pool. Kept in one
/// place so every read path inherits the same settings.
pub fn init_connection(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA temp_store = MEMORY;",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn initial_schema_creates_core_tables() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        for table in [
            "servers",
            "artists",
            "albums",
            "tracks",
            "playlists",
            "album_genres",
            "playlist_tracks",
            "genres",
            "library_fts",
            "smart_playlists",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?1",
                    rusqlite::params![table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "expected table {table} to exist");
        }
    }

    #[test]
    fn foreign_keys_are_enforced_when_enabled() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        run_migrations(&conn).unwrap();
        // Insert a track with no parent album: should fail.
        let result = conn.execute(
            "INSERT INTO tracks (server_id, track_id, album_id, title, artist, album) VALUES ('s1','t1','missing','x','y','z')",
            [],
        );
        assert!(result.is_err());
    }

    /// Regression for the v4 migration: deleting an artist whose
    /// albums still reference it used to crash with
    /// `NOT NULL constraint failed: albums.server_id`. The v1 FK
    /// `(server_id, artist_id) ON DELETE SET NULL` is a composite
    /// FK; SQLite's FK action semantics NULL **every** column in
    /// the child FK on parent delete — including `server_id`,
    /// which is `NOT NULL` on `albums`. After v4 the FK is on
    /// `artist_id` alone, so SET NULL only touches `artist_id`.
    ///
    /// Asserts the migration ran (albums has the new single-column
    /// FK) and that an UPDATE with `server_id = NULL` would still
    /// be rejected — i.e. the column is still NOT NULL.
    #[test]
    fn albums_albums_fk_is_artist_id_only_after_v4() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // The FK list for `albums` should reference `artists(artist_id)`
        // and nothing else. SQLite stores FK definitions in
        // `pragma foreign_key_list(albums)`.
        let mut stmt = conn
            .prepare("SELECT \"from\", \"table\", \"to\" FROM pragma_foreign_key_list('albums')")
            .unwrap();
        let fks: Vec<(String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(
            fks,
            vec![("artist_id".to_string(), "artists".to_string(), "artist_id".to_string())],
            "albums FK must reference artists(artist_id) after v4 (was artists(server_id, artist_id))"
        );

        // Belt-and-braces: `server_id` must still be NOT NULL on albums.
        let result = conn.execute(
            "INSERT INTO albums (server_id, album_id, title, artist, artist_id) \
             VALUES (NULL, 'a-1', 'A', 'A', NULL)",
            [],
        );
        assert!(result.is_err(), "albums.server_id must remain NOT NULL");
    }
}
