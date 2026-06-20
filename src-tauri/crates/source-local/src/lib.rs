//! Local-files `MusicProvider` (v0.3).
//!
//! Phase 0: skeleton. Real implementation lands in Phase 3 (post-MVP).
//! Heavy dependencies (`lofty`, `notify`, `walkdir`) ship with the impl.

#![allow(dead_code)]

pub mod client;

use async_trait::async_trait;
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, FolderDetail, FolderId, Genre, GenreDetail, GenreId,
    MusicFolder, MusicFolderId, PagedRequest, PagedResponse, Playlist, PlaylistDetail, PlaylistId,
    SearchResults, StreamDescriptor, Track, TrackId,
};
use sinfonic_source::{
    AlbumDetailResponse, ArtistDetailResponse, Capabilities, FavoriteItemId, HomeSection, Identity,
    ImageBytes, ImageMetadata, ImageRequest, Lyrics, MusicProvider, PlaybackReport,
    ProviderError, ProviderResult, RandomTrackRequest,
};

pub struct LocalProvider;

impl LocalProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicProvider for LocalProvider {
    fn identity(&self) -> &Identity {
        unimplemented!("LocalProvider::identity — Phase 3 (v0.3)")
    }

    fn capabilities(&self) -> &Capabilities {
        unimplemented!("LocalProvider::capabilities — Phase 3 (v0.3)")
    }

    async fn home_sections(&self) -> ProviderResult<Vec<HomeSection>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn albums(&self, _request: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn album_detail(&self, _id: &AlbumId) -> ProviderResult<AlbumDetailResponse> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn tracks(&self, _request: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn track(&self, _id: &TrackId) -> ProviderResult<Track> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn music_folders(&self) -> ProviderResult<Vec<MusicFolder>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn tracks_in_music_folder(
        &self,
        _id: &MusicFolderId,
        _request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Track>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn folder(
        &self,
        _id: Option<&FolderId>,
        _music_folder_id: Option<&MusicFolderId>,
    ) -> ProviderResult<FolderDetail> {
        Err(ProviderError::Unsupported("folder browsing"))
    }

    async fn artists(&self, _request: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn album_artists(
        &self,
        _request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn artist_detail(
        &self,
        _id: &ArtistId,
    ) -> ProviderResult<ArtistDetailResponse> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn genres(&self, _request: PagedRequest) -> ProviderResult<PagedResponse<Genre>> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn genre_detail(&self, _id: &GenreId) -> ProviderResult<GenreDetail> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn playlists(&self, _request: PagedRequest) -> ProviderResult<PagedResponse<Playlist>> {
        Err(ProviderError::Unsupported("playlists"))
    }

    async fn playlist_detail(&self, _id: &PlaylistId) -> ProviderResult<PlaylistDetail> {
        Err(ProviderError::Unsupported("playlist_detail"))
    }

    async fn random_tracks(&self, _req: RandomTrackRequest) -> ProviderResult<Vec<Track>> {
        Err(ProviderError::Unsupported("random_tracks"))
    }

    async fn stream(&self, _id: &TrackId) -> ProviderResult<StreamDescriptor> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn search(&self, _query: &str) -> ProviderResult<SearchResults> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn image_metadata(
        &self,
        _item_id: &str,
        _kind: sinfonic_domain::ImageKind,
    ) -> ProviderResult<ImageMetadata> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn image_bytes(&self, _request: ImageRequest) -> ProviderResult<ImageBytes> {
        Err(ProviderError::Other("Local not implemented in skeleton".into()))
    }

    async fn set_favorite(
        &self,
        _item: FavoriteItemId,
        _favorite: bool,
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("set_favorite"))
    }

    async fn create_playlist(
        &self,
        _name: &str,
        _track_ids: &[TrackId],
    ) -> ProviderResult<PlaylistId> {
        Err(ProviderError::Unsupported("create_playlist"))
    }

    async fn rename_playlist(&self, _id: &PlaylistId, _name: &str) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("rename_playlist"))
    }

    async fn delete_playlist(&self, _id: &PlaylistId) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("delete_playlist"))
    }

    async fn add_playlist_tracks(
        &self,
        _id: &PlaylistId,
        _track_ids: &[TrackId],
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("add_playlist_tracks"))
    }

    async fn remove_playlist_entries(
        &self,
        _id: &PlaylistId,
        _entries: &[String],
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("remove_playlist_entries"))
    }

    async fn move_playlist_entry(
        &self,
        _id: &PlaylistId,
        _entry: &str,
        _new_index: usize,
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("move_playlist_entry"))
    }

    async fn lyrics(
        &self,
        _id: &TrackId,
        _allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>> {
        Err(ProviderError::Unsupported("lyrics"))
    }

    async fn report_playback(&self, _report: PlaybackReport) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("report_playback"))
    }
}
