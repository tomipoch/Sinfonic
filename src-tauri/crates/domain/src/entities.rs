//! Core music entities: `Album`, `Track`, `Artist`, `Genre`, `Playlist`.
//!
//! Field set mirrors Rufin's domain for parity, trimmed where v0.1 doesn't
//! need the value. Add fields as features land — keep the diff small per PR.
//!
//! `rename_all = "camelCase"` is applied to every type that crosses
//! the IPC boundary so the Rust snake_case fields line up with the
//! TypeScript camelCase types in `src/types/domain.ts`. Internal
//! Rust code continues to use snake_case field access.

use serde::{Deserialize, Serialize};

use super::ids::{AlbumId, ArtistId, GenreId, PlaylistId, TrackId};

/// Hint for what an image represents. The wire format is the
/// PascalCase variant name so the frontend can keep matching strings
/// ("Primary", "Backdrop", "CoverArt", "Embedded"). A new variant
/// only needs to be added here once and every mapping site picks it
/// up automatically.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ImageKindHint {
    /// The primary artwork (album cover, artist portrait).
    #[default]
    Primary,
    /// A backdrop / fanart image.
    Backdrop,
    /// A Subsonic-style "coverArt" id; not an indication of what the
    /// image *contains*, kept distinct for backward compat with the
    /// upstream API.
    CoverArt,
    /// Embedded artwork inside the audio file itself (local source).
    Embedded,
}

/// Reference to an image served by a provider, optionally with a tag for
/// cache-busting.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImageRef {
    pub item_id: String,
    pub kind: ImageKindHint,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<ArtistId>,
    pub year: Option<u16>,
    pub track_count: u16,
    pub duration_seconds: u32,
    pub favorite: bool,
    pub image_ref: Option<ImageRef>,
    pub genres: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: TrackId,
    pub album_id: AlbumId,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<ArtistId>,
    pub album: String,
    pub duration_seconds: u32,
    pub track_number: u16,
    pub disc_number: u16,
    pub favorite: bool,
    pub image_ref: Option<ImageRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub album_count: u32,
    pub track_count: u32,
    pub favorite: bool,
    pub image_ref: Option<ImageRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ArtistDetail {
    pub artist: Artist,
    pub albums: Vec<Album>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub id: GenreId,
    pub name: String,
    pub album_count: u32,
    pub track_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub track_count: u32,
    pub duration_seconds: u32,
    pub owner: Option<String>,
    pub public: bool,
    /// Optional cover art. Subsonic returns a `coverArt` string per
    /// playlist; Jellyfin tags it as `Primary`. The tag carries the
    /// provider's image id so the on-disk cache invalidates when the
    /// server bumps it.
    pub image_ref: Option<ImageRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenreDetail {
    pub genre: Genre,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MusicFolder {
    pub id: super::ids::MusicFolderId,
    pub name: String,
    pub track_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FolderEntry {
    pub id: super::ids::FolderId,
    pub name: String,
    pub path: String,
    pub kind: FolderEntryKind,
    pub track_count: u32,
    pub album_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum FolderEntryKind {
    Directory,
    Album,
    Track,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FolderDetail {
    pub path: String,
    pub entries: Vec<FolderEntry>,
}

// ─── Smart Playlists (Phase 9) ───────────────────────────────────

/// Single-rule smart playlist definition.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartPlaylist {
    pub id: super::ids::SmartPlaylistId,
    pub name: String,
    pub rule: SmartPlaylistRule,
    pub sort_field: SmartPlaylistSortField,
    pub sort_dir: SmartPlaylistSortDirection,
    pub limit_n: u16,
}

/// One filter rule for smart playlist evaluation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SmartPlaylistRule {
    pub field: SmartPlaylistRuleField,
    pub operator: SmartPlaylistRuleOperator,
    pub value: String,
}

/// Fields that can be used in a smart playlist rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartPlaylistRuleField {
    Title,
    Artist,
    Album,
    Genre,
    DurationSeconds,
    TrackNumber,
    Year,
    Favorite,
    PlayCount,
}

/// Operators for rule evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartPlaylistRuleOperator {
    Contains,
    StartsWith,
    EndsWith,
    Equals,
    LessThan,
    GreaterThan,
    NotContains,
    NotEquals,
}

/// Sort fields for smart playlist results.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartPlaylistSortField {
    Title,
    Artist,
    Album,
    DurationSeconds,
    Year,
    Random,
    DateAdded,
}

/// Sort direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartPlaylistSortDirection {
    Asc,
    Desc,
}

#[cfg(test)]
mod tests {
    //! Regression tests for the IPC wire format.
    //!
    //! Every entity type that crosses the Tauri command boundary is
    //! `#[serde(rename_all = "camelCase")]` so the TypeScript types in
    //! `src/types/domain.ts` line up field-for-field. These tests
    //! pin the JSON keys so a future refactor can't silently
    //! regress the boundary (the frontend used to receive `image_ref`
    //! for months because nobody asserted the camelCase).

    use super::*;

    #[test]
    fn image_ref_uses_camel_case_wire_format() {
        let image_ref = ImageRef {
            item_id: "abc".to_string(),
            kind: ImageKindHint::Primary,
            tag: Some("tag-1".to_string()),
        };
        let json = serde_json::to_string(&image_ref).unwrap();
        // Rust's snake_case fields must serialise as camelCase keys.
        assert!(json.contains("\"itemId\":\"abc\""), "got: {json}");
        assert!(json.contains("\"kind\":\"Primary\""), "got: {json}");
        assert!(json.contains("\"tag\":\"tag-1\""), "got: {json}");
        assert!(!json.contains("item_id"), "snake_case leaked: {json}");
    }

    #[test]
    fn track_uses_camel_case_wire_format() {
        let track = Track {
            id: TrackId::new("track-1"),
            album_id: AlbumId::new("album-1"),
            title: "T".to_string(),
            artist: "A".to_string(),
            artist_id: None,
            album: "Al".to_string(),
            duration_seconds: 1,
            track_number: 1,
            disc_number: 1,
            favorite: false,
            image_ref: None,
        };
        let json = serde_json::to_string(&track).unwrap();
        for key in ["albumId", "artistId", "durationSeconds", "trackNumber", "discNumber", "imageRef"] {
            assert!(json.contains(&format!("\"{key}\"")), "missing {key}: {json}");
        }
        for leaked in ["album_id", "artist_id", "duration_seconds", "track_number", "disc_number", "image_ref"] {
            assert!(!json.contains(leaked), "snake_case leaked `{leaked}`: {json}");
        }
    }

    #[test]
    fn album_uses_camel_case_wire_format() {
        let album = Album {
            id: AlbumId::new("album-1"),
            title: "Al".to_string(),
            artist: "A".to_string(),
            artist_id: None,
            year: None,
            track_count: 1,
            duration_seconds: 1,
            favorite: false,
            image_ref: None,
            genres: Vec::new(),
        };
        let json = serde_json::to_string(&album).unwrap();
        for key in ["trackCount", "durationSeconds", "imageRef"] {
            assert!(json.contains(&format!("\"{key}\"")), "missing {key}: {json}");
        }
        for leaked in ["track_count", "duration_seconds", "image_ref"] {
            assert!(!json.contains(leaked), "snake_case leaked `{leaked}`: {json}");
        }
    }

    #[test]
    fn playlist_uses_camel_case_wire_format() {
        let playlist = Playlist {
            id: PlaylistId::new("playlist-1"),
            name: "P".to_string(),
            track_count: 1,
            duration_seconds: 1,
            owner: None,
            public: false,
            image_ref: None,
        };
        let json = serde_json::to_string(&playlist).unwrap();
        for key in ["trackCount", "durationSeconds", "imageRef"] {
            assert!(json.contains(&format!("\"{key}\"")), "missing {key}: {json}");
        }
    }

    #[test]
    fn artist_uses_camel_case_wire_format() {
        let artist = Artist {
            id: ArtistId::new("artist-1"),
            name: "A".to_string(),
            album_count: 1,
            track_count: 1,
            favorite: false,
            image_ref: None,
        };
        let json = serde_json::to_string(&artist).unwrap();
        for key in ["albumCount", "trackCount", "imageRef"] {
            assert!(json.contains(&format!("\"{key}\"")), "missing {key}: {json}");
        }
        for leaked in ["album_count", "track_count", "image_ref"] {
            assert!(!json.contains(leaked), "snake_case leaked `{leaked}`: {json}");
        }
    }
}
