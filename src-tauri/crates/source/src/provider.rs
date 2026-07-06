//! `MusicProvider` trait.
//!
//! The trait surface is intentionally narrow — anything not wired up
//! in `commands::*` or `scrobble_watcher::*` was removed in the
//! `feature/cleanup-phase2` audit (e.g. `home_sections`,
//! `random_tracks`, `image_metadata`, every playlist mutation).
//! Methods the frontend never calls stay out of the trait so the
//! providers don't carry unimplemented boilerplate.

use async_trait::async_trait;

use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, PagedRequest, PagedResponse, Playlist, PlaylistDetail,
    PlaylistId, SearchResults, StreamDescriptor, Track, TrackId,
};

use crate::capabilities::ProviderCapabilities;
use crate::error::ProviderResult;
use crate::identity::ProviderIdentity;
use crate::types::{
    AlbumDetailResponse, ArtistDetailResponse, FavoriteItemId, ImageBytes, ImageRequest, Lyrics,
    PlaybackReport,
};

#[async_trait]
pub trait MusicProvider: Send + Sync {
    // ─── Identity & capabilities ────────────────────────────────
    fn identity(&self) -> &ProviderIdentity;
    fn capabilities(&self) -> &ProviderCapabilities;

    // ─── Albums ────────────────────────────────────────────────
    async fn albums(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Album>>;
    async fn album_detail(&self, album_id: &AlbumId) -> ProviderResult<AlbumDetailResponse>;

    // ─── Tracks ────────────────────────────────────────────────
    async fn tracks(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Track>>;

    // ─── Artists ───────────────────────────────────────────────
    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>>;
    async fn artist_detail(
        &self,
        artist_id: &ArtistId,
    ) -> ProviderResult<ArtistDetailResponse>;

    // ─── Genres & Playlists ────────────────────────────────────
    async fn playlists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Playlist>>;
    async fn playlist_detail(
        &self,
        playlist_id: &PlaylistId,
    ) -> ProviderResult<PlaylistDetail>;

    // ─── Streaming ─────────────────────────────────────────────
    async fn stream(&self, track_id: &TrackId) -> ProviderResult<StreamDescriptor>;

    // ─── Search ────────────────────────────────────────────────
    async fn search(&self, query: &str) -> ProviderResult<SearchResults>;

    // ─── Images ────────────────────────────────────────────────
    async fn image_bytes(&self, request: ImageRequest) -> ProviderResult<ImageBytes>;

    // ─── Favorites ─────────────────────────────────────────────
    async fn set_favorite(
        &self,
        item_id: FavoriteItemId,
        favorite: bool,
    ) -> ProviderResult<()>;

    // ─── Lyrics ────────────────────────────────────────────────
    async fn lyrics(
        &self,
        track_id: &TrackId,
        allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>>;

    // ─── Reporting ─────────────────────────────────────────────
    async fn report_playback(&self, report: PlaybackReport) -> ProviderResult<()>;
}
