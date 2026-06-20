//! Full-text search over the `library_fts` virtual table.
//!
//! The FTS5 index is contentless (`content=' '`) and stores one row
//! per cached entity, with `kind` distinguishing the three kinds
//! (album / track / artist) we currently search. Queries are
//! dispatched once per kind and ranked by FTS5's `bm25` (lower
//! score = better match). Each kind returns up to `limit` rows; the
//! caller controls the global cap.
//!
//! Query sanitization: we strip FTS5 syntax characters and quote
//! the cleaned input as a single phrase, then append a prefix
//! wildcard (`*`) so the user gets prefix matching for free.

use rusqlite::{params_from_iter, Connection};
use sinfonic_domain::{Album, Artist, SearchResults, ServerId, Track};

use crate::error::LibraryResult;
use crate::rows;

/// Run an FTS5 search scoped to one server. Returns at most
/// `per_kind_limit` matches of each kind, ranked by FTS5's bm25.
pub fn search(
    conn: &Connection,
    server_id: &ServerId,
    query: &str,
    per_kind_limit: usize,
) -> LibraryResult<SearchResults> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(SearchResults::default());
    }

    // Sanitize: strip FTS5 syntax characters, then quote as a single
    // phrase with a trailing wildcard for prefix matching. Pure
    // punctuation yields an empty result.
    let cleaned: String = trimmed
        .chars()
        .filter(|c| {
            !matches!(
                *c,
                '"' | '*' | '(' | ')' | ':' | '^' | '+' | '-' | '[' | ']' | '{' | '}' | '\\'
            )
        })
        .collect();
    if cleaned.trim().is_empty() {
        return Ok(SearchResults::default());
    }
    let fts_query = format!("\"{}\"*", cleaned);

    // FTS5 ranks lower-is-better. We grab `3 * per_kind_limit` rows
    // (heuristic: at most ~1/3 of hits are of any one kind) and
    // then trim each kind to `per_kind_limit` after sorting.
    let total_limit = per_kind_limit.saturating_mul(3).max(per_kind_limit) as i64;

    let mut stmt = conn.prepare(
        "SELECT kind, entity_id, bm25(library_fts) AS score
         FROM library_fts
         WHERE library_fts MATCH ?1 AND server_id = ?2
         ORDER BY score
         LIMIT ?3",
    )?;

    let mut rows_iter = stmt.query(params_from_iter([
        fts_query,
        server_id.as_str().to_string(),
        total_limit.to_string(),
    ]))?;

    let mut by_kind: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    while let Some(row) = rows_iter.next()? {
        let kind: String = row.get(0)?;
        let entity_id: String = row.get(1)?;
        let key = match kind.as_str() {
            "album" => "album",
            "track" => "track",
            "artist" => "artist",
            _ => continue,
        };
        by_kind.entry(key).or_default().push(entity_id);
    }

    let results = SearchResults {
        albums: fetch_albums(conn, server_id, &by_kind.remove("album").unwrap_or_default(), per_kind_limit)?,
        artists: fetch_artists(conn, server_id, &by_kind.remove("artist").unwrap_or_default(), per_kind_limit)?,
        tracks: fetch_tracks(conn, server_id, &by_kind.remove("track").unwrap_or_default(), per_kind_limit)?,
        playlists: Vec::new(),
    };
    Ok(results)
}

fn fetch_albums(
    conn: &Connection,
    server_id: &ServerId,
    entity_ids: &[String],
    limit: usize,
) -> LibraryResult<Vec<Album>> {
    fetch_entities(
        conn,
        server_id,
        entity_ids,
        limit,
        "albums",
        "album_id",
        rows::row_to_album,
    )
}

fn fetch_artists(
    conn: &Connection,
    server_id: &ServerId,
    entity_ids: &[String],
    limit: usize,
) -> LibraryResult<Vec<Artist>> {
    fetch_entities(
        conn,
        server_id,
        entity_ids,
        limit,
        "artists",
        "artist_id",
        rows::row_to_artist,
    )
}

fn fetch_tracks(
    conn: &Connection,
    server_id: &ServerId,
    entity_ids: &[String],
    limit: usize,
) -> LibraryResult<Vec<Track>> {
    fetch_entities(
        conn,
        server_id,
        entity_ids,
        limit,
        "tracks",
        "track_id",
        rows::row_to_track,
    )
}

fn fetch_entities<T, F>(
    conn: &Connection,
    server_id: &ServerId,
    entity_ids: &[String],
    limit: usize,
    table: &str,
    id_column: &str,
    row_to_entity: F,
) -> LibraryResult<Vec<T>>
where
    F: Fn(&rusqlite::Row) -> rusqlite::Result<T>,
{
    if entity_ids.is_empty() {
        return Ok(Vec::new());
    }
    let take: Vec<&String> = entity_ids.iter().take(limit).collect();
    if take.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders: Vec<String> = (0..take.len()).map(|i| format!("?{}", i + 2)).collect();
    let sql = format!(
        "SELECT * FROM {table} WHERE server_id = ?1 AND {id_column} IN ({ph})",
        table = table,
        id_column = id_column,
        ph = placeholders.join(",")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(1 + take.len());
    params.push(Box::new(server_id.as_str().to_string()));
    for id in &take {
        params.push(Box::new((*id).clone()));
    }
    let items = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())), row_to_entity)?
        .collect::<rusqlite::Result<Vec<T>>>()?;
    Ok(items)
}

#[cfg(test)]
mod tests {
    use crate::Store;
    use sinfonic_domain::{Album, AlbumId, Artist, ArtistId, ServerId, Track, TrackId};

    fn s() -> ServerId {
        ServerId::new("server-1")
    }

    fn album(id: &str, title: &str) -> Album {
        Album {
            id: AlbumId::new(id),
            title: title.into(),
            artist: "Radiohead".into(),
            artist_id: None,
            year: Some(1997),
            track_count: 12,
            duration_seconds: 3540,
            favorite: false,
            image_ref: None,
            genres: vec!["Rock".into()],
        }
    }

    fn track(id: &str, title: &str) -> Track {
        Track {
            id: TrackId::new(id),
            album_id: AlbumId::new("album-1"),
            title: title.into(),
            artist: "Radiohead".into(),
            artist_id: None,
            album: "OK Computer".into(),
            duration_seconds: 240,
            track_number: 1,
            disc_number: 1,
            favorite: false,
            image_ref: None,
        }
    }

    fn artist(id: &str, name: &str) -> Artist {
        Artist {
            id: ArtistId::new(id),
            name: name.into(),
            album_count: 5,
            track_count: 60,
            favorite: false,
            image_ref: None,
        }
    }

    #[test]
    fn search_returns_matching_tracks() {
        let store = Store::open_memory().unwrap();
        let server = s();
        store
            .replace_albums(&server, &[album("album-1", "OK Computer")])
            .unwrap();
        store
            .replace_tracks(
                &server,
                &[
                    track("track-1", "Karma Police"),
                    track("track-2", "Lucky"),
                    track("track-3", "Paranoid Android"),
                ],
            )
            .unwrap();

        let results = store.search(&server, "karma", 10).unwrap();
        assert_eq!(results.tracks.len(), 1);
        assert_eq!(results.tracks[0].title, "Karma Police");
        assert!(results.albums.is_empty());
        assert!(results.artists.is_empty());
    }

    #[test]
    fn search_supports_prefix_matching() {
        let store = Store::open_memory().unwrap();
        let server = s();
        store
            .replace_albums(&server, &[album("album-1", "OK Computer")])
            .unwrap();
        store
            .replace_tracks(
                &server,
                &[
                    track("track-1", "Karma Police"),
                    track("track-2", "Karate"),
                    track("track-3", "Lucky"),
                ],
            )
            .unwrap();
        let results = store.search(&server, "kar", 10).unwrap();
        assert_eq!(results.tracks.len(), 2);
    }

    #[test]
    fn search_sanitises_fts5_operators() {
        let store = Store::open_memory().unwrap();
        let server = s();
        store
            .replace_albums(&server, &[album("album-1", "OK Computer")])
            .unwrap();
        store.replace_tracks(&server, &[track("t1", "Karma")]).unwrap();
        // Wildcard in input — should not raise an FTS5 syntax error.
        let r1 = store.search(&server, "*", 10).unwrap();
        assert!(r1.tracks.is_empty());
        // Plus stripped.
        let r2 = store.search(&server, "+karma", 10).unwrap();
        assert_eq!(r2.tracks.len(), 1);
        // Pure punctuation yields nothing.
        let r3 = store.search(&server, "()", 10).unwrap();
        assert!(r3.is_empty());
        // Empty / whitespace returns nothing.
        let r4 = store.search(&server, "   ", 10).unwrap();
        assert!(r4.is_empty());
    }

    #[test]
    fn search_respects_per_kind_limit() {
        let store = Store::open_memory().unwrap();
        let server = s();
        let albums: Vec<Album> = (0..10)
            .map(|i| Album {
                id: AlbumId::new(format!("a-{i}")),
                title: format!("Abbey Road {i}"),
                ..album("placeholder", "x")
            })
            .collect();
        store.replace_albums(&server, &albums).unwrap();
        let results = store.search(&server, "Abbey", 3).unwrap();
        assert_eq!(results.albums.len(), 3);
    }

    #[test]
    fn search_finds_artists() {
        let store = Store::open_memory().unwrap();
        let server = s();
        store
            .replace_artists(&server, &[artist("ar-1", "Radiohead")])
            .unwrap();
        let results = store.search(&server, "Radio", 10).unwrap();
        assert_eq!(results.artists.len(), 1);
        assert_eq!(results.artists[0].name, "Radiohead");
    }

    #[test]
    fn search_finds_albums() {
        let store = Store::open_memory().unwrap();
        let server = s();
        store
            .replace_albums(&server, &[album("album-1", "OK Computer")])
            .unwrap();
        let results = store.search(&server, "Computer", 10).unwrap();
        assert_eq!(results.albums.len(), 1);
        assert_eq!(results.albums[0].title, "OK Computer");
    }

    #[test]
    fn search_is_server_scoped() {
        let store = Store::open_memory().unwrap();
        let s1 = ServerId::new("server-1");
        let s2 = ServerId::new("server-2");
        store
            .replace_albums(&s1, &[album("album-1", "OK Computer")])
            .unwrap();
        store
            .replace_albums(&s2, &[album("album-1", "Best Of")])
            .unwrap();
        store
            .replace_tracks(&s1, &[track("t-1", "Karma Police")])
            .unwrap();
        store
            .replace_tracks(&s2, &[track("t-1", "Karma Chameleon")])
            .unwrap();

        let r1 = store.search(&s1, "Karma", 10).unwrap();
        assert_eq!(r1.tracks.len(), 1);
        assert_eq!(r1.tracks[0].title, "Karma Police");

        let r2 = store.search(&s2, "Karma", 10).unwrap();
        assert_eq!(r2.tracks.len(), 1);
        assert_eq!(r2.tracks[0].title, "Karma Chameleon");
    }
}
