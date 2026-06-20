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

/// Current schema version. Bump this when appending a migration.
pub const SCHEMA_VERSION: u32 = 1;

/// One versioned schema change.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Migrations in version order. Append-only.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: INITIAL_SCHEMA,
}];

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
    artist_id   TEXT NOT NULL,
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
    FOREIGN KEY (server_id, artist_id) REFERENCES artists(server_id, artist_id)
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
        assert_eq!(count, 1);
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
}
