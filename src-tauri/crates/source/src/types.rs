//! Shared provider DTOs that are part of the trait surface.

use serde::{Deserialize, Serialize};
use sinfonic_domain::{
    Album, AlbumDetail, Artist, ArtistDetail, Genre, Playlist, PlaylistId, PagedResponse,
    SearchResults as DomainSearchResults, Track, TrackId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum HomeSectionKind {
    Explore,
    #[default]
    MostPlayed,
    NewlyAdded,
    RecentlyPlayed,
    RecentlyReleased,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct HomeSection {
    pub kind: HomeSectionKind,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AlbumDetailResponse {
    pub detail: AlbumDetail,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ArtistDetailResponse {
    pub detail: ArtistDetail,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RandomTrackRequest {
    pub limit: usize,
    pub genre: Option<String>,
    pub from_year: Option<u16>,
    pub to_year: Option<u16>,
    pub only_unplayed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlaybackReport {
    pub kind: PlaybackReportKind,
    pub track_id: TrackId,
    pub position_seconds: u32,
    pub paused: bool,
    pub muted: bool,
    pub volume_percent: u8,
    pub shuffle: bool,
    pub repeat_one: bool,
    pub repeat_all: bool,
    pub failed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum PlaybackReportKind {
    Started,
    Progress,
    Stopped,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StreamRequest {
    pub track_id: TrackId,
    pub max_bitrate_kbps: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageRequest {
    pub item_id: String,
    pub kind: sinfonic_domain::ImageKind,
    pub tag: Option<String>,
    pub size: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageBytes {
    pub bytes: Vec<u8>,
    /// MIME type from the server's `Content-Type` header when
    /// available (e.g. `image/jpeg`, `image/png`). Providers should
    /// populate this so the frontend can render the right format;
    /// `None` means "unknown — guess from the bytes".
    pub content_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageMetadata {
    pub item_id: String,
    pub kind: sinfonic_domain::ImageKind,
    pub tag: Option<String>,
    pub url: String,
}

/// What a provider considers a "favoritable" item.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum FavoriteItemId {
    Track(TrackId),
    Album(sinfonic_domain::AlbumId),
    Artist(sinfonic_domain::ArtistId),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Lyrics {
    pub plain: Option<String>,
    pub synced: Option<String>,
    pub source: Option<String>,
}

/// Provider-specific search response. Re-exported under the crate's root
/// so consumers don't have to know which layer defined the type.
pub use sinfonic_domain::SearchResults as SearchResultsAlias;

// Re-export selected domain types to keep the trait signature self-contained.
pub type AlbumsPage = PagedResponse<Album>;
pub type ArtistsPage = PagedResponse<Artist>;
pub type TracksPage = PagedResponse<Track>;
pub type GenresPage = PagedResponse<Genre>;
pub type PlaylistsPage = PagedResponse<Playlist>;
pub type SearchResultsView = DomainSearchResults;

// Silence unused-import warnings for the re-exports.
#[allow(dead_code)]
fn _ensure_used(_: PlaylistId) {}
