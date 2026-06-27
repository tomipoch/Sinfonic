//! Mapping Jellyfin DTOs → domain entities.
//!
//! Every mapper is a pure function over a `BaseItemDto` /
//! `PlaylistDto` so it can be unit-tested without spinning up a
//! server. Jellyfin expresses durations in 100-ns ticks; we divide
//! by `TICKS_PER_SECOND` once at the boundary and the rest of the
//! codebase only sees whole seconds.
//!
//! Image references are normalised to `kind = "Primary"` /
//! `"Backdrop"`. Anything Jellyfin doesn't tag is dropped.

use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, ImageKindHint, ImageRef, Playlist, PlaylistId, Track,
    TrackId,
};

use super::dto::{BaseItemDto, PlaylistDto};

/// Jellyfin expresses time as the number of 100-ns ticks since the
/// Windows epoch. Anything else in the app talks in seconds.
const TICKS_PER_SECOND: u64 = 10_000_000;

/// Build an `Album` from a Jellyfin `BaseItemDto` typed as
/// `MusicAlbum`. Returns `None` if `Id` is empty — without an id we
/// cannot scope the row in the SQLite cache.
pub fn album_from_dto(dto: &BaseItemDto) -> Option<Album> {
    if dto.id.is_empty() {
        return None;
    }
    let artist_name = dto
        .album_artist
        .clone()
        .or_else(|| {
            dto.album_artists
                .first()
                .map(|p| p.name.clone())
        })
        .unwrap_or_default();
    let artist_id = dto
        .album_artists
        .first()
        .map(|p| ArtistId::from_external(&p.id));

    let year = dto
        .production_year
        .or_else(|| dto.premiere_date.as_deref().and_then(extract_year));

    Some(Album {
        id: AlbumId::from_external(&dto.id),
        title: dto.name.clone().unwrap_or_default(),
        artist: artist_name,
        artist_id,
        year,
        track_count: dto.child_count.unwrap_or(0) as u16,
        duration_seconds: ticks_to_seconds(dto.cumulative_run_time_ticks),
        favorite: dto
            .user_data
            .as_ref()
            .map(|u| u.is_favorite)
            .unwrap_or(false),
        image_ref: image_ref_from_tags(dto, ImageKindHint::Primary),
        genres: dto.genres.clone(),
    })
}

pub fn artist_from_dto(dto: &BaseItemDto) -> Option<Artist> {
    if dto.id.is_empty() {
        return None;
    }
    Some(Artist {
        id: ArtistId::from_external(&dto.id),
        name: dto.name.clone().unwrap_or_default(),
        album_count: dto.child_count.unwrap_or(0),
        track_count: 0,
        favorite: dto
            .user_data
            .as_ref()
            .map(|u| u.is_favorite)
            .unwrap_or(false),
        image_ref: image_ref_from_tags(dto, ImageKindHint::Primary),
    })
}

pub fn track_from_dto(dto: &BaseItemDto) -> Option<Track> {
    if dto.id.is_empty() || dto.album_id.is_none() {
        return None;
    }
    let artist_name = dto
        .artists
        .first()
        .cloned()
        .or_else(|| dto.artist_items.first().map(|p| p.name.clone()))
        .unwrap_or_default();
    let artist_id = dto
        .artist_items
        .first()
        .map(|p| ArtistId::from_external(&p.id));
    Some(Track {
        id: TrackId::from_external(&dto.id),
        album_id: AlbumId::from_external(dto.album_id.as_deref().unwrap_or("")),
        title: dto.name.clone().unwrap_or_default(),
        artist: artist_name,
        artist_id,
        album: dto.album.clone().unwrap_or_default(),
        duration_seconds: ticks_to_seconds(dto.run_time_ticks),
        track_number: dto.index_number.unwrap_or(0),
        disc_number: dto.parent_index_number.unwrap_or(1),
        favorite: dto
            .user_data
            .as_ref()
            .map(|u| u.is_favorite)
            .unwrap_or(false),
        image_ref: image_ref_from_tags(dto, ImageKindHint::Primary),
    })
}

pub fn playlist_from_dto(dto: &PlaylistDto) -> Option<Playlist> {
    if dto.id.is_empty() {
        return None;
    }
    Some(Playlist {
        id: PlaylistId::from_external(&dto.id),
        name: dto.name.clone().unwrap_or_default(),
        track_count: dto.child_count.unwrap_or(0),
        duration_seconds: ticks_to_seconds(dto.cumulative_run_time_ticks),
        owner: dto.owner_user_id.clone(),
        public: dto.open_access.unwrap_or(false),
        image_ref: dto
            .image_tags
            .as_ref()
            .and_then(|t| t.primary.as_ref().filter(|s| !s.is_empty()))
            .map(|tag| ImageRef {
                item_id: dto.id.clone(),
                kind: ImageKindHint::Primary,
                tag: Some(tag.clone()),
            }),
    })
}

/// Build the streaming URL for a track. The token is appended as
/// `api_key=` so audio clients (browsers, mpv) can be hand-launched
/// without sending the `X-Emby-Authorization` header.
pub fn track_stream_url(server_url: &str, track_id: &str, token: &str) -> String {
    let stripped = track_id
        .strip_prefix("track-")
        .unwrap_or(track_id);
    let server_url = server_url.trim_end_matches('/');
    format!(
        "{}/Audio/{}/universal?userId=&api_key={}&deviceId=sinfonic&container=mp3,aac,m4a,flac,webm,ogg,wav",
        server_url, stripped, token
    )
}

fn image_ref_from_tags(dto: &BaseItemDto, kind: ImageKindHint) -> Option<ImageRef> {
    let tag = dto.image_tags.as_ref().and_then(|t| match kind {
        ImageKindHint::Primary => t.primary.clone(),
        ImageKindHint::Backdrop => t.backdrop.clone(),
        _ => None,
    });
    if dto.id.is_empty() {
        return None;
    }
    let kind_label = match kind {
        ImageKindHint::Primary => "Primary",
        ImageKindHint::Backdrop => "Backdrop",
        _ => "Primary",
    };
    tag.map(|t| ImageRef {
        item_id: format!("{kind_label}:{}", dto.id),
        kind,
        tag: Some(t),
    })
}

fn ticks_to_seconds(ticks: Option<u64>) -> u32 {
    ticks
        .map(|t| (t / TICKS_PER_SECOND) as u32)
        .unwrap_or(0)
}

fn extract_year(date: &str) -> Option<u16> {
    date.get(..4).and_then(|y| y.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{ImageTags, NameIdPair, UserData};

    fn album_dto() -> BaseItemDto {
        BaseItemDto {
            id: "abc".into(),
            name: Some("OK Computer".into()),
            r#type: Some("MusicAlbum".into()),
            server_id: None,
            album_artist: Some("Radiohead".into()),
            album_artists: vec![NameIdPair {
                id: "a1".into(),
                name: "Radiohead".into(),
            }],
            production_year: Some(1997),
            premiere_date: None,
            child_count: Some(12),
            cumulative_run_time_ticks: Some(3540 * TICKS_PER_SECOND),
            genres: vec!["Rock".into()],
            is_folder: Some(true),
            album_id: None,
            album: None,
            artists: vec![],
            artist_items: vec![],
            index_number: None,
            parent_index_number: None,
            run_time_ticks: None,
            image_tags: Some(ImageTags {
                primary: Some("tag-1".into()),
                backdrop: None,
            }),
            primary_image_aspect_ratio: Some(1.0),
            user_data: Some(UserData {
                is_favorite: true,
                play_count: 3,
                last_played_date: None,
            }),
            can_download: Some(true),
            play_access: Some("Full".into()),
        }
    }

    #[test]
    fn album_dto_maps_to_album() {
        let album = album_from_dto(&album_dto()).expect("non-empty id");
        assert_eq!(album.id.as_str(), "album-abc");
        assert_eq!(album.title, "OK Computer");
        assert_eq!(album.artist, "Radiohead");
        assert_eq!(album.artist_id.unwrap().as_str(), "artist-a1");
        assert_eq!(album.year, Some(1997));
        assert_eq!(album.track_count, 12);
        assert_eq!(album.duration_seconds, 3540);
        assert!(album.favorite);
        assert_eq!(album.image_ref.as_ref().unwrap().kind, "Primary");
        assert_eq!(album.image_ref.as_ref().unwrap().tag.as_deref(), Some("tag-1"));
        assert_eq!(album.genres, vec!["Rock".to_string()]);
    }

    #[test]
    fn album_dto_without_id_is_dropped() {
        let mut dto = album_dto();
        dto.id = String::new();
        assert!(album_from_dto(&dto).is_none());
    }

    #[test]
    fn track_dto_maps_to_track() {
        let mut dto = album_dto();
        dto.id = "t-1".into();
        dto.album_id = Some("a-1".into());
        dto.album = Some("OK Computer".into());
        dto.artists = vec!["Radiohead".into()];
        dto.artist_items = vec![NameIdPair {
            id: "a1".into(),
            name: "Radiohead".into(),
        }];
        dto.index_number = Some(3);
        dto.parent_index_number = Some(1);
        dto.run_time_ticks = Some(240 * TICKS_PER_SECOND);
        dto.cumulative_run_time_ticks = None;

        let track = track_from_dto(&dto).expect("has id + album_id");
        assert_eq!(track.id.as_str(), "track-t-1");
        assert_eq!(track.album_id.as_str(), "album-a-1");
        assert_eq!(track.album, "OK Computer");
        assert_eq!(track.artist, "Radiohead");
        assert_eq!(track.track_number, 3);
        assert_eq!(track.disc_number, 1);
        assert_eq!(track.duration_seconds, 240);
    }

    #[test]
    fn track_without_album_id_is_dropped() {
        let mut dto = album_dto();
        dto.id = "t-1".into();
        dto.album_id = None;
        assert!(track_from_dto(&dto).is_none());
    }

    #[test]
    fn artist_dto_maps_to_artist() {
        let mut dto = album_dto();
        dto.id = "ar-1".into();
        dto.album_artist = None;
        dto.album_artists.clear();
        dto.album = None;
        let artist = artist_from_dto(&dto).expect("non-empty id");
        assert_eq!(artist.id.as_str(), "artist-ar-1");
        assert_eq!(artist.name, "OK Computer");
        assert_eq!(artist.album_count, 12);
    }

    #[test]
    fn track_stream_url_strips_prefix_and_appends_token() {
        let url = track_stream_url("http://localhost:8096/", "track-abc123", "tok");
        assert!(url.contains("/Audio/abc123/universal"));
        assert!(url.contains("api_key=tok"));
        // No leaked prefix.
        assert!(!url.contains("track-track-"));
    }

    #[test]
    fn ticks_to_seconds_rounds_down() {
        assert_eq!(ticks_to_seconds(Some(9_999_999)), 0);
        assert_eq!(ticks_to_seconds(Some(10_000_000)), 1);
        assert_eq!(ticks_to_seconds(None), 0);
    }
}