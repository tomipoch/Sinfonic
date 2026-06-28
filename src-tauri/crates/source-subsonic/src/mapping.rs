//! Mapping Subsonic DTOs → domain entities.
//!
//! Every mapper is a pure function so it can be unit-tested without
//! an HTTP server. Image refs are normalised to `kind = "coverArt"`
//! (the Subsonic convention). Durations are in seconds on the wire
//! so no conversion is needed.
//!
//! Subsonic ids are strings with no inherent prefix; we wrap them
//! with `track-` / `album-` / `artist-` / `playlist-` so the SQLite
//! cache can scope them and the rest of the codebase can pattern
//! match on the prefix (the same convention as Jellyfin).

use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, ImageKindHint, ImageRef, Playlist, PlaylistId, Track,
    TrackId,
};

use super::dto::{AlbumDto, ArtistDto, ChildDto, PlaylistDto, PlaylistEntryDto};

/// Build an `Album` from a Subsonic `AlbumDto`. Returns `None` if
/// the id is empty — without an id we cannot scope the row in the
/// SQLite cache.
pub fn album_from_dto(dto: &AlbumDto) -> Option<Album> {
    if dto.id.is_empty() {
        return None;
    }
    let title = if !dto.title.as_deref().unwrap_or("").is_empty() {
        dto.title.clone().unwrap_or_default()
    } else {
        dto.name.clone()
    };
    let artist = dto.artist.clone().unwrap_or_default();
    let artist_id = dto
        .artist_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(ArtistId::from_external);
    let genres = collect_genres(dto.genre.as_deref(), &dto.genres);
    Some(Album {
        id: AlbumId::from_external(&dto.id),
        title,
        artist,
        artist_id,
        year: dto.year,
        track_count: dto.song_count,
        duration_seconds: dto.duration,
        favorite: dto.starred.is_some(),
        image_ref: image_ref_from_cover_art(&dto.id, dto.cover_art.as_deref()),
        genres,
    })
}

pub fn artist_from_dto(dto: &ArtistDto) -> Option<Artist> {
    if dto.id.is_empty() {
        return None;
    }
    // Always build an `image_ref` for the artist. Subsonic's
    // `getArtists` / `getIndexes` responses don't include the
    // `coverArt` field (only `getArtist` does), so we fall back to
    // the artist id itself: Navidrome accepts `getCoverArt?id=ar-…`
    // and returns the artist image. The `tag` carries the original
    // `coverArt` value when present so a server-side image swap
    // invalidates the on-disk cache.
    let tag = dto
        .cover_art
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some(Artist {
        id: ArtistId::from_external(&dto.id),
        name: dto.name.clone(),
        album_count: dto.album_count,
        track_count: 0,
        favorite: dto.starred.is_some(),
        image_ref: Some(sinfonic_domain::ImageRef {
            item_id: format!("coverArt:{}", dto.id),
            kind: sinfonic_domain::ImageKindHint::CoverArt,
            tag: tag.or_else(|| Some(dto.id.clone())),
        }),
    })
}

pub fn track_from_child(dto: &ChildDto) -> Option<Track> {
    if dto.id.is_empty() || dto.album_id.is_none() {
        return None;
    }
    let artist_name = dto.artist.clone().unwrap_or_default();
    let artist_id = dto
        .artist_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(ArtistId::from_external);
    let _genres = collect_genres(dto.genre.as_deref(), &dto.genres);
    Some(Track {
        id: TrackId::from_external(&dto.id),
        album_id: AlbumId::from_external(dto.album_id.as_deref().unwrap_or("")),
        title: dto.title.clone(),
        artist: artist_name,
        artist_id,
        album: dto.album.clone().unwrap_or_default(),
        duration_seconds: dto.duration.unwrap_or(0),
        track_number: dto.track.unwrap_or(0),
        disc_number: dto.disc_number.unwrap_or(1),
        favorite: dto.starred.is_some(),
        image_ref: image_ref_from_cover_art(&dto.id, dto.cover_art.as_deref()),
    })
}

pub fn playlist_from_dto(dto: &PlaylistDto) -> Option<Playlist> {
    if dto.id.is_empty() {
        return None;
    }
    let image_ref = playlist_image_ref(dto.id.as_str(), dto.cover_art.as_deref());
    Some(Playlist {
        id: PlaylistId::from_external(&dto.id),
        name: dto.name.clone(),
        track_count: dto.song_count,
        duration_seconds: dto.duration,
        owner: dto.owner.clone(),
        public: dto.public,
        image_ref,
    })
}

/// Build the `ImageRef` for a playlist. `cover_art` is the Subsonic
/// cover id when present; we fall back to the playlist id itself so
/// `getCoverArt?id=playlist-…` resolves to whatever the server has
/// (Navidrome accepts playlist ids for `getCoverArt`).
fn playlist_image_ref(playlist_id: &str, cover_art: Option<&str>) -> Option<ImageRef> {
    let tag = cover_art
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| Some(playlist_id.to_string()));
    Some(ImageRef {
        item_id: format!("coverArt:{playlist_id}"),
        kind: ImageKindHint::CoverArt,
        tag,
    })
}

/// Same as `track_from_child` but reads from a playlist entry which
/// carries the same fields (`title`, `album`, `artist`, etc.) plus
/// an entry id. We use the entry id only as a hint; the domain
/// `Track` only carries the underlying track id.
pub fn track_from_playlist_entry(dto: &PlaylistEntryDto) -> Option<Track> {
    if dto.id.is_empty() {
        return None;
    }
    let child = ChildDto {
        id: dto.id.clone(),
        parent: dto.parent.clone(),
        title: dto.title.clone(),
        album: dto.album.clone(),
        artist: dto.artist.clone(),
        track: dto.track,
        disc_number: dto.disc_number,
        year: dto.year,
        duration: dto.duration,
        album_id: dto.album_id.clone(),
        artist_id: dto.artist_id.clone(),
        cover_art: dto.cover_art.clone(),
        starred: None,
        genre: dto.genre.clone(),
        genres: dto.genres.clone(),
        ..Default::default()
    };
    track_from_child(&child)
}

/// Build the streaming URL for a track. The token is embedded as
/// `p=<token>` so audio clients (browsers, mpv) can be hand-launched
/// without needing to know the salt. The salt is sent alongside
/// for servers that require it.
pub fn track_stream_url(
    server_url: &str,
    track_id: &str,
    username: &str,
    salt: &str,
    token: &str,
) -> String {
    let stripped = track_id.strip_prefix("track-").unwrap_or(track_id);
    let server_url = server_url.trim_end_matches('/');
    format!(
        "{}/rest/stream?id={}&maxBitRate=0&format=raw&u={}&t={}&s={}&v=1.16.1&c=sinfonic&f=json",
        server_url, stripped, username, token, salt
    )
}

fn image_ref_from_cover_art(id: &str, cover_art: Option<&str>) -> Option<ImageRef> {
    let value = cover_art?;
    if value.is_empty() {
        return None;
    }
    // If the cover art is an `id` use that; otherwise it's a
    // server-relative path or a full URL.
    Some(ImageRef {
        item_id: format!("coverArt:{id}"),
        kind: ImageKindHint::CoverArt,
        tag: Some(value.to_string()),
    })
}

fn collect_genres(
    primary: Option<&str>,
    refs: &[super::dto::GenreRefDto],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(name) = primary {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    for r in refs {
        if !r.name.is_empty() && !out.iter().any(|g| g == &r.name) {
            out.push(r.name.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::GenreRefDto;

    fn album_dto() -> AlbumDto {
        AlbumDto {
            id: "al-1".into(),
            name: "OK Computer".into(),
            title: None,
            artist: Some("Radiohead".into()),
            artist_id: Some("ar-1".into()),
            year: Some(1997),
            song_count: 12,
            duration: 3540,
            cover_art: Some("al-1".into()),
            genre: Some("Rock".into()),
            genres: vec![GenreRefDto {
                name: "Alternative".into(),
            }],
            starred: Some("2024-01-01T00:00:00Z".into()),
            created: Some("2023-01-01T00:00:00Z".into()),
            song: vec![],
        }
    }

    #[test]
    fn album_dto_maps_to_album() {
        let album = album_from_dto(&album_dto()).expect("non-empty id");
        assert_eq!(album.id.as_str(), "album-al-1");
        assert_eq!(album.title, "OK Computer");
        assert_eq!(album.artist, "Radiohead");
        assert_eq!(album.artist_id.unwrap().as_str(), "artist-ar-1");
        assert_eq!(album.year, Some(1997));
        assert_eq!(album.track_count, 12);
        assert_eq!(album.duration_seconds, 3540);
        assert!(album.favorite);
        assert_eq!(album.image_ref.as_ref().unwrap().kind, ImageKindHint::CoverArt);
        assert_eq!(
            album.image_ref.as_ref().unwrap().tag.as_deref(),
            Some("al-1")
        );
        assert!(album.genres.contains(&"Rock".to_string()));
        assert!(album.genres.contains(&"Alternative".to_string()));
    }

    #[test]
    fn album_dto_without_id_is_dropped() {
        let mut dto = album_dto();
        dto.id = String::new();
        assert!(album_from_dto(&dto).is_none());
    }

    #[test]
    fn track_dto_maps_to_track() {
        let dto = ChildDto {
            id: "t-1".into(),
            parent: Some("al-1".into()),
            is_dir: false,
            title: "Airbag".into(),
            album: Some("OK Computer".into()),
            artist: Some("Radiohead".into()),
            track: Some(1),
            disc_number: Some(1),
            year: Some(1997),
            duration: Some(270),
            album_id: Some("al-1".into()),
            artist_id: Some("ar-1".into()),
            cover_art: Some("al-1".into()),
            starred: Some("2024-01-01T00:00:00Z".into()),
            genre: Some("Rock".into()),
            genres: vec![],
            ..Default::default()
        };
        let track = track_from_child(&dto).expect("has id + album_id");
        assert_eq!(track.id.as_str(), "track-t-1");
        assert_eq!(track.album_id.as_str(), "album-al-1");
        assert_eq!(track.title, "Airbag");
        assert_eq!(track.artist, "Radiohead");
        assert_eq!(track.track_number, 1);
        assert_eq!(track.disc_number, 1);
        assert_eq!(track.duration_seconds, 270);
        assert!(track.favorite);
    }

    #[test]
    fn track_without_album_id_is_dropped() {
        let dto = ChildDto {
            id: "t-1".into(),
            title: "Airbag".into(),
            ..Default::default()
        };
        assert!(track_from_child(&dto).is_none());
    }

    #[test]
    fn artist_dto_maps_to_artist() {
        let dto = ArtistDto {
            id: "ar-1".into(),
            name: "Radiohead".into(),
            album_count: 9,
            cover_art: Some("ar-1".into()),
            artist_image_url: None,
            starred: None,
        };
        let artist = artist_from_dto(&dto).expect("non-empty id");
        assert_eq!(artist.id.as_str(), "artist-ar-1");
        assert_eq!(artist.name, "Radiohead");
        assert_eq!(artist.album_count, 9);
        assert!(!artist.favorite);
        let image_ref = artist.image_ref.as_ref().expect("image_ref is set");
        assert_eq!(image_ref.item_id, "coverArt:ar-1");
        assert_eq!(image_ref.kind, ImageKindHint::CoverArt);
        assert_eq!(image_ref.tag.as_deref(), Some("ar-1"));
    }

    #[test]
    fn artist_dto_without_cover_art_still_gets_image_ref() {
        // `getArtists` / `getIndexes` don't include the `coverArt`
        // field — the artist image_ref must still be populated so
        // the frontend can request `getCoverArt?id=ar-…`.
        let dto = ArtistDto {
            id: "ar-2".into(),
            name: "Björk".into(),
            album_count: 10,
            cover_art: None,
            artist_image_url: None,
            starred: None,
        };
        let artist = artist_from_dto(&dto).expect("non-empty id");
        let image_ref = artist.image_ref.as_ref().expect("image_ref is set");
        assert_eq!(image_ref.item_id, "coverArt:ar-2");
        assert_eq!(image_ref.tag.as_deref(), Some("ar-2"));
    }

    #[test]
    fn track_stream_url_strips_prefix_and_signs_request() {
        let url = track_stream_url("http://localhost:4533/", "track-abc123", "alice", "salt", "tok");
        assert!(url.contains("/rest/stream?id=abc123"));
        assert!(url.contains("u=alice"));
        assert!(url.contains("t=tok"));
        assert!(url.contains("s=salt"));
        assert!(!url.contains("track-track-"));
    }

    #[test]
    fn collect_genres_dedupes_and_trims() {
        let merged = collect_genres(
            Some("  Rock  "),
            &[
                GenreRefDto { name: "Rock".into() },
                GenreRefDto { name: "Alt".into() },
            ],
        );
        assert_eq!(merged, vec!["Rock".to_string(), "Alt".to_string()]);
    }
}
