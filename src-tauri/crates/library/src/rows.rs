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
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, Genre, GenreId, ImageKindHint, ImageRef, Playlist,
    PlaylistId, Track, TrackId,
};

/// Map a wire-string image kind (e.g. "Primary") to the closed
/// `ImageKindHint` enum. Unknown or empty values fall back to
/// `Primary` because every row in the cache is keyed by it and a
/// missing kind would otherwise render a row undecodable.
fn parse_image_kind(s: Option<String>) -> ImageKindHint {
    match s.as_deref() {
        Some("Primary") | None => ImageKindHint::Primary,
        Some("Backdrop") => ImageKindHint::Backdrop,
        Some("CoverArt") => ImageKindHint::CoverArt,
        Some("Embedded") => ImageKindHint::Embedded,
        Some(_) => ImageKindHint::Primary,
    }
}

fn row_to_image_ref(row: &Row, kind_col: &str, tag_col: &str, id_col: &str) -> rusqlite::Result<Option<ImageRef>> {
    let kind_raw: Option<String> = row.get(kind_col)?;
    let kind = match kind_raw.as_deref() {
        Some(k) if !k.is_empty() => k,
        _ => return Ok(None),
    };
    let tag: Option<String> = row.get(tag_col)?;
    let raw_id: String = row.get(id_col)?;
    // Fresh fetches from each provider use a kind-prefixed
    // `item_id` (`"coverArt:tr-123"` for Subsonic, `"Primary:abc123"`
    // for Jellyfin, `"embedded:abc"` for the local-files provider).
    // The SQLite cache stores the prefixed alias in the id_col
    // (`"track-123"` / `"album-12"` / etc.), so re-add the kind prefix
    // on read so the on-disk cache key matches what fresh fetches
    // produce and the JS-side blob URL cache stays consistent across
    // prewarm + round-trip reads.
    let item_id = match kind {
        "CoverArt" | "Embedded" | "Primary" | "Backdrop" => {
            format!("{}:{}", kind_to_prefix(kind), raw_id)
        }
        _ => raw_id,
    };
    Ok(Some(ImageRef {
        item_id,
        kind: parse_image_kind(Some(kind.to_string())),
        tag,
    }))
}

/// Lowercase the wire `image_kind` value (`"CoverArt"` →
/// `"coverArt"`) so it matches the prefixes `mapping::image_ref_*`
/// helpers use in each provider. `"Primary"` / `"Backdrop"` /
/// `"Embedded"` pass through as PascalCase to match the Jellyfin
/// and local-files formats.
fn kind_to_prefix(kind: &str) -> String {
    match kind {
        "CoverArt" => "coverArt".to_string(),
        "Embedded" => "embedded".to_string(),
        // Primary / Backdrop are already lowercase-friendly on
        // the Jellyfin side; pass through unchanged so the prefix
        // matches what `image_ref_from_tags` produces.
        other => other.to_string(),
    }
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

/// Build a `Genre` from a row produced by `Library::list_genres`.
/// The `id` is derived from the genre name so a UI can build stable
/// links without needing the original provider's opaque id (we
/// don't currently store it in `album_genres`).
pub fn row_to_genre(row: &Row) -> rusqlite::Result<Genre> {
    let name: String = row.get("name")?;
    Ok(Genre {
        id: GenreId::new(name.clone()),
        name,
        album_count: row.get::<_, i64>("album_count")? as u32,
        track_count: row.get::<_, i64>("track_count")? as u32,
    })
}

/// Build a `Playlist` from a `playlists` row. The image columns
/// (`image_kind` / `image_tag`) are reconstructed into an
/// `ImageRef` whose `item_id` is the playlist's id — the
/// `image_metadata` / `image_bytes` provider methods strip the
/// prefix and resolve the cover via the provider's own endpoint.
pub fn row_to_playlist(row: &Row) -> rusqlite::Result<Playlist> {
    let image_ref = row_to_image_ref(row, "image_kind", "image_tag", "playlist_id")?;
    let id: String = row.get("playlist_id")?;
    Ok(Playlist {
        id: PlaylistId::new(id),
        name: row.get("name")?,
        track_count: row.get::<_, i64>("track_count")? as u32,
        duration_seconds: row.get::<_, i64>("duration_seconds")? as u32,
        owner: row.get("owner")?,
        public: row.get::<_, i64>("public")? != 0,
        image_ref,
    })
}
