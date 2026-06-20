//! `MusicProvider` trait.
//!
//! This is the full surface from `SINFONIC_ARCHITECTURE.md` § 6.
//! Providers implement everything; the frontend checks
//! `ProviderCapabilities` before invoking optional methods.

use async_trait::async_trait;

use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, FolderDetail, FolderId, Genre, GenreDetail, GenreId,
    MusicFolder, MusicFolderId, PagedRequest, PagedResponse, Playlist, PlaylistDetail, PlaylistId,
    SearchResults, StreamDescriptor, Track, TrackId,
};

use crate::capabilities::ProviderCapabilities;
use crate::error::ProviderResult;
use crate::identity::ProviderIdentity;
use crate::types::{
    AlbumDetailResponse, ArtistDetailResponse, FavoriteItemId, HomeSection, ImageBytes,
    ImageMetadata, ImageRequest, Lyrics, PlaybackReport, PlaybackReportKind, RandomTrackRequest,
    StreamRequest,
};

#[async_trait]
pub trait MusicProvider: Send + Sync {
    // ─── Identity & capabilities ────────────────────────────────
    fn identity(&self) -> &ProviderIdentity;
    fn capabilities(&self) -> &ProviderCapabilities;

    // ─── Home ───────────────────────────────────────────────────
    async fn home_sections(&self) -> ProviderResult<Vec<HomeSection>>;

    // ─── Albums ────────────────────────────────────────────────
    async fn albums(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Album>>;
    async fn album_detail(&self, album_id: &AlbumId) -> ProviderResult<AlbumDetailResponse>;

    // ─── Tracks ────────────────────────────────────────────────
    async fn tracks(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Track>>;
    async fn track(&self, track_id: &TrackId) -> ProviderResult<Track>;
    async fn music_folders(&self) -> ProviderResult<Vec<MusicFolder>>;
    async fn tracks_in_music_folder(
        &self,
        folder_id: &MusicFolderId,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Track>>;
    async fn folder(
        &self,
        folder_id: Option<&FolderId>,
        music_folder_id: Option<&MusicFolderId>,
    ) -> ProviderResult<FolderDetail>;

    // ─── Artists ───────────────────────────────────────────────
    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>>;
    async fn album_artists(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>>;
    async fn artist_detail(
        &self,
        artist_id: &ArtistId,
    ) -> ProviderResult<ArtistDetailResponse>;

    // ─── Genres & Playlists ────────────────────────────────────
    async fn genres(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Genre>>;
    async fn genre_detail(&self, genre_id: &GenreId) -> ProviderResult<GenreDetail>;
    async fn playlists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Playlist>>;
    async fn playlist_detail(
        &self,
        playlist_id: &PlaylistId,
    ) -> ProviderResult<PlaylistDetail>;

    // ─── Random ────────────────────────────────────────────────
    async fn random_tracks(
        &self,
        request: RandomTrackRequest,
    ) -> ProviderResult<Vec<Track>>;

    // ─── Streaming ─────────────────────────────────────────────
    async fn stream(&self, track_id: &TrackId) -> ProviderResult<StreamDescriptor>;
    async fn stream_with_request(
        &self,
        request: StreamRequest,
    ) -> ProviderResult<StreamDescriptor> {
        let _ = request;
        Err(crate::error::ProviderError::Unsupported(
            "stream_with_request not implemented",
        ))
    }

    // ─── Search ────────────────────────────────────────────────
    async fn search(&self, query: &str) -> ProviderResult<SearchResults>;

    // ─── Images ────────────────────────────────────────────────
    async fn image_metadata(
        &self,
        item_id: &str,
        kind: sinfonic_domain::ImageKind,
    ) -> ProviderResult<ImageMetadata>;
    async fn image_bytes(&self, request: ImageRequest) -> ProviderResult<ImageBytes>;

    // ─── Favorites ─────────────────────────────────────────────
    async fn set_favorite(
        &self,
        item_id: FavoriteItemId,
        favorite: bool,
    ) -> ProviderResult<()>;

    // ─── Playlists ─────────────────────────────────────────────
    async fn create_playlist(
        &self,
        name: &str,
        track_ids: &[TrackId],
    ) -> ProviderResult<PlaylistId>;
    async fn rename_playlist(&self, playlist_id: &PlaylistId, name: &str) -> ProviderResult<()>;
    async fn delete_playlist(&self, playlist_id: &PlaylistId) -> ProviderResult<()>;
    async fn add_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        track_ids: &[TrackId],
    ) -> ProviderResult<()>;
    async fn remove_playlist_entries(
        &self,
        playlist_id: &PlaylistId,
        entry_ids: &[String],
    ) -> ProviderResult<()>;
    async fn move_playlist_entry(
        &self,
        playlist_id: &PlaylistId,
        entry_id: &str,
        new_index: usize,
    ) -> ProviderResult<()>;

    // ─── Lyrics ────────────────────────────────────────────────
    async fn lyrics(
        &self,
        track_id: &TrackId,
        allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>>;

    // ─── Reporting ─────────────────────────────────────────────
    async fn report_playback(&self, report: PlaybackReport) -> ProviderResult<()>;

    // ─── Internal helpers (not part of the public contract) ────
    #[doc(hidden)]
    fn _unused(&self) {
        // Forces the compiler to keep the kind variants referenced in
        // case a provider impl never uses them.
        let _ = PlaybackReportKind::Started;
    }
}
