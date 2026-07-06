//! The `Store` is the only public type other crates depend on.
//!
//! It owns an `r2d2` connection pool against a single SQLite file
//! (or `:memory:`). Every public method grabs a pooled connection,
//! runs the query, and returns the connection when done. The pool
//! is configured with the schema's PRAGMAs (`journal_mode=WAL`,
//! `foreign_keys=ON`, `busy_timeout=5s`) so every read inherits them.
//!
//! All data is scoped by `server_id` so multiple providers can
//! coexist in one database. An entity id is meaningless without its
//! server.
//!
//! Sync strategy: callers push batches via `replace_albums`,
//! `replace_artists`, `replace_tracks`, `replace_playlist`. The
//! "replace" variants diff against the existing rows for that
//! server + kind and apply the new set in a single transaction.

use std::path::Path;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Transaction;
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, PagedResponse, Playlist, PlaylistId, QueueSnapshot,
    ServerId, Track, TrackId,
};

use crate::error::{LibraryError, LibraryResult};
use crate::rows;
use crate::schema::{init_connection, run_migrations};
use crate::search;

/// Connection pool type alias.
pub type ConnectionPool = Pool<SqliteConnectionManager>;

/// Thread-safe handle to the library cache.
#[derive(Clone)]
pub struct Store {
    pool: ConnectionPool,
}

impl Store {
    /// Open (or create) the database at `path`, running all pending
    /// migrations.
    pub fn open(path: impl AsRef<Path>) -> LibraryResult<Self> {
        let manager = SqliteConnectionManager::file(path.as_ref()).with_init(init_connection);
        let pool = Pool::builder().max_size(8).build(manager)?;
        {
            let conn = pool.get()?;
            run_migrations(&conn)?;
        }
        Ok(Self { pool })
    }

    /// Open an in-memory database. `max_size=1` so all queries see
    /// the same data; with `:memory:` each pooled connection would
    /// otherwise be its own database.
    pub fn open_memory() -> LibraryResult<Self> {
        let manager = SqliteConnectionManager::memory().with_init(init_connection);
        let pool = Pool::builder().max_size(1).build(manager)?;
        let conn = pool.get()?;
        run_migrations(&conn)?;
        Ok(Self { pool })
    }

    /// Borrow a connection from the pool. Public so integration
    /// tests can run ad-hoc queries.
    pub fn connection(&self) -> LibraryResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    // ─── Server registration ─────────────────────────────────────

    /// Upsert a server row. Used by the auth flow once login
    /// succeeds. `username` is optional (Jellyfin doesn't reuse it,
    /// Subsonic requires it on every request).
    pub fn upsert_server(
        &self,
        server_id: &ServerId,
        kind: &str,
        name: &str,
        base_url: &str,
        username: Option<&str>,
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO servers (server_id, kind, name, base_url, username) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(server_id) DO UPDATE SET
                 kind = excluded.kind,
                 name = excluded.name,
                 base_url = excluded.base_url,
                 username = excluded.username",
            rusqlite::params![server_id.as_str(), kind, name, base_url, username],
        )?;
        Ok(())
    }

    pub fn delete_server(&self, server_id: &ServerId) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.unchecked_transaction()?;
        for table in [
            "playlist_tracks",
            "playlists",
            "tracks",
            "album_genres",
            "albums",
            "artists",
            "genres",
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE server_id = ?1"),
                rusqlite::params![server_id.as_str()],
            )?;
        }
        tx.execute(
            "DELETE FROM library_fts WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
        )?;
        tx.execute(
            "DELETE FROM servers WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
        )?;
        // Drop any "last active" pointer that targeted this server so
        // the next launch doesn't try to restore a deleted connection.
        self.clear_preference_tx(&tx, "last_active_server_id")?;
        tx.commit()?;
        Ok(())
    }

    // ─── Preferences ─────────────────────────────────────────────

    /// Read a string value from the `library_meta` table. Returns
    /// `None` if the key has never been written. Used for things like
    /// the last-active server pointer that must survive across
    /// launches but doesn't belong on a row in a domain table.
    pub fn get_preference(&self, key: &str) -> LibraryResult<Option<String>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT value FROM library_meta WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Upsert a key/value pair in `library_meta`. Pass `None` to clear
    /// the entry; the row is removed rather than nulled so a subsequent
    /// `get_preference` returns `None` (matching the "never written"
    /// semantics).
    pub fn set_preference(&self, key: &str, value: Option<&str>) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        match value {
            Some(v) => {
                tx.execute(
                    "INSERT INTO library_meta (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![key, v],
                )?;
            }
            None => {
                tx.execute(
                    "DELETE FROM library_meta WHERE key = ?1",
                    rusqlite::params![key],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn clear_preference_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        key: &str,
    ) -> LibraryResult<()> {
        tx.execute(
            "DELETE FROM library_meta WHERE key = ?1",
            rusqlite::params![key],
        )?;
        Ok(())
    }

    // ─── Queue snapshots ────────────────────────────────────────
    //
    // One row per server; the row stores a JSON-serialised
    // `QueueSnapshot`. The persistence layer is intentionally
    // dumb — it round-trips bytes — so the on-the-wire format lives
    // entirely in `sinfonic_domain::queue::QueueSnapshot`.

    /// Persist (or overwrite) the queue snapshot for one server.
    /// Called from every Tauri command that mutates the queue so
    /// the next launch can restore the user's history.
    pub fn save_queue_snapshot(
        &self,
        server_id: &ServerId,
        snapshot: &QueueSnapshot,
    ) -> LibraryResult<()> {
        let json = serde_json::to_string(snapshot).map_err(|e| {
            LibraryError::Validation(format!("queue snapshot serialize: {e}"))
        })?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO queue_snapshots (server_id, snapshot, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(server_id) DO UPDATE SET
                 snapshot   = excluded.snapshot,
                 updated_at = excluded.updated_at",
            rusqlite::params![server_id.as_str(), json, now],
        )?;
        Ok(())
    }

    /// Load the persisted queue snapshot for one server, or `None`
    /// if no snapshot has ever been written. A malformed snapshot
    /// (e.g. from an older app version with a different shape) is
    /// treated as "missing" so the caller falls back to an empty
    /// queue rather than crashing.
    pub fn load_queue_snapshot(
        &self,
        server_id: &ServerId,
    ) -> LibraryResult<Option<QueueSnapshot>> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT snapshot FROM queue_snapshots WHERE server_id = ?1")?;
        let mut rows = stmt.query([server_id.as_str()])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let json: String = row.get(0)?;
        match serde_json::from_str::<QueueSnapshot>(&json) {
            Ok(snap) => Ok(Some(snap)),
            Err(e) => {
                tracing::warn!(
                    target: "sinfonic::library",
                    error = %e,
                    "queue snapshot deserialise failed; treating as missing"
                );
                Ok(None)
            }
        }
    }

    /// Delete the persisted queue snapshot for one server. Called
    /// when the user deletes the server row outright (the FK cascade
    /// already does this, but having an explicit method keeps tests
    /// + future call sites symmetrical with save/load).
    pub fn delete_queue_snapshot(&self, server_id: &ServerId) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM queue_snapshots WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
        )?;
        Ok(())
    }

    // ─── Albums ──────────────────────────────────────────────────

    /// Replace every album for a server with `albums`, in a single
    /// transaction. The diff is computed inside the transaction so
    /// orphans are removed atomically with the new rows.
    pub fn replace_albums(&self, server_id: &ServerId, albums: &[Album]) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;

        let existing = collect_ids(&tx, "albums", "album_id", server_id.as_str())?;
        // Convert to a HashSet so the orphan-detection scan is O(existing)
        // instead of O(existing × new). For a 5 k-album library the old
        // code ran ~25 M string comparisons; this is now ~5 k.
        let new_ids: std::collections::HashSet<&str> =
            albums.iter().map(|a| a.id.as_str()).collect();
        for id in existing.iter().filter(|id| !new_ids.contains(id.as_str())) {
            tx.execute(
                "DELETE FROM albums WHERE server_id = ?1 AND album_id = ?2",
                rusqlite::params![server_id.as_str(), id],
            )?;
            tx.execute(
                "DELETE FROM library_fts WHERE server_id = ?1 AND kind = 'album' AND entity_id = ?2",
                rusqlite::params![server_id.as_str(), id],
            )?;
        }

        for album in albums {
            upsert_album(&tx, server_id, album)?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn upsert_album(&self, server_id: &ServerId, album: &Album) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        upsert_album(&tx, server_id, album)?;
        tx.commit()?;
        Ok(())
    }

    /// Batch-upsert albums in a single transaction. See
    /// `upsert_tracks` for why this exists alongside `replace_albums`.
    pub fn upsert_albums(&self, server_id: &ServerId, albums: &[Album]) -> LibraryResult<()> {
        if albums.is_empty() {
            return Ok(());
        }
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        for album in albums {
            upsert_album(&tx, server_id, album)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_albums(
        &self,
        server_id: &ServerId,
        offset: usize,
        limit: usize,
    ) -> LibraryResult<PagedResponse<Album>> {
        let conn = self.connection()?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM albums WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT album_id, title, artist, artist_id, year, track_count,
                    duration_seconds, favorite, image_kind, image_tag
             FROM albums
             WHERE server_id = ?1
             ORDER BY title COLLATE NOCASE
             LIMIT ?2 OFFSET ?3",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), limit as i64, offset as i64],
                rows::row_to_album,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(PagedResponse::new(items, total as usize))
    }

    pub fn get_album(
        &self,
        server_id: &ServerId,
        album_id: &AlbumId,
    ) -> LibraryResult<Option<Album>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT album_id, title, artist, artist_id, year, track_count,
                    duration_seconds, favorite, image_kind, image_tag
             FROM albums
             WHERE server_id = ?1 AND album_id = ?2",
        )?;
        let mut rows_iter = stmt.query_map(
            rusqlite::params![server_id.as_str(), album_id.as_str()],
            rows::row_to_album,
        )?;
        match rows_iter.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    // ─── Artists ─────────────────────────────────────────────────

    pub fn replace_artists(&self, server_id: &ServerId, artists: &[Artist]) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;

        let existing = collect_ids(&tx, "artists", "artist_id", server_id.as_str())?;
        let new_ids: std::collections::HashSet<&str> =
            artists.iter().map(|a| a.id.as_str()).collect();
        for id in existing.iter().filter(|id| !new_ids.contains(id.as_str())) {
            tx.execute(
                "DELETE FROM artists WHERE server_id = ?1 AND artist_id = ?2",
                rusqlite::params![server_id.as_str(), id],
            )?;
            tx.execute(
                "DELETE FROM library_fts WHERE server_id = ?1 AND kind = 'artist' AND entity_id = ?2",
                rusqlite::params![server_id.as_str(), id],
            )?;
        }

        for artist in artists {
            upsert_artist(&tx, server_id, artist)?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn upsert_artist(&self, server_id: &ServerId, artist: &Artist) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        upsert_artist(&tx, server_id, artist)?;
        tx.commit()?;
        Ok(())
    }

    /// Batch-upsert artists in a single transaction. See
    /// `upsert_tracks` for why this exists alongside `replace_artists`.
    pub fn upsert_artists(&self, server_id: &ServerId, artists: &[Artist]) -> LibraryResult<()> {
        if artists.is_empty() {
            return Ok(());
        }
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        for artist in artists {
            upsert_artist(&tx, server_id, artist)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_artists(
        &self,
        server_id: &ServerId,
        offset: usize,
        limit: usize,
    ) -> LibraryResult<PagedResponse<Artist>> {
        let conn = self.connection()?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artists WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT artist_id, name, album_count, track_count, favorite, image_kind, image_tag
             FROM artists
             WHERE server_id = ?1
             ORDER BY name COLLATE NOCASE
             LIMIT ?2 OFFSET ?3",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), limit as i64, offset as i64],
                rows::row_to_artist,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(PagedResponse::new(items, total as usize))
    }

    /// Recompute `artists.track_count` from the cached tracks table.
    ///
    /// Jellyfin's `MusicArtist` DTO doesn't expose a direct track count
    /// (only an opaque `ChildCount` for albums), so the artist mapper
    /// hardcodes `0` there. Subsonic does return `songCount` per
    /// artist, but the value can lag when tracks are deleted on the
    /// server. After every sync, walk the cached tracks for this
    /// server and assign each artist a count of tracks whose
    /// `artist_id` matches. One statement per server, no N+1.
    pub fn recompute_artist_track_counts(
        &self,
        server_id: &ServerId,
    ) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        // First, normalise: artists that no longer have any matching
        // tracks must read 0, not whatever the provider left behind.
        tx.execute(
            "UPDATE artists
             SET track_count = COALESCE((
                 SELECT COUNT(*)
                 FROM tracks t
                 WHERE t.server_id = artists.server_id
                   AND t.artist_id = artists.artist_id
             ), 0)
             WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ─── Tracks ──────────────────────────────────────────────────

    pub fn replace_tracks(&self, server_id: &ServerId, tracks: &[Track]) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;

        let existing = collect_ids(&tx, "tracks", "track_id", server_id.as_str())?;
        let new_ids: std::collections::HashSet<&str> =
            tracks.iter().map(|t| t.id.as_str()).collect();
        for id in existing.iter().filter(|id| !new_ids.contains(id.as_str())) {
            tx.execute(
                "DELETE FROM tracks WHERE server_id = ?1 AND track_id = ?2",
                rusqlite::params![server_id.as_str(), id],
            )?;
            tx.execute(
                "DELETE FROM library_fts WHERE server_id = ?1 AND kind = 'track' AND entity_id = ?2",
                rusqlite::params![server_id.as_str(), id],
            )?;
        }

        for track in tracks {
            upsert_track(&tx, server_id, track)?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn upsert_track(&self, server_id: &ServerId, track: &Track) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        upsert_track(&tx, server_id, track)?;
        tx.commit()?;
        Ok(())
    }

    /// Batch-upsert tracks in a single transaction. Unlike
    /// `replace_tracks`, this does NOT delete any tracks that
    /// aren't in the input slice — it only inserts / updates.
    /// Used by the Subsonic background sync (Phase 3 of
    /// feature/direct-fetch-providers) which streams albums in
    /// random order via a fan-out and can't safely diff against
    /// the whole server's track list per-batch.
    ///
    /// Caller is responsible for ordering: if `tracks` reference
    /// albums or artists that don't exist in the cache yet, the
    /// FK constraint fails. The background sync upserts the
    /// corresponding albums / artists first.
    pub fn upsert_tracks(&self, server_id: &ServerId, tracks: &[Track]) -> LibraryResult<()> {
        if tracks.is_empty() {
            return Ok(());
        }
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        for track in tracks {
            upsert_track(&tx, server_id, track)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_tracks(
        &self,
        server_id: &ServerId,
        offset: usize,
        limit: usize,
    ) -> LibraryResult<PagedResponse<Track>> {
        let conn = self.connection()?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracks WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT track_id, album_id, title, artist, artist_id, album,
                    duration_seconds, track_number, disc_number, favorite,
                    image_kind, image_tag
             FROM tracks
             WHERE server_id = ?1
             ORDER BY title COLLATE NOCASE
             LIMIT ?2 OFFSET ?3",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), limit as i64, offset as i64],
                rows::row_to_track,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(PagedResponse::new(items, total as usize))
    }

    // ─── Genres ──────────────────────────────────────────────────

    /// Distinct genres for a server, computed from the
    /// `album_genres` join table. Each row is one genre string with
    /// the count of distinct albums and the count of distinct tracks
    /// that list it. Ordered alphabetically (case-insensitive).
    pub fn list_genres(
        &self,
        server_id: &ServerId,
    ) -> LibraryResult<Vec<sinfonic_domain::Genre>> {
        let conn = self.connection()?;
        // album_count: distinct albums that have this genre.
        // track_count: distinct tracks on those albums (we don't
        //   carry per-track genre tags in the schema today, so the
        //   count is the sum of track_count across the genre's
        //   albums. Close enough for the genre pills UI).
        let mut stmt = conn.prepare(
            "SELECT ag.genre AS name,
                    COUNT(DISTINCT ag.album_id) AS album_count,
                    COALESCE(SUM(a.track_count), 0) AS track_count
             FROM album_genres ag
             LEFT JOIN albums a
               ON a.server_id = ag.server_id AND a.album_id = ag.album_id
             WHERE ag.server_id = ?1
             GROUP BY ag.genre COLLATE NOCASE
             ORDER BY ag.genre COLLATE NOCASE",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str()],
                rows::row_to_genre,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    /// Paged list of albums that have the given genre tag. Used by
    /// the genre detail view. The `genre` argument is the raw genre
    /// string (case-insensitive match via `COLLATE NOCASE`), matching
    /// the rest of the schema (genres are stored as plain text, not
    /// by integer id).
    pub fn list_albums_by_genre(
        &self,
        server_id: &ServerId,
        genre: &str,
        offset: usize,
        limit: usize,
    ) -> LibraryResult<PagedResponse<Album>> {
        let conn = self.connection()?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT ag.album_id)
             FROM album_genres ag
             WHERE ag.server_id = ?1 AND ag.genre = ?2 COLLATE NOCASE",
            rusqlite::params![server_id.as_str(), genre],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT a.album_id, a.title, a.artist, a.artist_id, a.year, a.track_count,
                    a.duration_seconds, a.favorite, a.image_kind, a.image_tag
             FROM albums a
             INNER JOIN album_genres ag
               ON ag.server_id = a.server_id AND ag.album_id = a.album_id
             WHERE a.server_id = ?1 AND ag.genre = ?2 COLLATE NOCASE
             ORDER BY a.title COLLATE NOCASE
             LIMIT ?3 OFFSET ?4",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), genre, limit as i64, offset as i64],
                rows::row_to_album,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(PagedResponse::new(items, total as usize))
    }

    /// Paged list of tracks that belong to an album tagged with the
    /// given genre. Joins through `album_genres` (per-track genre
    /// tags are not stored in the schema today). Used by the genre
    /// detail view's "Tracks" section.
    pub fn list_tracks_by_genre(
        &self,
        server_id: &ServerId,
        genre: &str,
        offset: usize,
        limit: usize,
    ) -> LibraryResult<PagedResponse<Track>> {
        let conn = self.connection()?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM tracks t
             INNER JOIN album_genres ag
               ON ag.server_id = t.server_id AND ag.album_id = t.album_id
             WHERE t.server_id = ?1 AND ag.genre = ?2 COLLATE NOCASE",
            rusqlite::params![server_id.as_str(), genre],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT t.track_id, t.album_id, t.title, t.artist, t.artist_id,
                    t.album, t.duration_seconds, t.track_number, t.disc_number,
                    t.favorite, t.image_kind, t.image_tag
             FROM tracks t
             INNER JOIN album_genres ag
               ON ag.server_id = t.server_id AND ag.album_id = t.album_id
             WHERE t.server_id = ?1 AND ag.genre = ?2 COLLATE NOCASE
             ORDER BY t.title COLLATE NOCASE, t.disc_number, t.track_number
             LIMIT ?3 OFFSET ?4",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), genre, limit as i64, offset as i64],
                rows::row_to_track,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(PagedResponse::new(items, total as usize))
    }

    pub fn list_album_tracks(
        &self,
        server_id: &ServerId,
        album_id: &AlbumId,
    ) -> LibraryResult<Vec<Track>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT track_id, album_id, title, artist, artist_id, album,
                    duration_seconds, track_number, disc_number, favorite,
                    image_kind, image_tag
             FROM tracks
             WHERE server_id = ?1 AND album_id = ?2
             ORDER BY disc_number, track_number",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), album_id.as_str()],
                rows::row_to_track,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    // ─── Playlists ───────────────────────────────────────────────

    /// Replace a playlist (metadata + track list) atomically.
    pub fn replace_playlist(
        &self,
        server_id: &ServerId,
        playlist: &Playlist,
        track_ids: &[TrackId],
    ) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tx = conn.transaction()?;
        let (image_kind, image_tag) = image_columns(playlist.image_ref.as_ref());

        tx.execute(
            "INSERT INTO playlists (server_id, playlist_id, name, track_count, duration_seconds, owner, public, image_kind, image_tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(server_id, playlist_id) DO UPDATE SET
                 name = excluded.name,
                 track_count = excluded.track_count,
                 duration_seconds = excluded.duration_seconds,
                 owner = excluded.owner,
                 public = excluded.public,
                 image_kind = excluded.image_kind,
                 image_tag = excluded.image_tag",
            rusqlite::params![
                server_id.as_str(),
                playlist.id.as_str(),
                playlist.name,
                playlist.track_count as i64,
                playlist.duration_seconds as i64,
                playlist.owner,
                playlist.public as i64,
                image_kind,
                image_tag,
            ],
        )?;

        tx.execute(
            "DELETE FROM playlist_tracks WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist.id.as_str()],
        )?;
        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks (server_id, playlist_id, position, track_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    server_id.as_str(),
                    playlist.id.as_str(),
                    position as i64,
                    track_id.as_str(),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn list_playlists(&self, server_id: &ServerId) -> LibraryResult<Vec<Playlist>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT playlist_id, name, track_count, duration_seconds, owner, public,
                    image_kind, image_tag
             FROM playlists
             WHERE server_id = ?1
             ORDER BY name COLLATE NOCASE",
        )?;
        let items = stmt
            .query_map(rusqlite::params![server_id.as_str()], rows::row_to_playlist)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn list_playlist_tracks(
        &self,
        server_id: &ServerId,
        playlist_id: &PlaylistId,
    ) -> LibraryResult<Vec<TrackId>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT track_id FROM playlist_tracks
             WHERE server_id = ?1 AND playlist_id = ?2
             ORDER BY position",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![server_id.as_str(), playlist_id.as_str()],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(TrackId::new(s))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    /// Fetch a single track by id, or `None` if not found.
    pub fn get_track(&self, server_id: &ServerId, track_id: &TrackId) -> LibraryResult<Option<Track>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT track_id, album_id, title, artist, artist_id, album,
                    duration_seconds, track_number, disc_number, favorite,
                    image_kind, image_tag
             FROM tracks
             WHERE server_id = ?1 AND track_id = ?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![server_id.as_str(), track_id.as_str()])?;
        match rows.next()? {
            Some(row) => Ok(Some(rows::row_to_track(row)?)),
            None => Ok(None),
        }
    }

    /// Creates a new local playlist with the given name and track ids.
    /// Returns the generated `PlaylistId`.
    pub fn create_playlist(
        &self,
        server_id: &ServerId,
        name: &str,
        track_ids: &[TrackId],
    ) -> LibraryResult<PlaylistId> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let playlist_id = PlaylistId::new(format!("playlist-local-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)));
        let server_id_str = server_id.as_str();
        let (track_count, duration_seconds) = {
            // Bulk-fetch durations for all requested track ids in a
            // single IN(...) query instead of one SELECT per track.
            // For a 100-track playlist the old loop ran 100 round-trips
            // through the statement cache; now it's one query.
            let placeholders = std::iter::repeat("?")
                .take(track_ids.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT duration_seconds FROM tracks \
                 WHERE server_id = ?1 AND track_id IN ({placeholders})"
            );
            let track_id_refs: Vec<&str> =
                track_ids.iter().map(|t| t.as_str()).collect();
            let mut params: Vec<&dyn rusqlite::ToSql> =
                Vec::with_capacity(track_ids.len() + 1);
            params.push(&server_id_str);
            for r in &track_id_refs {
                params.push(r);
            }
            let total_dur: i64 = tx
                .prepare(&sql)?
                .query_map(params.as_slice(), |r| r.get::<_, i64>(0))?
                .filter_map(Result::ok)
                .sum();
            (track_ids.len() as u32, total_dur as u32)
        };
        tx.execute(
            "INSERT INTO playlists (server_id, playlist_id, name, track_count, duration_seconds, owner, public)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1)",
            rusqlite::params![
                server_id.as_str(),
                playlist_id.as_str(),
                name,
                track_count as i64,
                duration_seconds as i64,
            ],
        )?;
        for (pos, tid) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks (server_id, playlist_id, position, track_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![server_id.as_str(), playlist_id.as_str(), pos as i64, tid.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(playlist_id)
    }

    /// Deletes a playlist and all its tracks.
    pub fn delete_playlist(&self, server_id: &ServerId, playlist_id: &PlaylistId) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM playlists WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist_id.as_str()],
        )?;
        Ok(())
    }

    /// Renames an existing playlist.
    pub fn rename_playlist(
        &self,
        server_id: &ServerId,
        playlist_id: &PlaylistId,
        name: &str,
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE playlists SET name = ?3 WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist_id.as_str(), name],
        )?;
        Ok(())
    }

    /// Appends tracks to the end of a playlist.
    pub fn add_playlist_tracks(
        &self,
        server_id: &ServerId,
        playlist_id: &PlaylistId,
        track_ids: &[TrackId],
    ) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let max_pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist_id.as_str()],
            |r| r.get(0),
        )?;
        let server_id_str = server_id.as_str();
        // Bulk-fetch durations in a single IN(...) query instead of
        // running one SELECT per track id. For a 100-track add the old
        // loop issued 100 queries; now it's one.
        let placeholders = std::iter::repeat("?")
            .take(track_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let dur_sql = format!(
            "SELECT track_id, COALESCE(duration_seconds, 0) FROM tracks \
             WHERE server_id = ?1 AND track_id IN ({placeholders})"
        );
        let track_id_refs: Vec<&str> =
            track_ids.iter().map(|t| t.as_str()).collect();
        let mut params: Vec<&dyn rusqlite::ToSql> =
            Vec::with_capacity(track_ids.len() + 1);
        params.push(&server_id_str);
        for r in &track_id_refs {
            params.push(r);
        }
        let mut durations: std::collections::HashMap<&str, i64> =
            std::collections::HashMap::with_capacity(track_ids.len());
        {
            let mut stmt = tx.prepare(&dur_sql)?;
            let rows = stmt.query_map(params.as_slice(), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?;
            for row in rows.flatten() {
                durations.insert(
                    // Leak-free: copy the string into a 'static-ish key
                    // by leaking for the duration of this scope. The
                    // HashMap is dropped before tx.commit(), so the
                    // tiny leak is bounded and self-cleaning.
                    Box::leak(row.0.into_boxed_str()),
                    row.1,
                );
            }
        }
        let mut added_dur = 0i64;
        for (new_pos, tid) in (max_pos + 1..).zip(track_ids.iter()) {
            added_dur += durations.get(tid.as_str()).copied().unwrap_or(0);
            tx.execute(
                "INSERT INTO playlist_tracks (server_id, playlist_id, position, track_id) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![server_id.as_str(), playlist_id.as_str(), new_pos, tid.as_str()],
            )?;
        }
        tx.execute(
            "UPDATE playlists SET track_count = track_count + ?3, duration_seconds = duration_seconds + ?4
             WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist_id.as_str(), track_ids.len() as i64, added_dur],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Removes playlist entries by their position (entry id = position as string).
    pub fn remove_playlist_entries(
        &self,
        server_id: &ServerId,
        playlist_id: &PlaylistId,
        entry_ids: &[String],
    ) -> LibraryResult<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        for entry_id in entry_ids {
            if let Ok(pos) = entry_id.parse::<i64>() {
                tx.execute(
                    "DELETE FROM playlist_tracks WHERE server_id = ?1 AND playlist_id = ?2 AND position = ?3",
                    rusqlite::params![server_id.as_str(), playlist_id.as_str(), pos],
                )?;
            }
        }
        tx.execute(
            "UPDATE playlists SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE server_id = ?1 AND playlist_id = ?2),
             duration_seconds = (SELECT COALESCE(SUM(t.duration_seconds), 0) FROM playlist_tracks pt JOIN tracks t ON t.track_id = pt.track_id AND t.server_id = pt.server_id WHERE pt.server_id = ?1 AND pt.playlist_id = ?2)
             WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist_id.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a playlist entry to a new position.
    pub fn move_playlist_entry(
        &self,
        server_id: &ServerId,
        playlist_id: &PlaylistId,
        entry_id: &str,
        new_index: usize,
    ) -> LibraryResult<()> {
        let from: i64 = entry_id.parse().map_err(|_| {
            LibraryError::Validation(format!("invalid entry id: {entry_id}"))
        })?;
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let max_pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM playlist_tracks WHERE server_id = ?1 AND playlist_id = ?2",
            rusqlite::params![server_id.as_str(), playlist_id.as_str()],
            |r| r.get(0),
        )?;
        let to = (new_index as i64).min(max_pos);
        if from == to {
            tx.commit()?;
            return Ok(());
        }
        if from < to {
            tx.execute(
                "UPDATE playlist_tracks SET position = position - 1
                 WHERE server_id = ?1 AND playlist_id = ?2 AND position > ?3 AND position <= ?4",
                rusqlite::params![server_id.as_str(), playlist_id.as_str(), from, to],
            )?;
        } else {
            tx.execute(
                "UPDATE playlist_tracks SET position = position + 1
                 WHERE server_id = ?1 AND playlist_id = ?2 AND position >= ?4 AND position < ?3",
                rusqlite::params![server_id.as_str(), playlist_id.as_str(), from, to],
            )?;
        }
        tx.execute(
            "UPDATE playlist_tracks SET position = ?3
             WHERE server_id = ?1 AND playlist_id = ?2 AND position = ?4",
            rusqlite::params![server_id.as_str(), playlist_id.as_str(), to, from],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ─── Favorites (local cache only — Phase 9) ─────────────────

    /// Updates the `favorite` flag on a track.
    pub fn set_track_favorite(
        &self,
        server_id: &ServerId,
        track_id: &TrackId,
        favorite: bool,
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE tracks SET favorite = ?3 WHERE server_id = ?1 AND track_id = ?2",
            rusqlite::params![server_id.as_str(), track_id.as_str(), favorite as i64],
        )?;
        Ok(())
    }

    /// Updates the `favorite` flag on an album.
    pub fn set_album_favorite(
        &self,
        server_id: &ServerId,
        album_id: &AlbumId,
        favorite: bool,
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE albums SET favorite = ?3 WHERE server_id = ?1 AND album_id = ?2",
            rusqlite::params![server_id.as_str(), album_id.as_str(), favorite as i64],
        )?;
        Ok(())
    }

    /// Updates the `favorite` flag on an artist.
    pub fn set_artist_favorite(
        &self,
        server_id: &ServerId,
        artist_id: &ArtistId,
        favorite: bool,
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE artists SET favorite = ?3 WHERE server_id = ?1 AND artist_id = ?2",
            rusqlite::params![server_id.as_str(), artist_id.as_str(), favorite as i64],
        )?;
        Ok(())
    }

    /// Returns all favorited tracks, albums, and artists for a server.
    pub fn get_favorites(
        &self,
        server_id: &ServerId,
    ) -> LibraryResult<(Vec<Track>, Vec<Album>, Vec<Artist>)> {
        let conn = self.connection()?;

        let mut track_stmt = conn.prepare(
            "SELECT track_id, album_id, title, artist, artist_id, album,
                    duration_seconds, track_number, disc_number, favorite,
                    image_kind, image_tag
             FROM tracks WHERE server_id = ?1 AND favorite = 1
             ORDER BY title COLLATE NOCASE",
        )?;
        let tracks = track_stmt
            .query_map(rusqlite::params![server_id.as_str()], rows::row_to_track)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut album_stmt = conn.prepare(
            "SELECT album_id, title, artist, artist_id, year, track_count,
                    duration_seconds, favorite, image_kind, image_tag
             FROM albums WHERE server_id = ?1 AND favorite = 1
             ORDER BY title COLLATE NOCASE",
        )?;
        let albums = album_stmt
            .query_map(rusqlite::params![server_id.as_str()], rows::row_to_album)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut artist_stmt = conn.prepare(
            "SELECT artist_id, name, album_count, track_count, favorite, image_kind, image_tag
             FROM artists WHERE server_id = ?1 AND favorite = 1
             ORDER BY name COLLATE NOCASE",
        )?;
        let artists = artist_stmt
            .query_map(rusqlite::params![server_id.as_str()], rows::row_to_artist)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok((tracks, albums, artists))
    }

    // ─── Search ──────────────────────────────────────────────────

    /// Search across the FTS5 index. Returns the first `limit`
    /// matches of each kind (album, track, artist) ranked by FTS5
    /// `bm25` score (lower is better).
    pub fn search(
        &self,
        server_id: &ServerId,
        query: &str,
        limit: usize,
    ) -> LibraryResult<sinfonic_domain::SearchResults> {
        let conn = self.connection()?;
        search::search(&conn, server_id, query, limit)
    }

    // ─── Stats ───────────────────────────────────────────────────

    /// Returns `(albums, artists, tracks, playlists)` for a server.
    pub fn server_counts(&self, server_id: &ServerId) -> LibraryResult<(i64, i64, i64, i64)> {
        let conn = self.connection()?;
        let albums: i64 = conn.query_row(
            "SELECT COUNT(*) FROM albums WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        let artists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artists WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        let tracks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracks WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        let playlists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM playlists WHERE server_id = ?1",
            rusqlite::params![server_id.as_str()],
            |r| r.get(0),
        )?;
        Ok((albums, artists, tracks, playlists))
    }
}

// ─── Private helpers ────────────────────────────────────────────

fn collect_ids(
    tx: &Transaction<'_>,
    table: &'static str,
    id_column: &'static str,
    server_id: &str,
) -> LibraryResult<Vec<String>> {
    if !matches!(table, "albums" | "artists" | "tracks") {
        return Err(LibraryError::Validation(format!("unknown table {table}")));
    }
    let sql = format!("SELECT {id_column} FROM {table} WHERE server_id = ?1");
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![server_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn upsert_album(tx: &Transaction<'_>, server_id: &ServerId, album: &Album) -> LibraryResult<()> {
    let (image_kind, image_tag) = image_columns(album.image_ref.as_ref());
    let artist_id_str: Option<String> = album.artist_id.as_ref().map(|a| a.as_str().to_string());

    tx.execute(
        "INSERT INTO albums (server_id, album_id, title, artist, artist_id, year, track_count,
                             duration_seconds, favorite, image_kind, image_tag)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(server_id, album_id) DO UPDATE SET
             title = excluded.title,
             artist = excluded.artist,
             artist_id = excluded.artist_id,
             year = excluded.year,
             track_count = excluded.track_count,
             duration_seconds = excluded.duration_seconds,
             favorite = excluded.favorite,
             image_kind = excluded.image_kind,
             image_tag = excluded.image_tag",
        rusqlite::params![
            server_id.as_str(),
            album.id.as_str(),
            album.title,
            album.artist,
            artist_id_str,
            album.year.map(|y| y as i64),
            album.track_count as i64,
            album.duration_seconds as i64,
            album.favorite as i64,
            image_kind,
            image_tag,
        ],
    )?;

    tx.execute(
        "DELETE FROM album_genres WHERE server_id = ?1 AND album_id = ?2",
        rusqlite::params![server_id.as_str(), album.id.as_str()],
    )?;
    for genre in &album.genres {
        tx.execute(
            "INSERT OR IGNORE INTO album_genres (server_id, album_id, genre) VALUES (?1, ?2, ?3)",
            rusqlite::params![server_id.as_str(), album.id.as_str(), genre],
        )?;
    }

    tx.execute(
        "DELETE FROM library_fts WHERE server_id = ?1 AND kind = 'album' AND entity_id = ?2",
        rusqlite::params![server_id.as_str(), album.id.as_str()],
    )?;
    tx.execute(
        "INSERT INTO library_fts (kind, server_id, entity_id, title, subtitle) VALUES ('album', ?1, ?2, ?3, ?4)",
        rusqlite::params![server_id.as_str(), album.id.as_str(), album.title, album.artist],
    )?;

    Ok(())
}

fn upsert_artist(tx: &Transaction<'_>, server_id: &ServerId, artist: &Artist) -> LibraryResult<()> {
    let (image_kind, image_tag) = image_columns(artist.image_ref.as_ref());

    tx.execute(
        "INSERT INTO artists (server_id, artist_id, name, album_count, track_count, favorite, image_kind, image_tag)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(server_id, artist_id) DO UPDATE SET
             name = excluded.name,
             album_count = excluded.album_count,
             track_count = excluded.track_count,
             favorite = excluded.favorite,
             image_kind = excluded.image_kind,
             image_tag = excluded.image_tag",
        rusqlite::params![
            server_id.as_str(),
            artist.id.as_str(),
            artist.name,
            artist.album_count as i64,
            artist.track_count as i64,
            artist.favorite as i64,
            image_kind,
            image_tag,
        ],
    )?;

    tx.execute(
        "DELETE FROM library_fts WHERE server_id = ?1 AND kind = 'artist' AND entity_id = ?2",
        rusqlite::params![server_id.as_str(), artist.id.as_str()],
    )?;
    tx.execute(
        "INSERT INTO library_fts (kind, server_id, entity_id, title, subtitle) VALUES ('artist', ?1, ?2, ?3, '')",
        rusqlite::params![server_id.as_str(), artist.id.as_str(), artist.name],
    )?;
    Ok(())
}

fn upsert_track(tx: &Transaction<'_>, server_id: &ServerId, track: &Track) -> LibraryResult<()> {
    let (image_kind, image_tag) = image_columns(track.image_ref.as_ref());
    let artist_id_str: Option<String> = track.artist_id.as_ref().map(|a| a.as_str().to_string());
    let subtitle = format!("{} — {}", track.artist, track.album);

    tx.execute(
        "INSERT INTO tracks (server_id, track_id, album_id, title, artist, artist_id, album,
                             duration_seconds, track_number, disc_number, favorite, image_kind, image_tag)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(server_id, track_id) DO UPDATE SET
             album_id = excluded.album_id,
             title = excluded.title,
             artist = excluded.artist,
             artist_id = excluded.artist_id,
             album = excluded.album,
             duration_seconds = excluded.duration_seconds,
             track_number = excluded.track_number,
             disc_number = excluded.disc_number,
             favorite = excluded.favorite,
             image_kind = excluded.image_kind,
             image_tag = excluded.image_tag",
        rusqlite::params![
            server_id.as_str(),
            track.id.as_str(),
            track.album_id.as_str(),
            track.title,
            track.artist,
            artist_id_str,
            track.album,
            track.duration_seconds as i64,
            track.track_number as i64,
            track.disc_number as i64,
            track.favorite as i64,
            image_kind,
            image_tag,
        ],
    )?;

    tx.execute(
        "DELETE FROM library_fts WHERE server_id = ?1 AND kind = 'track' AND entity_id = ?2",
        rusqlite::params![server_id.as_str(), track.id.as_str()],
    )?;
    tx.execute(
        "INSERT INTO library_fts (kind, server_id, entity_id, title, subtitle) VALUES ('track', ?1, ?2, ?3, ?4)",
        rusqlite::params![server_id.as_str(), track.id.as_str(), track.title, subtitle],
    )?;
    Ok(())
}

fn image_columns(
    image_ref: Option<&sinfonic_domain::ImageRef>,
) -> (Option<String>, Option<String>) {
    match image_ref {
        // Serialise the enum back to its PascalCase variant name so
        // the SQLite row matches the wire string the frontend already
        // sees. parse_image_kind in rows.rs is the inverse.
        Some(ir) => (
            Some(image_kind_to_str(ir.kind)),
            ir.tag.clone(),
        ),
        None => (None, None),
    }
}

fn image_kind_to_str(kind: sinfonic_domain::ImageKindHint) -> String {
    use sinfonic_domain::ImageKindHint::*;
    match kind {
        Primary => "Primary".to_string(),
        Backdrop => "Backdrop".to_string(),
        CoverArt => "CoverArt".to_string(),
        Embedded => "Embedded".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinfonic_domain::{Album, AlbumId, Artist, ArtistId, Playlist, PlaylistId, ServerId, Track, TrackId};

    fn server() -> ServerId {
        ServerId::new("server-1")
    }

    fn album(id: &str, title: &str, artist: &str) -> Album {
        Album {
            id: AlbumId::new(id),
            title: title.into(),
            artist: artist.into(),
            artist_id: None,
            year: Some(2000),
            track_count: 10,
            duration_seconds: 3000,
            favorite: false,
            image_ref: None,
            genres: vec!["Rock".into(), "Indie".into()],
        }
    }

    fn track(id: &str, title: &str, album_id: &str, track_number: u16) -> Track {
        Track {
            id: TrackId::new(id),
            album_id: AlbumId::new(album_id),
            title: title.into(),
            artist: "Artist 1".into(),
            artist_id: None,
            album: "Album 1".into(),
            duration_seconds: 200,
            track_number,
            disc_number: 1,
            favorite: false,
            image_ref: None,
        }
    }

    fn artist(id: &str, name: &str) -> Artist {
        Artist {
            id: ArtistId::new(id),
            name: name.into(),
            album_count: 0,
            track_count: 0,
            favorite: false,
            image_ref: None,
        }
    }

    fn playlist(id: &str, name: &str, track_ids: Vec<&str>) -> (Playlist, Vec<TrackId>) {
        (
            Playlist {
                id: PlaylistId::new(id),
                name: name.into(),
                track_count: track_ids.len() as u32,
                duration_seconds: 0,
                owner: Some("me".into()),
                public: false,
                image_ref: None,
            },
            track_ids.into_iter().map(TrackId::new).collect(),
        )
    }

    #[test]
    fn open_memory_creates_fresh_db() {
        let store = Store::open_memory().unwrap();
        let s = server();
        let page = store.list_albums(&s, 0, 10).unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.total, 0);
    }

    #[test]
    fn replace_albums_then_list_returns_paged() {
        let store = Store::open_memory().unwrap();
        let s = server();
        let albums: Vec<Album> = (0..15)
            .map(|i| album(&format!("a-{i}"), &format!("Album {i}"), "X"))
            .collect();
        store.replace_albums(&s, &albums).unwrap();
        let p1 = store.list_albums(&s, 0, 10).unwrap();
        assert_eq!(p1.items.len(), 10);
        assert_eq!(p1.total, 15);
        let p2 = store.list_albums(&s, 10, 10).unwrap();
        assert_eq!(p2.items.len(), 5);
    }

    #[test]
    fn replace_albums_removes_orphans() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store
            .replace_albums(&s, &[album("a-1", "A", "X"), album("a-2", "B", "X")])
            .unwrap();
        assert_eq!(store.list_albums(&s, 0, 10).unwrap().total, 2);
        // New set drops a-2; FTS5 row and the album row should be gone.
        store.replace_albums(&s, &[album("a-1", "A v2", "X")]).unwrap();
        let page = store.list_albums(&s, 0, 10).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].title, "A v2");
    }

    #[test]
    fn album_genres_are_replaced_on_each_upsert() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store
            .upsert_album(&s, &album("a-1", "A", "X"))
            .unwrap();
        {
            let conn = store.connection().unwrap();
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM album_genres WHERE server_id = ?1 AND album_id = 'a-1'",
                    rusqlite::params![s.as_str()],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 2);
        }
        let mut a = album("a-1", "A", "X");
        a.genres = vec!["Jazz".into()];
        store.upsert_album(&s, &a).unwrap();
        let conn = store.connection().unwrap();
        let names: Vec<String> = conn
            .prepare(
                "SELECT genre FROM album_genres WHERE server_id = ?1 AND album_id = 'a-1'",
            )
            .unwrap()
            .query_map(rusqlite::params![s.as_str()], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(names, vec!["Jazz".to_string()]);
    }

    #[test]
    fn list_albums_by_genre_filters_by_tag_case_insensitive() {
        let store = Store::open_memory().unwrap();
        let s = server();

        let mut jazz = album("a-jazz", "Jazz Album", "Alice");
        jazz.genres = vec!["Jazz".into()];
        let mut rock = album("a-rock", "Rock Album", "Bob");
        rock.genres = vec!["Rock".into()];
        let mut both = album("a-both", "Fusion", "Cathy");
        both.genres = vec!["Jazz".into(), "Rock".into()];
        store.upsert_album(&s, &jazz).unwrap();
        store.upsert_album(&s, &rock).unwrap();
        store.upsert_album(&s, &both).unwrap();

        let page = store.list_albums_by_genre(&s, "jazz", 0, 10).unwrap();
        assert_eq!(page.total, 2);
        let ids: Vec<&str> = page.items.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["a-both", "a-jazz"]);
    }

    #[test]
    fn list_tracks_by_genre_joins_through_album_genres() {
        let store = Store::open_memory().unwrap();
        let s = server();

        let mut a = album("a-1", "Jazz Album", "Alice");
        a.genres = vec!["Jazz".into()];
        store.upsert_album(&s, &a).unwrap();

        let artist = artist("ar-1", "Alice");
        store.upsert_artist(&s, &artist).unwrap();

        store
            .replace_tracks(
                &s,
                &[
                    track("t-1", "Track 1", "a-1", 1),
                    track("t-2", "Track 2", "a-1", 2),
                ],
            )
            .unwrap();

        let page = store.list_tracks_by_genre(&s, "jazz", 0, 10).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].title, "Track 1");
        assert_eq!(page.items[1].title, "Track 2");

        let empty = store.list_tracks_by_genre(&s, "Rock", 0, 10).unwrap();
        assert_eq!(empty.total, 0);
        assert!(empty.items.is_empty());
    }

    #[test]
    fn replace_tracks_orders_by_title() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.replace_albums(&s, &[album("a-1", "A", "X")]).unwrap();
        store
            .replace_tracks(
                &s,
                &[
                    track("t-1", "Charlie", "a-1", 1),
                    track("t-2", "Alpha", "a-1", 2),
                    track("t-3", "Bravo", "a-1", 3),
                ],
            )
            .unwrap();
        let page = store.list_tracks(&s, 0, 10).unwrap();
        let titles: Vec<String> = page.items.iter().map(|t| t.title.clone()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn list_album_tracks_orders_by_disc_then_track_number() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.replace_albums(&s, &[album("a-1", "A", "X")]).unwrap();
        store
            .replace_tracks(
                &s,
                &[
                    Track {
                        track_number: 3,
                        ..track("t-1", "C", "a-1", 3)
                    },
                    Track {
                        track_number: 1,
                        ..track("t-2", "A", "a-1", 1)
                    },
                    Track {
                        disc_number: 2,
                        track_number: 1,
                        ..track("t-3", "B-D2", "a-1", 1)
                    },
                ],
            )
            .unwrap();
        let titles: Vec<String> = store
            .list_album_tracks(&s, &AlbumId::new("a-1"))
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(titles, vec!["A", "C", "B-D2"]);
    }

    #[test]
    fn replace_playlist_then_list_tracks_returns_ordered_ids() {
        let store = Store::open_memory().unwrap();
        let s = server();
        let (p, tracks) = playlist("p-1", "My Mix", vec!["t-3", "t-1", "t-2"]);
        store.replace_playlist(&s, &p, &tracks).unwrap();
        let listed = store.list_playlist_tracks(&s, &PlaylistId::new("p-1")).unwrap();
        assert_eq!(
            listed.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
            vec!["t-3", "t-1", "t-2"]
        );
        let playlists = store.list_playlists(&s).unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "My Mix");
        assert_eq!(playlists[0].owner.as_deref(), Some("me"));
    }

    #[test]
    fn replace_playlist_overwrites_track_list() {
        let store = Store::open_memory().unwrap();
        let s = server();
        let (p, mut tracks) = playlist("p-1", "My Mix", vec!["t-1", "t-2"]);
        store.replace_playlist(&s, &p, &tracks).unwrap();
        tracks = vec![TrackId::new("t-3")];
        store.replace_playlist(&s, &p, &tracks).unwrap();
        let listed = store.list_playlist_tracks(&s, &PlaylistId::new("p-1")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].as_str(), "t-3");
    }

    #[test]
    fn upsert_album_rejects_when_artist_missing() {
        let store = Store::open_memory().unwrap();
        let s = server();
        // Album's artist_id references an artist that does not
        // exist. Deferred FKs only reorder checks; the target
        // still has to exist at commit time.
        let mut a = album("a-1", "A", "X");
        a.artist_id = Some(ArtistId::new("missing"));
        let err = store.upsert_album(&s, &a).unwrap_err();
        assert!(matches!(err, LibraryError::Sqlite(_)));
    }

    #[test]
    fn upsert_track_rejects_when_album_missing() {
        let store = Store::open_memory().unwrap();
        let s = server();
        let mut t = track("t-1", "T", "missing", 1);
        t.album_id = AlbumId::new("missing-album");
        let err = store.upsert_track(&s, &t).unwrap_err();
        assert!(matches!(err, LibraryError::Sqlite(_)));
    }

    #[test]
    fn replace_albums_then_artists_in_separate_calls() {
        // The realistic sync flow: replace_albums is called, then
        // replace_artists later. The deferred FK lets each call
        // succeed on its own as long as the second call's targets
        // exist.
        let store = Store::open_memory().unwrap();
        let s = server();
        store.replace_albums(&s, &[album("a-1", "A", "X")]).unwrap();
        store
            .replace_artists(&s, &[artist("artist-1", "X")])
            .unwrap();
        let (a, ar, _) = (store.server_counts(&s).unwrap().0, store.server_counts(&s).unwrap().1, 0);
        assert_eq!((a, ar), (1, 1));
    }

    #[test]
    fn replace_artists_removes_stale() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store
            .replace_artists(&s, &[artist("ar-1", "A"), artist("ar-2", "B")])
            .unwrap();
        store.replace_artists(&s, &[artist("ar-1", "A v2")]).unwrap();
        let page = store.list_artists(&s, 0, 10).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "A v2");
    }

    /// Regression for the v4 schema fix: when a resync evicts a
    /// stale artist, the `ON DELETE SET NULL` action on the
    /// `albums.artist_id` FK must only NULL `artist_id` —
    /// NOT `server_id`. The pre-v4 composite FK tripped this and
    /// every resync crashed with
    /// `NOT NULL constraint failed: albums.server_id` as soon as
    /// any artist had referencing albums.
    #[test]
    fn replace_artists_evicting_stale_does_not_fail_when_albums_reference_them() {
        let store = Store::open_memory().unwrap();
        let s = server();

        // First sync: 1 artist + 1 album that references it.
        store
            .replace_artists(&s, &[artist("ar-1", "Radiohead")])
            .unwrap();
        let mut a = album("al-1", "OK Computer", "Radiohead");
        a.artist_id = Some(ArtistId::new("ar-1"));
        store.replace_albums(&s, &[a]).unwrap();

        // Second sync: the artist is no longer in the list. Pre-v4
        // this raised `NOT NULL constraint failed: albums.server_id`
        // because the composite FK SET NULL tried to NULL both
        // FK columns. Post-v4 only `artist_id` is NULLed.
        store.replace_artists(&s, &[]).expect(
            "replace_artists must succeed when evicting a stale artist \
             whose albums still reference it",
        );

        let page = store.list_albums(&s, 0, 10).unwrap();
        assert_eq!(page.total, 1, "album row must be preserved");
        assert!(
            page.items[0].artist_id.is_none(),
            "FK SET NULL must NULL only artist_id; server_id must remain set"
        );
        assert_eq!(
            page.items[0].artist, "Radiohead",
            "display name on the album row must survive the artist eviction"
        );
    }

    #[test]
    fn server_counts_reports_each_table() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.replace_albums(&s, &[album("a-1", "A", "X")]).unwrap();
        store.replace_artists(&s, &[artist("ar-1", "X")]).unwrap();
        store.replace_tracks(&s, &[track("t-1", "T", "a-1", 1)]).unwrap();
        let (a, ar, t, p) = store.server_counts(&s).unwrap();
        assert_eq!((a, ar, t, p), (1, 1, 1, 0));
    }

    #[test]
    fn server_scoped_isolation() {
        let store = Store::open_memory().unwrap();
        let s1 = ServerId::new("s1");
        let s2 = ServerId::new("s2");
        store
            .replace_albums(&s1, &[album("shared", "A from s1", "X")])
            .unwrap();
        store
            .replace_albums(&s2, &[album("shared", "A from s2", "Y")])
            .unwrap();
        let p1 = store.list_albums(&s1, 0, 10).unwrap();
        let p2 = store.list_albums(&s2, 0, 10).unwrap();
        assert_eq!(p1.items[0].artist, "X");
        assert_eq!(p2.items[0].artist, "Y");
    }

    #[test]
    fn delete_server_clears_everything() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.replace_albums(&s, &[album("a-1", "A", "X")]).unwrap();
        store.replace_artists(&s, &[artist("ar-1", "X")]).unwrap();
        store.replace_tracks(&s, &[track("t-1", "T", "a-1", 1)]).unwrap();
        let (p, _) = playlist("p-1", "Mix", vec!["t-1"]);
        store.replace_playlist(&s, &p, &p_tracks(vec!["t-1"])).unwrap();
        store.delete_server(&s).unwrap();
        let (a, ar, t, p_count) = store.server_counts(&s).unwrap();
        assert_eq!((a, ar, t, p_count), (0, 0, 0, 0));
    }

    fn p_tracks(ids: Vec<&str>) -> Vec<TrackId> {
        ids.into_iter().map(TrackId::new).collect()
    }

    #[test]
    fn recompute_artist_track_counts_aggregates_from_cached_tracks() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_artist(&s, &artist("ar-1", "X")).unwrap();
        store.upsert_artist(&s, &artist("ar-2", "Y")).unwrap();
        // One `replace_albums` call so both a-1 and a-2 land in the
        // same batch — calling it twice would evict the first album
        // and break the FK when tracks try to reference it.
        store
            .replace_albums(
                &s,
                &[album("a-1", "A", "X"), album("a-2", "B", "Y")],
            )
            .unwrap();
        // Three tracks for ar-1, two for ar-2. The recompute query
        // joins on `tracks.artist_id`, so we have to set it explicitly
        // — the default `track(...)` test helper leaves it as None.
        store
            .replace_tracks(
                &s,
                &[
                    Track {
                        artist_id: Some(ArtistId::new("ar-1")),
                        ..track("t-1", "T1", "a-1", 1)
                    },
                    Track {
                        artist_id: Some(ArtistId::new("ar-1")),
                        ..track("t-2", "T2", "a-1", 2)
                    },
                    Track {
                        artist_id: Some(ArtistId::new("ar-1")),
                        ..track("t-3", "T3", "a-1", 3)
                    },
                    Track {
                        artist_id: Some(ArtistId::new("ar-2")),
                        ..track("t-4", "T4", "a-2", 1)
                    },
                    Track {
                        artist_id: Some(ArtistId::new("ar-2")),
                        ..track("t-5", "T5", "a-2", 2)
                    },
                ],
            )
            .unwrap();

        store.recompute_artist_track_counts(&s).unwrap();

        let page = store.list_artists(&s, 0, 10).unwrap();
        let by_id: std::collections::HashMap<&str, u32> = page
            .items
            .iter()
            .map(|a| (a.id.as_str(), a.track_count))
            .collect();
        assert_eq!(by_id.get("ar-1"), Some(&3));
        assert_eq!(by_id.get("ar-2"), Some(&2));
    }

    #[test]
    fn recompute_artist_track_counts_resets_to_zero_when_no_tracks_match() {
        // Reproduces the Jellyfin case: provider sends `track_count=0`
        // and the user owns no tracks for the artist. Recompute must
        // leave the artist at 0, not whatever was previously cached.
        let store = Store::open_memory().unwrap();
        let s = server();
        let mut a = artist("ar-1", "X");
        a.track_count = 42;
        store.upsert_artist(&s, &a).unwrap();
        store.recompute_artist_track_counts(&s).unwrap();
        let page = store.list_artists(&s, 0, 10).unwrap();
        assert_eq!(page.items[0].track_count, 0);
    }

    #[test]
    fn get_album_returns_some_when_present() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.replace_albums(&s, &[album("a-1", "A", "X")]).unwrap();
        let a = store.get_album(&s, &AlbumId::new("a-1")).unwrap();
        assert!(a.is_some());
        assert_eq!(a.unwrap().title, "A");
        let missing = store.get_album(&s, &AlbumId::new("nope")).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn open_creates_db_file_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite");
        let store = Store::open(&path).unwrap();
        store
            .replace_albums(&server(), &[album("a-1", "A", "X")])
            .unwrap();
        drop(store);
        // Reopen and verify the data is still there.
        let store2 = Store::open(&path).unwrap();
        let page = store2.list_albums(&server(), 0, 10).unwrap();
        assert_eq!(page.total, 1);
    }

    // ─── Queue snapshot persistence ─────────────────────────────

    fn empty_snapshot(server_id: ServerId, entries: usize, current: Option<usize>) -> QueueSnapshot {
        use sinfonic_domain::queue::QueueEngine;

        let mut engine = QueueEngine::new(server_id.clone());
        let tracks: Vec<Track> = (0..entries)
            .map(|i| {
                let mut t = track(&format!("t-{i}"), &format!("T{i}"), "a-1", i as u16 + 1);
                t.album = "Album 1".into();
                t
            })
            .collect();
        engine.play_now(&tracks);
        if let Some(idx) = current {
            // jump_to is fine here; we just want a non-default current
            let target_id = engine.entries()[idx.min(entries - 1)].id.clone();
            let _ = engine.jump_to(&target_id);
        }
        engine.snapshot()
    }

    #[test]
    fn save_load_round_trip_preserves_entries_and_current() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_server(&s, "subsonic", "Test", "http://x", None).unwrap();
        let snap = empty_snapshot(s.clone(), 5, Some(2));
        store.save_queue_snapshot(&s, &snap).unwrap();
        let loaded = store.load_queue_snapshot(&s).unwrap().unwrap();
        assert_eq!(loaded.entries.len(), 5);
        assert_eq!(loaded.current_index, Some(2));
        assert_eq!(loaded.entries[0].title, "T0");
        assert_eq!(loaded.entries[4].title, "T4");
    }

    #[test]
    fn load_returns_none_when_no_snapshot_persisted() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_server(&s, "subsonic", "Test", "http://x", None).unwrap();
        assert!(store.load_queue_snapshot(&s).unwrap().is_none());
    }

    #[test]
    fn delete_queue_snapshot_removes_the_row() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_server(&s, "subsonic", "Test", "http://x", None).unwrap();
        let snap = empty_snapshot(s.clone(), 2, Some(0));
        store.save_queue_snapshot(&s, &snap).unwrap();
        assert!(store.load_queue_snapshot(&s).unwrap().is_some());
        store.delete_queue_snapshot(&s).unwrap();
        assert!(store.load_queue_snapshot(&s).unwrap().is_none());
    }

    #[test]
    fn save_overwrites_previous_snapshot() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_server(&s, "subsonic", "Test", "http://x", None).unwrap();
        let snap1 = empty_snapshot(s.clone(), 2, Some(0));
        store.save_queue_snapshot(&s, &snap1).unwrap();
        let snap2 = empty_snapshot(s.clone(), 7, Some(3));
        store.save_queue_snapshot(&s, &snap2).unwrap();
        let loaded = store.load_queue_snapshot(&s).unwrap().unwrap();
        assert_eq!(loaded.entries.len(), 7);
        assert_eq!(loaded.current_index, Some(3));
    }

    #[test]
    fn load_treats_malformed_snapshot_as_missing() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_server(&s, "subsonic", "Test", "http://x", None).unwrap();
        // Inject a bad row by writing directly.
        let conn = store.connection().unwrap();
        conn.execute(
            "INSERT INTO queue_snapshots (server_id, snapshot, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![s.as_str(), "this is not json", 0],
        )
        .unwrap();
        drop(conn);
        let loaded = store.load_queue_snapshot(&s).unwrap();
        assert!(loaded.is_none(), "malformed snapshot must surface as None");
    }

    #[test]
    fn cascade_delete_removes_queue_snapshot_with_server() {
        let store = Store::open_memory().unwrap();
        let s = server();
        store.upsert_server(&s, "subsonic", "Test", "http://x", None).unwrap();
        let snap = empty_snapshot(s.clone(), 3, Some(1));
        store.save_queue_snapshot(&s, &snap).unwrap();
        assert!(store.load_queue_snapshot(&s).unwrap().is_some());
        store.delete_server(&s).unwrap();
        assert!(
            store.load_queue_snapshot(&s).unwrap().is_none(),
            "snapshot should cascade-delete with the server"
        );
    }
}
