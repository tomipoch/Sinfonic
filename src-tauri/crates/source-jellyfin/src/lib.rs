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
    Album, AlbumId, Artist, ArtistId, PagedRequest, PagedResponse, Playlist, PlaylistDetail, PlaylistId,
    SearchResults, ServerId, StreamDescriptor, Track, TrackId,
};
use sinfonic_source::{
    split_image_id, strip_prefix, AlbumDetailResponse, ArtistDetailResponse, Capabilities,
    FavoriteItemId, Identity, ImageBytes, ImageRequest, Lyrics,
    MusicProvider, PlaybackReport, ProviderCapabilities, ProviderError, ProviderIdentity,
    ProviderResult,
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
        playlist_mutations: false,
        playlist_delete: false,
        favorite_mutations: true,
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

    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
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
