//! Core music entities: `Album`, `Track`, `Artist`, `Genre`, `Playlist`.
//!
//! Field set mirrors Rufin's domain for parity, trimmed where v0.1 doesn't
//! need the value. Add fields as features land — keep the diff small per PR.

use serde::{Deserialize, Serialize};

use super::ids::{AlbumId, ArtistId, GenreId, PlaylistId, TrackId};

/// Reference to an image served by a provider, optionally with a tag for
/// cache-busting.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ImageRef {
    pub item_id: String,
    pub kind: String,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
pub struct Genre {
    pub id: GenreId,
    pub name: String,
    pub album_count: u32,
    pub track_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub track_count: u32,
    pub duration_seconds: u32,
    pub owner: Option<String>,
    pub public: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlaylistDetail {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
