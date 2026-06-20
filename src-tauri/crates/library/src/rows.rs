//! Conversions between SQLite rows and domain types.
//!
//! Every conversion is a pure function over a `&rusqlite::Row` so
//! the calling code in `store.rs` stays focused on the SQL, not the
//! field-by-field plumbing.
//!
//! Return type is `rusqlite::Result<T>` because the converters are
//! passed directly to `query_map`, which has that signature
//! constraint. Storage errors (corrupt row, wrong type) are
//! surfaced as `rusqlite::Error`.

use rusqlite::Row;
use sinfonic_domain::{Album, AlbumId, Artist, ArtistId, ImageRef, Track, TrackId};

fn row_to_image_ref(row: &Row, kind_col: &str, tag_col: &str, id_col: &str) -> rusqlite::Result<Option<ImageRef>> {
    let kind: Option<String> = row.get(kind_col)?;
    let kind = match kind {
        Some(k) if !k.is_empty() => k,
        _ => return Ok(None),
    };
    let tag: Option<String> = row.get(tag_col)?;
    let item_id: String = row.get(id_col)?;
    Ok(Some(ImageRef { item_id, kind, tag }))
}

pub fn row_to_album(row: &Row) -> rusqlite::Result<Album> {
    let image_ref = row_to_image_ref(row, "image_kind", "image_tag", "album_id")?;
    let id: String = row.get("album_id")?;
    let artist_id: Option<String> = row.get("artist_id")?;
    Ok(Album {
        id: AlbumId::new(id),
        title: row.get("title")?,
        artist: row.get("artist")?,
        artist_id: artist_id.map(ArtistId::new),
        year: row.get::<_, Option<u16>>("year")?,
        track_count: row.get::<_, u16>("track_count")?,
        duration_seconds: row.get("duration_seconds")?,
        favorite: row.get::<_, i64>("favorite")? != 0,
        image_ref,
        genres: Vec::new(),
    })
}

pub fn row_to_artist(row: &Row) -> rusqlite::Result<Artist> {
    let image_ref = row_to_image_ref(row, "image_kind", "image_tag", "artist_id")?;
    let id: String = row.get("artist_id")?;
    Ok(Artist {
        id: ArtistId::new(id),
        name: row.get("name")?,
        album_count: row.get("album_count")?,
        track_count: row.get("track_count")?,
        favorite: row.get::<_, i64>("favorite")? != 0,
        image_ref,
    })
}

pub fn row_to_track(row: &Row) -> rusqlite::Result<Track> {
    let image_ref = row_to_image_ref(row, "image_kind", "image_tag", "track_id")?;
    let id: String = row.get("track_id")?;
    let album_id: String = row.get("album_id")?;
    let artist_id: Option<String> = row.get("artist_id")?;
    Ok(Track {
        id: TrackId::new(id),
        album_id: AlbumId::new(album_id),
        title: row.get("title")?,
        artist: row.get("artist")?,
        artist_id: artist_id.map(ArtistId::new),
        album: row.get("album")?,
        duration_seconds: row.get("duration_seconds")?,
        track_number: row.get("track_number")?,
        disc_number: row.get("disc_number")?,
        favorite: row.get::<_, i64>("favorite")? != 0,
        image_ref,
    })
}
