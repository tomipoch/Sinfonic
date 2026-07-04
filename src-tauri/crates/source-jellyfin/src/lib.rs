//! Jellyfin `MusicProvider` implementation.
//!
//! # Architecture
//!
//! - `JellyfinClient` owns the `reqwest::Client` + base URL. Cloning
//!   it is cheap.
//! - `JellyfinSession` is the auth context (base URL + token + user
//!   id). `JellyfinProvider::new(session)` returns a value that
//!   implements every `MusicProvider` method.
//! - The provider caches nothing: the `library` crate is the single
//!   source of truth for albums / artists / tracks. Provider methods
//!   fan out to HTTP and return parsed domain types.
//!
//! # Mapping convention
//!
//! Jellyfin item ids are stored in the SQLite cache with a `track-`
//! / `album-` / `artist-` / `playlist-` prefix. Stripping the prefix
//! is required when calling image or stream endpoints — see
//! `mapping::track_stream_url` for the canonical helper.

pub mod auth;
pub mod client;
pub mod discovery;
pub mod dto;
pub mod mapping;

use std::sync::Arc;

use async_trait::async_trait;
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, FolderDetail, FolderId, Genre, GenreDetail, GenreId,
    MusicFolder, MusicFolderId, PagedRequest, PagedResponse, Playlist, PlaylistDetail, PlaylistId,
    SearchResults, ServerId, StreamDescriptor, Track, TrackId,
};
use sinfonic_source::{
    AlbumDetailResponse, ArtistDetailResponse, Capabilities, FavoriteItemId, HomeSection, Identity,
    ImageBytes, ImageMetadata, ImageRequest, Lyrics, MusicProvider, PlaybackReport,
    ProviderCapabilities, ProviderError, ProviderIdentity, ProviderResult, RandomTrackRequest,
};
use url::Url;

use crate::client::{AuthContext, JellyfinClient};
use crate::dto::{BaseItemDto, ItemsResponse};

/// Holds a connection session with a Jellyfin server. Cheap to clone.
#[derive(Clone, Debug)]
pub struct JellyfinSession {
    pub server_id: ServerId,
    pub base_url: String,
    pub access_token: String,
    pub user_id: String,
    pub device_id: String,
}

impl JellyfinSession {
    /// Build the `AuthContext` the HTTP client uses for signing.
    fn auth(&self) -> AuthContext {
        AuthContext {
            device_id: self.device_id.clone(),
            access_token: Some(self.access_token.clone()),
        }
    }

    /// Build a URL the rest of the app can hand to an audio player.
    /// The token is embedded as `api_key` because some clients (mpv,
    /// web audio) can't send the `X-Emby-Authorization` header.
    pub fn stream_url(&self, track_id: &TrackId) -> String {
        mapping::track_stream_url(&self.base_url, track_id.as_str(), &self.access_token)
    }
}

/// Public entry point for the Jellyfin provider.
#[derive(Clone)]
pub struct JellyfinProvider {
    session: JellyfinSession,
    client: JellyfinClient,
    identity: Arc<ProviderIdentity>,
    capabilities: Arc<ProviderCapabilities>,
}

impl JellyfinProvider {
    /// Build a provider for an already-authenticated session.
    pub fn new(session: JellyfinSession) -> Result<Self, ProviderError> {
        let base_url = Url::parse(session.base_url.trim_end_matches('/'))
            .map_err(|e| ProviderError::Other(format!("invalid base_url: {e}")))?;
        let client = JellyfinClient::new(base_url)?;
        let identity = Arc::new(ProviderIdentity {
            provider_id: "jellyfin".into(),
            server_id: session.server_id.clone(),
            server_name: session.base_url.clone(),
            user_id: session.user_id.clone(),
            username: String::new(),
        });
        let capabilities = Arc::new(jellyfin_capabilities());
        Ok(Self {
            session,
            client,
            identity,
            capabilities,
        })
    }

    /// Borrow the active session (used by tests + command layer).
    pub fn session(&self) -> &JellyfinSession {
        &self.session
    }

    /// `GET /System/Info/Public` — fetch the server's display name
    /// and store it on the provider identity. The first call may
    /// rename the `ServerId` row in the cache.
    pub async fn refresh_server_name(&self) -> ProviderResult<String> {
        let info = self
            .client
            .get_json::<dto::PublicSystemInfo>("System/Info/Public", &self.session.auth())
            .await?;
        Ok(info.server_name)
    }

    /// `GET /Items?IncludeItemTypes=MusicAlbum&Recursive=true` with
    /// paging — used by `sync_albums` style flows.
    pub async fn fetch_albums(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Album>> {
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(
                &items_query(&ItemsQuery {
                    types: "MusicAlbum",
                    user_id: Some(&self.session.user_id),
                    start_index: Some(request.offset),
                    limit: Some(request.limit),
                    sort_by: Some("SortName"),
                    recursive: true,
                    fields: Some("Genres,ChildCount,ProductionYear,RunTimeTicks,PrimaryImageAspectRatio,UserData"),
                }),
                &self.session.auth(),
            )
            .await?;
        let albums: Vec<Album> = resp
            .items
            .iter()
            .filter_map(mapping::album_from_dto)
            .collect();
        Ok(PagedResponse::new(albums, resp.total_record_count))
    }

    pub async fn fetch_artists(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>> {
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(
                &items_query(&ItemsQuery {
                    types: "MusicArtist",
                    user_id: Some(&self.session.user_id),
                    start_index: Some(request.offset),
                    limit: Some(request.limit),
                    sort_by: Some("SortName"),
                    recursive: true,
                    fields: Some("ChildCount,PrimaryImageAspectRatio,UserData"),
                }),
                &self.session.auth(),
            )
            .await?;
        let artists: Vec<Artist> = resp
            .items
            .iter()
            .filter_map(mapping::artist_from_dto)
            .collect();
        Ok(PagedResponse::new(artists, resp.total_record_count))
    }

    pub async fn fetch_tracks(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Track>> {
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(
                &items_query(&ItemsQuery {
                    types: "Audio",
                    user_id: Some(&self.session.user_id),
                    start_index: Some(request.offset),
                    limit: Some(request.limit),
                    sort_by: Some("SortName"),
                    recursive: true,
                    fields: Some(
                        "Genres,ChildCount,ProductionYear,RunTimeTicks,IndexNumber,ParentIndexNumber,Album,AlbumArtists,ArtistItems,UserData",
                    ),
                }),
                &self.session.auth(),
            )
            .await?;
        let tracks: Vec<Track> = resp
            .items
            .iter()
            .filter_map(mapping::track_from_dto)
            .collect();
        Ok(PagedResponse::new(tracks, resp.total_record_count))
    }

    pub async fn search_remote(
        &self,
        query: &str,
    ) -> ProviderResult<SearchResults> {
        // v0.1: empty results. The full Jellyfin `/Search/Hints`
        // endpoint returns a different DTO shape; the local FTS5
        // search command on the Tauri layer covers most queries
        // once a sync has been performed. Once `/Search/Hints` is
        // needed we add it here without touching the trait surface.
        let _ = query;
        Ok(SearchResults::default())
    }
}

/// Jellyfin-specific capability defaults. We enable everything
/// supported by the public API and disable what isn't (lyrics,
/// playback reporting, playlist mutations).
fn jellyfin_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        // Jellyfin has `/Audio/{Id}/Lyrics` but we don't speak it
        // yet; declaring `true` lets the frontend attempt the lookup
        // so the LRCLIB fallback in `commands::get_lyrics` runs
        // when Jellyfin itself comes up empty. The provider's
        // `lyrics()` implementation still returns `Ok(None)`.
        lyrics: true,
        playback_reporting: true,
        playlist_mutations: true,
        playlist_delete: true,
        favorite_mutations: true,
        auto_dj: false,
        random_tracks: false,
        random_played_filter: false,
        music_folders: false,
        folder_browsing: false,
        ..ProviderCapabilities::default()
    }
}

#[async_trait]
impl MusicProvider for JellyfinProvider {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn home_sections(&self) -> ProviderResult<Vec<HomeSection>> {
        // v0.1: surface the most recently added albums.
        let albums = self
            .fetch_albums(PagedRequest::new(0, 10))
            .await?
            .items;
        Ok(vec![HomeSection {
            kind: sinfonic_source::HomeSectionKind::NewlyAdded,
            albums,
            tracks: Vec::new(),
        }])
    }

    async fn albums(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
        self.fetch_albums(request).await
    }

    async fn album_detail(
        &self,
        album_id: &AlbumId,
    ) -> ProviderResult<AlbumDetailResponse> {
        let id = album_id.as_str().trim_start_matches("album-");
        let dto: BaseItemDto = self
            .client
            .get_json(&format!("Items/{id}"), &self.session.auth())
            .await?;
        let album = mapping::album_from_dto(&dto)
            .ok_or_else(|| ProviderError::Other("album detail without id".into()))?;
        // Album tracks: same query as fetch_tracks but filtered by
        // parent id.
        let url = format!(
            "Items?ParentId={id}&IncludeItemTypes=Audio&Recursive=true&Fields=IndexNumber,ParentIndexNumber,RunTimeTicks,AlbumArtists,ArtistItems,UserData"
        );
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(&url, &self.session.auth())
            .await?;
        let tracks = resp
            .items
            .iter()
            .filter_map(mapping::track_from_dto)
            .collect();
        Ok(AlbumDetailResponse {
            detail: sinfonic_domain::AlbumDetail { album, tracks },
        })
    }

    async fn tracks(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
        self.fetch_tracks(request).await
    }

    async fn track(&self, track_id: &TrackId) -> ProviderResult<Track> {
        let id = track_id.as_str().trim_start_matches("track-");
        let dto: BaseItemDto = self
            .client
            .get_json(&format!("Items/{id}"), &self.session.auth())
            .await?;
        mapping::track_from_dto(&dto)
            .ok_or_else(|| ProviderError::Other("track detail without id".into()))
    }

    async fn music_folders(&self) -> ProviderResult<Vec<MusicFolder>> {
        Err(ProviderError::Unsupported("music_folders"))
    }

    async fn tracks_in_music_folder(
        &self,
        _id: &MusicFolderId,
        _request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Track>> {
        Err(ProviderError::Unsupported("tracks_in_music_folder"))
    }

    async fn folder(
        &self,
        _id: Option<&FolderId>,
        _music_folder_id: Option<&MusicFolderId>,
    ) -> ProviderResult<FolderDetail> {
        Err(ProviderError::Unsupported("folder"))
    }

    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        self.fetch_artists(request).await
    }

    async fn album_artists(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>> {
        self.fetch_artists(request).await
    }

    async fn artist_detail(
        &self,
        artist_id: &ArtistId,
    ) -> ProviderResult<ArtistDetailResponse> {
        let id = artist_id.as_str().trim_start_matches("artist-");
        let dto: BaseItemDto = self
            .client
            .get_json(&format!("Items/{id}"), &self.session.auth())
            .await?;
        let artist = mapping::artist_from_dto(&dto)
            .ok_or_else(|| ProviderError::Other("artist detail without id".into()))?;
        let url = format!(
            "Items?AlbumArtistIds={id}&IncludeItemTypes=MusicAlbum&Recursive=true&Fields=ChildCount,ProductionYear,RunTimeTicks,Genres,UserData"
        );
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(&url, &self.session.auth())
            .await?;
        let albums = resp
            .items
            .iter()
            .filter_map(mapping::album_from_dto)
            .collect();
        Ok(ArtistDetailResponse {
            detail: sinfonic_domain::ArtistDetail { artist, albums },
        })
    }

    async fn genres(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Genre>> {
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(
                &items_query(&ItemsQuery {
                    types: "MusicGenre",
                    user_id: Some(&self.session.user_id),
                    start_index: Some(request.offset),
                    limit: Some(request.limit),
                    sort_by: Some("SortName"),
                    recursive: true,
                    fields: Some("ChildCount"),
                }),
                &self.session.auth(),
            )
            .await?;
        let genres = resp
            .items
            .iter()
            .filter_map(|d| {
                if d.id.is_empty() {
                    return None;
                }
                Some(Genre {
                    id: GenreId::new(format!("genre-{}", d.id)),
                    name: d.name.clone().unwrap_or_default(),
                    album_count: 0,
                    track_count: 0,
                })
            })
            .collect();
        Ok(PagedResponse::new(genres, resp.total_record_count))
    }

    async fn genre_detail(&self, _id: &GenreId) -> ProviderResult<GenreDetail> {
        Err(ProviderError::Unsupported("genre_detail"))
    }

    async fn playlists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Playlist>> {
        let resp: ItemsResponse<dto::PlaylistDto> = self
            .client
            .get_json(
                &items_query(&ItemsQuery {
                    types: "Playlist",
                    user_id: Some(&self.session.user_id),
                    start_index: Some(request.offset),
                    limit: Some(request.limit),
                    sort_by: Some("SortName"),
                    recursive: true,
                    fields: Some("CumulativeRunTimeTicks,ChildCount,OpenAccess,ImageTags"),
                }),
                &self.session.auth(),
            )
            .await?;
        let playlists = resp
            .items
            .iter()
            .filter_map(mapping::playlist_from_dto)
            .collect();
        Ok(PagedResponse::new(playlists, resp.total_record_count))
    }

    async fn playlist_detail(
        &self,
        playlist_id: &PlaylistId,
    ) -> ProviderResult<PlaylistDetail> {
        let id = playlist_id.as_str().trim_start_matches("playlist-");
        let dto: dto::PlaylistDto = self
            .client
            .get_json(&format!("Playlists/{id}?Fields=ImageTags"), &self.session.auth())
            .await?;
        let playlist = mapping::playlist_from_dto(&dto)
            .ok_or_else(|| ProviderError::Other("playlist without id".into()))?;
        let url = format!(
            "Playlists/{id}/Items?UserId={}&Fields=IndexNumber,ParentIndexNumber,RunTimeTicks,AlbumArtists,ArtistItems,UserData",
            self.session.user_id
        );
        let resp: ItemsResponse<BaseItemDto> = self
            .client
            .get_json(&url, &self.session.auth())
            .await?;
        let tracks = resp
            .items
            .iter()
            .filter_map(mapping::track_from_dto)
            .collect();
        Ok(PlaylistDetail { playlist, tracks })
    }

    async fn random_tracks(
        &self,
        _request: RandomTrackRequest,
    ) -> ProviderResult<Vec<Track>> {
        Err(ProviderError::Unsupported("random_tracks"))
    }

    async fn stream(&self, track_id: &TrackId) -> ProviderResult<StreamDescriptor> {
        Ok(StreamDescriptor::new(self.session.stream_url(track_id)))
    }

    async fn search(&self, query: &str) -> ProviderResult<SearchResults> {
        if query.trim().is_empty() {
            return Ok(SearchResults::default());
        }
        // The local FTS5 cache is what powers search results; the
        // remote provider doesn't need to be queried on every
        // keystroke. Callers wire `jellyfin_sync_library` to refresh
        // the cache and then call the `search` Tauri command.
        let _ = query;
        Ok(SearchResults::default())
    }

    async fn image_metadata(
        &self,
        item_id: &str,
        kind: sinfonic_domain::ImageKind,
    ) -> ProviderResult<ImageMetadata> {
        let (kind_part, real_id) = split_image_id(item_id);
        let kind_str = match kind {
            sinfonic_domain::ImageKind::Primary => "Primary",
            sinfonic_domain::ImageKind::Backdrop => "Backdrop",
        };
        // The kind in the ImageRef (e.g. "Primary") takes precedence
        // when present — the caller may not have known the original
        // kind, but the cache tagged it.
        let kind_str = if kind_part.is_empty() { kind_str } else { kind_part };
        Ok(ImageMetadata {
            item_id: real_id.to_string(),
            kind,
            tag: None,
            url: format!(
                "{}/Items/{}/Images/{}?maxHeight=600&quality=90",
                self.session.base_url.trim_end_matches('/'),
                real_id,
                kind_str,
            ),
        })
    }

    async fn image_bytes(&self, request: ImageRequest) -> ProviderResult<ImageBytes> {
        let (kind_part, real_id) = split_image_id(&request.item_id);
        let kind_str = match request.kind {
            sinfonic_domain::ImageKind::Primary => "Primary",
            sinfonic_domain::ImageKind::Backdrop => "Backdrop",
        };
        let kind_str = if kind_part.is_empty() { kind_str } else { kind_part };
        let path = format!(
            "Items/{}/Images/{}?maxHeight={}&quality=90",
            real_id,
            kind_str,
            request.size,
        );
        let (bytes, content_type) =
            self.client.get_bytes(&path, &self.session.auth()).await?;
        Ok(ImageBytes { bytes, content_type })
    }

    async fn set_favorite(
        &self,
        item: FavoriteItemId,
        favorite: bool,
    ) -> ProviderResult<()> {
        let id: String = match item {
            FavoriteItemId::Track(t) => strip_prefix(t.as_str(), "track-").to_string(),
            FavoriteItemId::Album(a) => strip_prefix(a.as_str(), "album-").to_string(),
            FavoriteItemId::Artist(a) => strip_prefix(a.as_str(), "artist-").to_string(),
        };
        self.client
            .post_json::<_, ()>(
                &format!("Users/{}/FavoriteItems/{}", self.session.user_id, id),
                &self.session.auth(),
                &serde_json::json!({ "IsFavorite": favorite }),
            )
            .await?;
        Ok(())
    }

    async fn create_playlist(
        &self,
        name: &str,
        track_ids: &[TrackId],
    ) -> ProviderResult<PlaylistId> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            #[serde(rename = "Name")]
            name: &'a str,
            #[serde(rename = "UserId")]
            user_id: &'a str,
            #[serde(rename = "MediaType")]
            media_type: &'a str,
            #[serde(rename = "Ids")]
            ids: Vec<String>,
        }
        let ids: Vec<String> = track_ids
            .iter()
            .map(|t| strip_prefix(t.as_str(), "track-").to_string())
            .collect();
        let body = Body {
            name,
            user_id: &self.session.user_id,
            media_type: "Audio",
            ids,
        };
        let resp: dto::PlaylistDto = self
            .client
            .post_json("Playlists", &self.session.auth(), &body)
            .await?;
        Ok(PlaylistId::from_external(&resp.id))
    }

    async fn rename_playlist(
        &self,
        playlist_id: &PlaylistId,
        name: &str,
    ) -> ProviderResult<()> {
        let id = strip_prefix(playlist_id.as_str(), "playlist-");
        let body = serde_json::json!({ "Name": name });
        self.client
            .post_json::<_, ()>(
                &format!("Playlists/{id}"),
                &self.session.auth(),
                &body,
            )
            .await?;
        Ok(())
    }

    async fn delete_playlist(&self, playlist_id: &PlaylistId) -> ProviderResult<()> {
        let id = strip_prefix(playlist_id.as_str(), "playlist-");
        self.client
            .delete(&format!("Playlists/{id}"), &self.session.auth())
            .await
    }

    async fn add_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        track_ids: &[TrackId],
    ) -> ProviderResult<()> {
        let id = strip_prefix(playlist_id.as_str(), "playlist-");
        let ids: Vec<String> = track_ids
            .iter()
            .map(|t| strip_prefix(t.as_str(), "track-").to_string())
            .collect();
        let body = serde_json::json!({ "Ids": ids });
        self.client
            .post_json::<_, ()>(
                &format!("Playlists/{id}/Items"),
                &self.session.auth(),
                &body,
            )
            .await?;
        Ok(())
    }

    async fn remove_playlist_entries(
        &self,
        playlist_id: &PlaylistId,
        entry_ids: &[String],
    ) -> ProviderResult<()> {
        let id = strip_prefix(playlist_id.as_str(), "playlist-");
        let body = serde_json::json!({ "EntryIds": entry_ids });
        self.client
            .delete_with_body(
                &format!("Playlists/{id}/Items"),
                &self.session.auth(),
                &body,
            )
            .await
    }

    async fn move_playlist_entry(
        &self,
        playlist_id: &PlaylistId,
        entry_id: &str,
        new_index: usize,
    ) -> ProviderResult<()> {
        let id = strip_prefix(playlist_id.as_str(), "playlist-");
        let body = serde_json::json!({
            "PlaylistItemId": entry_id,
            "NewIndex": new_index,
        });
        self.client
            .post_json::<_, ()>(
                &format!("Playlists/{id}/Items/{entry_id}/Move/{new_index}"),
                &self.session.auth(),
                &body,
            )
            .await?;
        Ok(())
    }

    async fn lyrics(
        &self,
        _track_id: &TrackId,
        _allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>> {
        Ok(None)
    }

    async fn report_playback(&self, report: PlaybackReport) -> ProviderResult<()> {
        let id = strip_prefix(report.track_id.as_str(), "track-");
        let body = serde_json::json!({
            "ItemId": id,
            "PositionTicks": report.position_seconds as u64 * 10_000_000,
            "IsPaused": report.paused,
            "IsMuted": report.muted,
            "VolumeLevel": report.volume_percent,
            "PlayMethod": "DirectStream",
            "PlaySessionId": "",
        });
        self.client
            .post_json::<_, ()>(
                "Sessions/Playing",
                &self.session.auth(),
                &body,
            )
            .await?;
        Ok(())
    }
}

// ─── Helpers ──────────────────────────────────────────────────────

struct ItemsQuery<'a> {
    types: &'a str,
    user_id: Option<&'a str>,
    start_index: Option<usize>,
    limit: Option<usize>,
    sort_by: Option<&'a str>,
    recursive: bool,
    fields: Option<&'a str>,
}

fn items_query(q: &ItemsQuery<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !q.types.is_empty() {
        parts.push(format!("IncludeItemTypes={}", q.types));
    }
    if let Some(u) = q.user_id {
        parts.push(format!("UserId={}", urlencoded(u)));
    }
    if let Some(s) = q.start_index {
        parts.push(format!("StartIndex={s}"));
    }
    if let Some(l) = q.limit {
        parts.push(format!("Limit={l}"));
    }
    if let Some(s) = q.sort_by {
        parts.push(format!("SortBy={}", urlencoded(s)));
    }
    if q.recursive {
        parts.push("Recursive=true".into());
    }
    if let Some(f) = q.fields {
        parts.push(format!("Fields={}", urlencoded(f)));
    }
    format!("Items?{}", parts.join("&"))
}

fn urlencoded(s: &str) -> String {
    // Jellyfin queries use the standard URL form. `url::form_urlencoded`
    // is overkill for the values we emit; a hand-rolled encoder keeps
    // the dep surface tiny.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn strip_prefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    sinfonic_source::strip_prefix(s, prefix)
}

fn split_image_id(item_id: &str) -> (&str, &str) {
    sinfonic_source::split_image_id(item_id)
}