//! Subsonic/Navidrome `MusicProvider` implementation.
//!
//! # Architecture
//!
//! - `SubsonicClient` owns the `reqwest::Client` + base URL. Cloning
//!   it is cheap.
//! - `SubsonicSession` is the auth context (base URL + username +
//!   password). The session exposes a `sign()` method that
//!   regenerates a fresh `salt + token` pair for every call so we
//!   never reuse a token across requests.
//! - `SubsonicProvider::new(session)` returns a value that
//!   implements every `MusicProvider` method. The provider caches
//!   nothing: the `library` crate is the single source of truth for
//!   albums / artists / tracks. Provider methods fan out to HTTP
//!   and return parsed domain types.
//!
//! # Mapping convention
//!
//! Subsonic ids are stored in the SQLite cache with a `track-` /
//! `album-` / `artist-` / `playlist-` prefix (same convention as
//! Jellyfin). Stripping the prefix is required when calling
//! stream / image / star endpoints — see
//! `mapping::track_stream_url` for the canonical helper.

pub mod auth;
pub mod client;
pub mod dto;
pub mod mapping;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex as AsyncMutex;

use async_trait::async_trait;
use futures::stream::{StreamExt, TryStreamExt};
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, FolderDetail, FolderId, Genre, GenreDetail, GenreId,
    MusicFolder, MusicFolderId, PagedRequest, PagedResponse, Playlist, PlaylistDetail, PlaylistId,
    SearchResults, StreamDescriptor, Track, TrackId,
};
use sinfonic_source::{
    split_image_id, strip_prefix, slugify, AlbumDetailResponse, ArtistDetailResponse, FavoriteItemId,
    HomeSection, HomeSectionKind, ImageBytes, ImageMetadata, ImageRequest, Lyrics, MusicProvider,
    PlaybackReport, ProviderCapabilities, ProviderError, ProviderIdentity, ProviderResult,
    RandomTrackRequest,
};
use tauri::Emitter;

pub use auth::{LoginRequest, LoginSuccess, SubsonicSession};
use client::{SubsonicClient, SUBSONIC_API_VERSION};
use dto::{
    AlbumDetailPayload, AlbumListPayload, ArtistDetailPayload, ArtistsPayload, CreatePlaylistResponse,
    GenreDto, GenresPayload, IndexesPayload, PlaylistsPayload, PlaylistDetailPayload,
    PlaylistDto, RandomSongsPayload, SearchResult3Payload,
};

/// Progress event name for `sync-progress`. Now sourced from
/// `sinfonic_domain::events::EventName` so the wire string stays in
/// lockstep with the app crate and the frontend subscription.
const SYNC_PROGRESS_EVENT: &str = sinfonic_domain::EventName::SyncProgress.as_str();

/// Phase label sent in the `sync-progress` payload for the album
/// fan-out phase of `tracks()`. Free-form string the UI can branch on.
const TRACKS_PHASE: &str = "tracks";

/// Page size used internally when paginating `getAlbumList2` to
/// collect every album hint before slicing into tracks. Navidrome
/// caps responses at `getAlbumListMaxSize` (default 500) but legacy
/// Subsonic servers cap lower. 200 is small enough to fit every
/// server cap and large enough that the bootstrap loop stays
/// sub-second on libraries with thousands of albums.
const ALBUM_LIST_PAGE_SIZE: usize = 200;

/// Concurrency for the per-album `getAlbum` fan-out. 8 in-flight
/// requests is enough to saturate a typical home connection without
/// overwhelming small servers.
const ALBUM_FETCH_CONCURRENCY: usize = 8;

/// Payload of the `sync-progress` event. Kept in this crate so the
/// Album hint — `(id, song_count)` — collected from `getAlbumList2`
/// so `tracks()` and `sync_album_tracks` know which albums to fetch
/// and the total track count for the response.
#[derive(Clone, Debug)]
pub struct AlbumHint {
    pub id: String,
    pub song_count: u16,
}

/// Public entry point for the Subsonic provider.
#[derive(Clone)]
pub struct SubsonicProvider {
    session: SubsonicSession,
    client: SubsonicClient,
    identity: Arc<ProviderIdentity>,
    capabilities: Arc<ProviderCapabilities>,
    /// Optional Tauri handle. When `Some`, the provider emits
    /// `sync-progress` events during the long `tracks()` fan-out so
    /// the UI can show a progress bar. When `None` (tests, library
    /// users) sync still works but no events fire.
    app_handle: Option<tauri::AppHandle>,
    /// Cache of `(album_id, song_count)` pairs pulled from
    /// `getAlbumList2` during the first `tracks()` call. Subsequent
    /// page requests reuse the cached hints instead of refetching the
    /// whole album list — `getAlbumList2` paginates up to the server
    /// cap (500 by default), so 5000 albums = 10+ HTTP round-trips we
    /// otherwise repeat on every page request.
    album_hints_cache: Arc<AsyncMutex<Option<Vec<AlbumHint>>>>,
}

impl SubsonicProvider {
    /// Build a provider for an already-authenticated session. The
    /// `server_id` on the session is the value the library cache
    /// uses to scope subsequent reads.
    pub fn new(session: SubsonicSession) -> Result<Self, ProviderError> {
        let base_url = ::url::Url::parse(session.base_url.trim_end_matches('/'))
            .map_err(|e| ProviderError::Other(format!("invalid base_url: {e}")))?;
        let client = SubsonicClient::new(base_url)?;
        let identity = Arc::new(ProviderIdentity {
            provider_id: "subsonic".into(),
            server_id: session.server_id.clone(),
            server_name: session.base_url.clone(),
            user_id: session.username.clone(),
            username: session.username.clone(),
        });
        let capabilities = Arc::new(subsonic_capabilities());
        Ok(Self {
            session,
            client,
            identity,
            capabilities,
            app_handle: None,
            album_hints_cache: Arc::new(AsyncMutex::new(None)),
        })
    }

    /// Borrow the active session (used by tests + command layer).
    pub fn session(&self) -> &SubsonicSession {
        &self.session
    }

    /// Attach a Tauri `AppHandle` so the provider can emit
    /// `sync-progress` events during long fan-out phases. Without
    /// this, sync still works correctly — progress events just don't
    /// fire. Called once per provider instance, right after login
    /// or restore.
    pub fn with_app_handle(mut self, app: tauri::AppHandle) -> Self {
        self.app_handle = Some(app);
        self
    }

    /// `GET /rest/ping` — health check. Returns the
    /// `(server_name, server_type)` pair so the caller can refresh
    /// the identity without re-running the full login flow.
    pub async fn ping(&self) -> ProviderResult<(String, String)> {
        let auth = self.session.sign();
        let resp: dto::PingResponse = self
            .client
            .get_json("rest/ping", &auth, SUBSONIC_API_VERSION, [])
            .await?;
        Ok((resp.server_name, resp.server_type))
    }

    /// `GET /rest/getAlbumList2?type=newest&size=10` — used by
    /// `home_sections` to populate the "Newly added" rail.
    pub async fn fetch_recent_albums(
        &self,
        list_type: &str,
        size: usize,
    ) -> ProviderResult<Vec<Album>> {
        let auth = self.session.sign();
        let resp: AlbumListPayload = self
            .client
            .get_json(
                "rest/getAlbumList2",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("type", list_type.to_string()),
                    ("size", size.to_string()),
                ],
            )
            .await?;
        let list = resp.album_list2.unwrap_or_default();
        Ok(list
            .album
            .iter()
            .filter_map(mapping::album_from_dto)
            .collect())
    }

    /// `GET /rest/getArtists` (OpenSubsonic 1.16.1). Older servers
    /// use `getIndexes` which has a different shape — we fall back
    /// to that if `getArtists` returns no body.
    pub async fn fetch_artists(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>> {
        let auth = self.session.sign();
        // OpenSubsonic: getArtists with size + offset
        let resp: ArtistsPayload = self
            .client
            .get_json(
                "rest/getArtists",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("size", request.limit.to_string()),
                    ("offset", request.offset.to_string()),
                ],
            )
            .await?;
        let body = resp.artists.unwrap_or_default();
        let total = body.total_count;
        let artists: Vec<Artist> = body
            .index
            .iter()
            .flat_map(|g| g.artist.iter())
            .filter_map(mapping::artist_from_dto)
            .collect();
        if !artists.is_empty() {
            return Ok(PagedResponse::new(artists, total));
        }
        // Fallback: getIndexes (legacy Subsonic). No paging, return
        // everything flattened; the limit/offset is applied
        // client-side as a best effort.
        self.fetch_artists_legacy(request).await
    }

    async fn fetch_artists_legacy(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>> {
        let auth = self.session.sign();
        let resp: IndexesPayload = self
            .client
            .get_json("rest/getIndexes", &auth, SUBSONIC_API_VERSION, [])
            .await?;
        let body = resp.indexes.unwrap_or_default();
        let mut all: Vec<Artist> = body
            .index
            .iter()
            .flat_map(|g| g.artist.iter())
            .filter_map(mapping::artist_from_dto)
            .collect();
        let total = all.len();
        let start = request.offset.min(all.len());
        let end = (start + request.limit).min(all.len());
        all.drain(..start);
        all.truncate(end - start);
        Ok(PagedResponse::new(all, total))
    }

    /// `GET /rest/getAlbumList2?type=alphabeticalByName&size=N&offset=M`.
    pub async fn fetch_albums(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Album>> {
        let auth = self.session.sign();
        let resp: AlbumListPayload = self
            .client
            .get_json(
                "rest/getAlbumList2",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("type", "alphabeticalByName".to_string()),
                    ("size", request.limit.to_string()),
                    ("offset", request.offset.to_string()),
                ],
            )
            .await?;
        let list = resp.album_list2.unwrap_or_default();
        let total = list.album.len();
        let albums: Vec<Album> = list
            .album
            .iter()
            .filter_map(mapping::album_from_dto)
            .collect();
        Ok(PagedResponse::new(albums, total))
    }

    /// `GET /rest/search3?query=…&artistCount=…&albumCount=…&songCount=…`.
    pub async fn search_remote(
        &self,
        query: &str,
        artist_count: usize,
        album_count: usize,
        song_count: usize,
    ) -> ProviderResult<SearchResults> {
        let auth = self.session.sign();
        let resp: SearchResult3Payload = self
            .client
            .get_json(
                "rest/search3",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("query", query.to_string()),
                    ("artistCount", artist_count.to_string()),
                    ("albumCount", album_count.to_string()),
                    ("songCount", song_count.to_string()),
                ],
            )
            .await?;
        let body = resp.search_result3.unwrap_or_default();
        Ok(SearchResults {
            artists: body.artist.iter().filter_map(mapping::artist_from_dto).collect(),
            albums: body.album.iter().filter_map(mapping::album_from_dto).collect(),
            tracks: body.song.iter().filter_map(mapping::track_from_child).collect(),
            playlists: Vec::new(),
        })
    }
}

/// Subsonic-specific capability defaults. Enables everything the
/// actual trait surface supports; any flags that survived the
/// `feature/cleanup-phase2` audit are settable here.
fn subsonic_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        lyrics: true,
        playback_reporting: true,
        playlist_mutations: false,
        playlist_delete: false,
        favorite_mutations: true,
        ..ProviderCapabilities::default()
    }
}

#[async_trait]
impl MusicProvider for SubsonicProvider {
    fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    fn capabilities(&self) -> &ProviderCapabilities {
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
        let auth = self.session.sign();
        let resp: AlbumDetailPayload = self
            .client
            .get_json(
                "rest/getAlbum",
                &auth,
                SUBSONIC_API_VERSION,
                [("id", id.to_string())],
            )
            .await?;
        let album = mapping::album_from_dto(&resp.album)
            .ok_or_else(|| ProviderError::Other("album detail without id".into()))?;
        let tracks: Vec<Track> = resp
            .album
            .song
            .iter()
            .filter_map(mapping::track_from_child)
            .collect();
        Ok(AlbumDetailResponse {
            detail: sinfonic_domain::AlbumDetail { album, tracks },
        })
    }

    async fn tracks(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
        // Subsonic has no "list every song" endpoint — `search3` is
        // capped server-side (Navidrome: ~200), and `getIndexes` only
        // returns artists. The robust strategy is two-phase:
        //
        // 1. Paginate `getAlbumList2` to collect every album's
        //    `(id, song_count)`. Cheap, no per-album detail payload.
        // 2. Compute which album range covers the requested
        //    `[offset..offset+limit]` window and fan out `getAlbum`
        //    for just those albums in parallel (concurrency = 8).
        //    Emit `sync-progress` after each album completes.
        // 3. Concatenate and slice exactly to the requested window.
        //
        // Step 1 is cached at the provider level — the trait can't
        // expose a cache but `SubsonicProvider` is shared by reference
        // (it's wrapped in `Arc<dyn MusicProvider>`), so subsequent
        // page requests hit the in-memory hints instead of repeating
        // 10 HTTP round-trips per call. The cache lives for the
        // lifetime of the provider; it is cleared when the user
        // logs out and a new instance is built.
        let album_hints: Vec<AlbumHint> = {
            let mut cache = self.album_hints_cache.lock().await;
            if let Some(hints) = cache.as_ref() {
                hints.clone()
            } else {
                let fresh = self.collect_all_album_hints().await?;
                *cache = Some(fresh.clone());
                fresh
            }
        };
        let total_tracks: usize = album_hints.iter().map(|a| a.song_count as usize).sum();

        if album_hints.is_empty() || request.offset >= total_tracks {
            return Ok(PagedResponse::new(Vec::new(), total_tracks));
        }

        // Determine the inclusive album range that covers
        // `[request.offset .. request.offset+request.limit)`.
        let window_end = request.offset.saturating_add(request.limit).min(total_tracks);
        let mut cumulative: usize = 0;
        let mut start_idx: Option<usize> = None;
        let mut end_idx_exclusive: usize = 0;
        for (idx, album) in album_hints.iter().enumerate() {
            let next = cumulative + album.song_count as usize;
            if start_idx.is_none() && request.offset < next {
                start_idx = Some(idx);
            }
            if start_idx.is_some() && window_end <= next {
                end_idx_exclusive = idx + 1;
                break;
            }
            cumulative = next;
        }
        let Some(start_idx) = start_idx else {
            return Ok(PagedResponse::new(Vec::new(), total_tracks));
        };
        if end_idx_exclusive == 0 {
            end_idx_exclusive = album_hints.len();
        }

        // Fan out `getAlbum` for the selected albums. `buffered`
        // preserves submission order (unlike `buffer_unordered`) so
        // the resulting `chunks[i]` corresponds to
        // `album_hints[start_idx + i]`, which is what the slice at
        // the end assumes.
        let total_to_fetch = end_idx_exclusive - start_idx;
        let counter = Arc::new(AtomicUsize::new(0));
        let chunks: Vec<Vec<Track>> = futures::stream::iter(
            album_hints[start_idx..end_idx_exclusive]
                .iter()
                .cloned(),
        )
        .map(|hint| {
            let client = self.client.clone();
            let session = self.session.clone();
            let counter = Arc::clone(&counter);
            let app_handle = self.app_handle.clone();
            async move {
                let auth = session.sign();
                let resp: AlbumDetailPayload = client
                    .get_json(
                        "rest/getAlbum",
                        &auth,
                        SUBSONIC_API_VERSION,
                        [("id", hint.id.clone())],
                    )
                    .await?;
                let tracks: Vec<Track> = resp
                    .album
                    .song
                    .iter()
                    .filter_map(mapping::track_from_child)
                    .collect();
                let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(app) = app_handle.as_ref() {
                    let _ = app.emit(
                        SYNC_PROGRESS_EVENT,
                        sinfonic_domain::SyncProgressPayload {
                            phase: TRACKS_PHASE.to_string(),
                            done,
                            total: total_to_fetch,
                        },
                    );
                }
                Ok::<Vec<Track>, ProviderError>(tracks)
            }
        })
        .buffered(ALBUM_FETCH_CONCURRENCY)
        .try_collect()
        .await?;

        // Assemble and slice exactly to `[request.offset .. window_end)`.
        // `chunks[i]` corresponds to `album_hints[start_idx + i]`
        // because `buffered` preserves submission order.
        let mut all: Vec<Track> = Vec::new();
        for chunk in chunks {
            all.extend(chunk);
        }

        let cumulative_before_start: usize = album_hints[..start_idx]
            .iter()
            .map(|a| a.song_count as usize)
            .sum();
        let start_in_all = request.offset - cumulative_before_start;
        let end_in_all = (start_in_all + (window_end - request.offset)).min(all.len());
        let sliced: Vec<Track> = if start_in_all < all.len() {
            all.drain(start_in_all..end_in_all).collect()
        } else {
            Vec::new()
        };

        Ok(PagedResponse::new(sliced, total_tracks))
    }

    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        self.fetch_artists(request).await
    }

    async fn artist_detail(
        &self,
        artist_id: &ArtistId,
    ) -> ProviderResult<ArtistDetailResponse> {
        let id = artist_id.as_str().trim_start_matches("artist-");
        let auth = self.session.sign();
        let resp: ArtistDetailPayload = self
            .client
            .get_json(
                "rest/getArtist",
                &auth,
                SUBSONIC_API_VERSION,
                [("id", id.to_string())],
            )
            .await?;
        let artist = mapping::artist_from_dto(&resp.artist)
            .ok_or_else(|| ProviderError::Other("artist detail without id".into()))?;
        // The detail payload does not include the artist's albums;
        // we fetch them via a search by album-artist name (best
        // effort — Navidrome supports this and returns the artist's
        // full discography).
        let albums = self
            .fetch_albums_by_artist(&artist.name)
            .await
            .unwrap_or_default();
        Ok(ArtistDetailResponse {
            detail: sinfonic_domain::ArtistDetail { artist, albums },
        })
    }

    async fn playlists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Playlist>> {
        let auth = self.session.sign();
        // `?username=` scopes the response to playlists owned by the
        // current user (Navidrome) and hides public playlists shared
        // from other accounts on the same server. `?size=` and
        // `?offset=` mirror the request the orchestrator passes in
        // — without them the server returns the same first page on
        // every call (Navidrome's default page is 10) and the
        // orchestrator's `fetch_all_pages` loop trips its
        // `received < page_size` break on the first iteration.
        //
        // The server honours `offset`+`size`, so the response is
        // already the requested window. We just map it; the
        // orchestrator's loop terminates correctly when the server
        // returns fewer items than `limit` (the last page is short).
        let resp: PlaylistsPayload = self
            .client
            .get_json(
                "rest/getPlaylists",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("username", self.session.username.clone()),
                    ("size", request.limit.to_string()),
                    ("offset", request.offset.to_string()),
                ],
            )
            .await?;
        let playlists: Vec<Playlist> = resp
            .playlists
            .playlist
            .iter()
            .filter_map(mapping::playlist_from_dto)
            .collect();
        let total = playlists.len();
        Ok(PagedResponse::new(playlists, total))
    }

    async fn playlist_detail(
        &self,
        playlist_id: &PlaylistId,
    ) -> ProviderResult<PlaylistDetail> {
        let id = playlist_id.as_str().trim_start_matches("playlist-");
        let auth = self.session.sign();
        let resp: PlaylistDetailPayload = self
            .client
            .get_json(
                "rest/getPlaylist",
                &auth,
                SUBSONIC_API_VERSION,
                [("id", id.to_string())],
            )
            .await?;
        let playlist = mapping::playlist_from_dto(&PlaylistDto {
            id: resp.playlist.id.clone(),
            name: resp.playlist.name.clone(),
            song_count: resp.playlist.song_count,
            duration: resp.playlist.duration,
            owner: resp.playlist.owner.clone(),
            public: resp.playlist.public,
            created: None,
            changed: None,
            cover_art: resp.playlist.cover_art.clone(),
        })
        .ok_or_else(|| ProviderError::Other("playlist without id".into()))?;
        let tracks: Vec<Track> = resp
            .playlist
            .entry
            .iter()
            .filter_map(mapping::track_from_playlist_entry)
            .collect();
        Ok(PlaylistDetail { playlist, tracks })
    }

    async fn stream(&self, track_id: &TrackId) -> ProviderResult<StreamDescriptor> {
        let auth = self.session.sign();
        let url = mapping::track_stream_url(
            &self.session.base_url,
            track_id.as_str(),
            &auth.username,
            &auth.salt,
            &auth.token,
        );
        Ok(StreamDescriptor::new(url))
    }

    async fn search(&self, query: &str) -> ProviderResult<SearchResults> {
        if query.trim().is_empty() {
            return Ok(SearchResults::default());
        }
        self.search_remote(query, 10, 20, 30).await
    }

    async fn image_bytes(&self, request: ImageRequest) -> ProviderResult<ImageBytes> {
        let (_, real_id) = split_image_id(&request.item_id);
        let stripped = real_id
            .strip_prefix("track-")
            .or_else(|| real_id.strip_prefix("album-"))
            .or_else(|| real_id.strip_prefix("artist-"))
            .unwrap_or(real_id);
        // Subsonic's `getCoverArt` accepts an `id` plus an optional
        // `size` hint. We use the cache's `item_id` for the lookup
        // (album / track / artist — Subsonic doesn't care) and pass
        // `size` so servers that respect it (Navidrome does) return
        // a smaller payload.
        let path = format!("rest/getCoverArt?id={}&size={}", stripped, request.size);
        let auth = self.session.sign();
        let (bytes, content_type) = self
            .client
            .get_bytes(&path, &auth, SUBSONIC_API_VERSION)
            .await?;
        Ok(ImageBytes { bytes, content_type })
    }

    async fn set_favorite(
        &self,
        item: FavoriteItemId,
        favorite: bool,
    ) -> ProviderResult<()> {
        let (id, kind) = match item {
            FavoriteItemId::Track(t) => (
                strip_prefix(t.as_str(), "track-").to_string(),
                "song",
            ),
            FavoriteItemId::Album(a) => (
                strip_prefix(a.as_str(), "album-").to_string(),
                "album",
            ),
            FavoriteItemId::Artist(a) => (
                strip_prefix(a.as_str(), "artist-").to_string(),
                "artist",
            ),
        };
        let path = if favorite {
            "rest/star"
        } else {
            "rest/unstar"
        };
        let auth = self.session.sign();
        let _: serde_json::Value = self
            .client
            .post_json(
                path,
                &auth,
                SUBSONIC_API_VERSION,
                [("id", id.clone()), ("kind", kind.to_string())],
            )
            .await?;
        Ok(())
    }

    async fn lyrics(
        &self,
        track_id: &TrackId,
        _allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>> {
        let id = track_id.as_str().trim_start_matches("track-");
        let auth = self.session.sign();
        let resp: dto::LyricsPayload = self
            .client
            .get_json(
                "rest/getLyrics",
                &auth,
                SUBSONIC_API_VERSION,
                [("id", id.to_string())],
            )
            .await?;
        let plain = resp
            .lyrics
            .value
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let synced = synced_lines_to_lrc(&resp.lyrics.r#struct);
        if plain.is_none() && synced.is_none() {
            return Ok(None);
        }
        Ok(Some(Lyrics {
            plain,
            synced,
            source: Some("subsonic".to_string()),
        }))
    }

    async fn report_playback(&self, report: PlaybackReport) -> ProviderResult<()> {
        let id = strip_prefix(report.track_id.as_str(), "track-");
        let auth = self.session.sign();
        let submission = match report.kind {
            sinfonic_source::PlaybackReportKind::Started => "true",
            sinfonic_source::PlaybackReportKind::Progress => "false",
            sinfonic_source::PlaybackReportKind::Stopped => "true",
        };
        let _: serde_json::Value = self
            .client
            .post_json(
                "rest/scrobble",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("id", id.to_string()),
                    ("time", report.position_seconds.to_string()),
                    ("submission", submission.to_string()),
                ],
            )
            .await?;
        Ok(())
    }
}

/// Render a `lyrics.struct[]` payload as the LRC-flavoured string
/// the frontend expects (`[mm:ss.xx]line\n[mm:ss.xx]line…`).
///
/// Picks the synced entries when the server flags them with
/// `synced: true`; falls back to every entry when the flag is
/// absent (some servers omit it). Lines from multiple language
/// entries are flattened and sorted by their millisecond `start`
/// so multi-language servers don't shuffle playback order. Lines
/// without a `start` (rare for synced entries, but defensible for
/// unsynced fallbacks) are emitted with no timestamp prefix so the
/// frontend still sees the text. Returns `None` when no usable
/// lines exist.
fn synced_lines_to_lrc(entries: &[dto::LyricsStructEntryDto]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let has_synced_flag = entries.iter().any(|e| e.synced.is_some());
    let keep: Vec<&dto::LyricsStructEntryDto> = if has_synced_flag {
        entries.iter().filter(|e| e.synced.unwrap_or(false)).collect()
    } else {
        entries.iter().collect()
    };
    let mut lines: Vec<(&Option<u64>, &str)> = keep
        .iter()
        .flat_map(|e| e.line.iter())
        .map(|l| (&l.start, l.value.as_str()))
        .collect();
    if lines.is_empty() {
        return None;
    }
    // Sort by `start` (None last so unsynced trailing lines don't
    // jump to the top after a synced block).
    lines.sort_by_key(|(start, _)| start.unwrap_or(u64::MAX));
    let body = lines
        .into_iter()
        .map(|(start, text)| match start {
            Some(ms) => format!("[{}]{}", format_lrc_timestamp(*ms), text),
            None => text.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(body)
}

/// Format a millisecond offset as the LRC canonical `[mm:ss.xx]`.
/// Two-digit centisecond precision matches what the frontend's
/// `parseLrc` accepts (alongside `mm:ss` and `mm:ss.xxx`). The
/// caller guarantees `ms` is the line's `start`; no clamping for
/// tracks longer than 99 minutes (the LRC spec only allocates two
/// digits to minutes and overflow is the same edge case all LRC
/// parsers ignore).
fn format_lrc_timestamp(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let centiseconds = (ms % 1000) / 10;
    format!("{:02}:{:02}.{:02}", minutes, seconds, centiseconds)
}

// ─── Helpers ──────────────────────────────────────────────────────

impl SubsonicProvider {
    /// `GET /rest/getAlbumList2?type=alphabeticalByArtist&size=…` —
    /// best-effort fallback for `artist_detail.albums`. Navidrome
    /// supports it; legacy Subsonic returns an empty list and the
    /// caller just shows the artist header.
    async fn fetch_albums_by_artist(
        &self,
        artist_name: &str,
    ) -> ProviderResult<Vec<Album>> {
        let auth = self.session.sign();
        let resp: AlbumListPayload = self
            .client
            .get_json(
                "rest/getAlbumList2",
                &auth,
                SUBSONIC_API_VERSION,
                [
                    ("type", "byArtist".to_string()),
                    ("size", "200".to_string()),
                    ("offset", "0".to_string()),
                ],
            )
            .await?;
        let list = resp.album_list2.unwrap_or_default();
        let needle = artist_name.to_lowercase();
        Ok(list
            .album
            .iter()
            .filter(|a| {
                a.artist
                    .as_deref()
                    .map(|n| n.to_lowercase() == needle)
                    .unwrap_or(false)
            })
            .filter_map(mapping::album_from_dto)
            .collect())
    }

    /// Paginate `getAlbumList2` to collect every `(id, song_count)`
    /// pair on the server. Stops when the page comes back smaller
    /// than `ALBUM_LIST_PAGE_SIZE` (server cap reached). Used by
    /// `tracks()` to drive the album fan-out.
    async fn collect_all_album_hints(&self) -> ProviderResult<Vec<AlbumHint>> {
        let mut hints: Vec<AlbumHint> = Vec::new();
        let mut offset: usize = 0;
        loop {
            let page = self
                .fetch_albums(PagedRequest::new(offset, ALBUM_LIST_PAGE_SIZE))
                .await?;
            let received = page.items.len();
            for album in page.items {
                hints.push(AlbumHint {
                    id: album.id.as_str().trim_start_matches("album-").to_string(),
                    song_count: album.track_count,
                });
            }
            if received < ALBUM_LIST_PAGE_SIZE {
                break;
            }
            offset = offset.saturating_add(received);
        }
        Ok(hints)
    }

    /// Phase 3 of feature/direct-fetch-providers: fetch every album's
    /// tracks on the server in parallel and yield each completed
    /// batch through a callback. Used by the Tauri command
    /// `kick_subsonic_background_sync` to populate the SQLite
    /// `tracks` table so `/songs` and `ArtistDetailView` become
    /// instant reads.
    ///
    /// `on_batch` is invoked once per album with the parsed `Vec<Track>`
    /// (already mapped through `mapping::track_from_child`). The
    /// outer `ProviderResult` only fails on transport errors; partial
    /// batch failures are reported via `on_error` so a single broken
    /// album doesn't kill the whole sync.
    ///
    /// Concurrency is the same `ALBUM_FETCH_CONCURRENCY` used by
    /// `tracks()`'s windowed fan-out (8 in flight), which is enough to
    /// saturate a home connection without overwhelming small servers.
    pub async fn sync_album_tracks<F, E>(
        &self,
        on_batch: F,
        on_error: E,
    ) -> ProviderResult<SyncAlbumStats>
    where
        F: Fn(Album, Vec<Track>) + Send + Sync + 'static,
        E: Fn(&str, ProviderError) + Send + Sync + 'static,
    {
        let hints = self.collect_all_album_hints().await?;
        let total = hints.len();
        let counter = Arc::new(AtomicUsize::new(0));
        let on_batch = Arc::new(on_batch);
        let on_error = Arc::new(on_error);
        let app_handle = self.app_handle.clone();

        let stream = futures::stream::iter(hints.iter().cloned())
            .map(|hint| {
                let client = self.client.clone();
                let session = self.session.clone();
                let counter = Arc::clone(&counter);
                let app_handle = app_handle.clone();
                async move {
                    let auth = session.sign();
                    let result: Result<(Album, Vec<Track>), ProviderError> = async {
                        let resp: AlbumDetailPayload = client
                            .get_json(
                                "rest/getAlbum",
                                &auth,
                                SUBSONIC_API_VERSION,
                                [("id", hint.id.clone())],
                            )
                            .await?;
                        // `album_from_dto` always returns Some when
                        // the dto has a non-empty id (we just fetched
                        // it by id, so it must). Fall back to a
                        // minimal stub if it somehow doesn't — the
                        // track write will then fail FK and we'll
                        // surface the error via the on_error path.
                        let album = mapping::album_from_dto(&resp.album).ok_or_else(|| {
                            ProviderError::Other(format!(
                                "getAlbum returned no id for {}",
                                hint.id
                            ))
                        })?;
                        let tracks: Vec<Track> = resp
                            .album
                            .song
                            .iter()
                            .filter_map(mapping::track_from_child)
                            .collect();
                        Ok((album, tracks))
                    }
                    .await;
                    let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if let Some(app) = app_handle.as_ref() {
                        let _ = app.emit(
                            SYNC_PROGRESS_EVENT,
                            sinfonic_domain::SyncProgressPayload {
                                phase: TRACKS_PHASE.to_string(),
                                done,
                                total,
                            },
                        );
                    }
                    (hint, result)
                }
            })
            .buffered(ALBUM_FETCH_CONCURRENCY);

        use futures::StreamExt;
        let mut collected = stream.collect::<Vec<_>>().await;
        let mut tracks_total = 0usize;
        let mut albums_failed = 0usize;
        for (hint, result) in collected.drain(..) {
            match result {
                Ok((album, tracks)) => {
                    tracks_total += tracks.len();
                    on_batch(album, tracks);
                }
                Err(err) => {
                    albums_failed += 1;
                    on_error(&hint.id, err);
                }
            }
        }

        if albums_failed > 0 {
            eprintln!(
                "sinfonic::source_subsonic sync_album_tracks completed with partial failures: {albums_failed}/{total} albums, {tracks_total} tracks"
            );
        }
        Ok(SyncAlbumStats {
            albums_total: total,
            albums_failed,
            tracks_total,
        })
    }
}

/// Aggregate counters for a single `sync_album_tracks` pass.
#[derive(Clone, Copy, Debug)]
pub struct SyncAlbumStats {
    pub albums_total: usize,
    pub albums_failed: usize,
    pub tracks_total: usize,
}
