// Sync order regression test — `commands::sync_library_data` must
// write artists before albums, because `albums.artist_id` is a
// foreign key into `artists(server_id, artist_id)` (see
// `library::schema::INITIAL_SCHEMA`).
//
// The original bug: `provider_sync_library` wrote albums first, which
// raised `sqlite error: FOREIGN KEY constraint failed` on the very
// first sync against any Subsonic-shaped provider that populated
// `artist_id` on its albums (Navidrome, Gonic, Airsonic, etc.).
// Subsonic's `album_from_dto` always sets `artist_id` when the DTO
// carries one, so this affected every Navidrome user.
//
// The test runs the pure helper (no Tauri runtime) against an
// in-memory `Store` and a stub `MusicProvider` that returns the
// smallest possible dataset — 1 artist, 1 album that references
// the artist, 1 track that references the album. If the helper
// writes albums before artists, the FK check at commit time fails.

use std::sync::Arc;

use async_trait::async_trait;
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, FolderDetail, FolderId, Genre, GenreDetail, GenreId,
    MusicFolder, MusicFolderId, PagedRequest, PagedResponse, Playlist, PlaylistDetail, PlaylistId,
    SearchResults, ServerId, StreamDescriptor, Track, TrackId,
};
use sinfonic_library::Store;
use sinfonic_source::{
    AlbumDetailResponse, ArtistDetailResponse, FavoriteItemId, HomeSection, ImageBytes,
    ImageMetadata, ImageRequest, Lyrics, MusicProvider, PlaybackReport, ProviderCapabilities,
    ProviderError, ProviderIdentity, ProviderResult, RandomTrackRequest, StreamRequest,
};

/// Stub provider that returns a single artist / album / track on each
/// fetch. Every other method is unimplemented; the sync pipeline only
/// touches `albums`, `artists`, `tracks`, `identity`, `capabilities`.
struct StubProvider {
    identity: ProviderIdentity,
    artists: Vec<Artist>,
    albums: Vec<Album>,
    tracks: Vec<Track>,
}

impl StubProvider {
    fn new(server_id: ServerId) -> Self {
        let artist_id = ArtistId::new("artist-radiohead");
        let album_id = AlbumId::new("album-okc");
        let track_id = TrackId::new("track-airbag");
        let artists = vec![Artist {
            id: artist_id.clone(),
            name: "Radiohead".into(),
            album_count: 1,
            track_count: 1,
            favorite: false,
            image_ref: None,
        }];
        let albums = vec![Album {
            id: album_id.clone(),
            title: "OK Computer".into(),
            artist: "Radiohead".into(),
            // The crucial line: album references an artist row that
            // must be inserted first. A FK violation here is the
            // exact failure mode we are regression-testing for.
            artist_id: Some(artist_id),
            year: Some(1997),
            track_count: 1,
            duration_seconds: 270,
            favorite: false,
            image_ref: None,
            genres: vec![],
        }];
        let tracks = vec![Track {
            id: track_id,
            album_id,
            title: "Airbag".into(),
            artist: "Radiohead".into(),
            artist_id: Some(ArtistId::new("artist-radiohead")),
            album: "OK Computer".into(),
            duration_seconds: 270,
            track_number: 1,
            disc_number: 1,
            favorite: false,
            image_ref: None,
        }];
        Self {
            identity: ProviderIdentity {
                provider_id: "stub".into(),
                server_id,
                server_name: "stub server".into(),
                user_id: "u".into(),
                username: "u".into(),
            },
            artists,
            albums,
            tracks,
        }
    }
}

#[async_trait]
impl MusicProvider for StubProvider {
    fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }
    fn capabilities(&self) -> &ProviderCapabilities {
        // Capabilities are unused by `sync_library_data`, but the
        // trait still requires a value. Return the default.
        static CAPS: std::sync::OnceLock<ProviderCapabilities> = std::sync::OnceLock::new();
        CAPS.get_or_init(ProviderCapabilities::default)
    }

    async fn albums(&self, _req: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
        Ok(PagedResponse::new(self.albums.clone(), self.albums.len()))
    }
    async fn album_detail(&self, _: &AlbumId) -> ProviderResult<AlbumDetailResponse> {
        unimplemented!()
    }
    async fn tracks(&self, _req: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
        Ok(PagedResponse::new(self.tracks.clone(), self.tracks.len()))
    }
    async fn artists(&self, _req: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        Ok(PagedResponse::new(self.artists.clone(), self.artists.len()))
    }
    async fn artist_detail(&self, _: &ArtistId) -> ProviderResult<ArtistDetailResponse> {
        unimplemented!()
    }
    async fn playlists(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Playlist>> {
        Ok(PagedResponse::new(vec![], 0))
    }
    async fn playlist_detail(&self, _: &PlaylistId) -> ProviderResult<PlaylistDetail> {
        unimplemented!()
    }
    async fn stream(&self, _: &TrackId) -> ProviderResult<StreamDescriptor> {
        unimplemented!()
    }
    async fn search(&self, _: &str) -> ProviderResult<SearchResults> {
        unimplemented!()
    }
    async fn image_bytes(&self, _: ImageRequest) -> ProviderResult<ImageBytes> {
        unimplemented!()
    }
    async fn set_favorite(&self, _: FavoriteItemId, _: bool) -> ProviderResult<()> {
        unimplemented!()
    }
    async fn lyrics(&self, _: &TrackId, _: bool) -> ProviderResult<Option<Lyrics>> {
        unimplemented!()
    }
    async fn report_playback(&self, _: PlaybackReport) -> ProviderResult<()> {
        unimplemented!()
    }
}

fn server_id() -> ServerId {
    ServerId::new("server-stub")
}

#[tokio::test]
async fn sync_library_data_writes_artists_before_albums() {
    // Regression: a previous version of this helper wrote albums
    // first, which raised `FOREIGN KEY constraint failed` because
    // Subsonic-shaped data always populates `album.artist_id`. The
    // assert below is the exact failure mode we are guarding.
    let store = Store::open_memory().expect("open_memory");
    let server = server_id();
    let provider = StubProvider::new(server.clone());

    sinfonic_lib::sync_library_data(&provider, &store, &server)
        .await
        .expect("sync_library_data must succeed when artist rows land before albums");

    let (albums, artists, tracks, _) = store.server_counts(&server).expect("counts");
    assert_eq!(albums, 1, "album row must be persisted");
    assert_eq!(artists, 1, "artist row must be persisted");
    assert_eq!(tracks, 1, "track row must be persisted");
}

#[tokio::test]
async fn sync_library_data_succeeds_against_empty_provider() {
    // No rows at all — a small Jellyfin library or a server that
    // just came online can both produce this shape. The FK ordering
    // has to keep working when the first page is empty.
    struct EmptyProvider {
        identity: ProviderIdentity,
    }
    #[async_trait]
    impl MusicProvider for EmptyProvider {
        fn identity(&self) -> &ProviderIdentity {
            &self.identity
        }
        fn capabilities(&self) -> &ProviderCapabilities {
            unimplemented!()
        }
        async fn albums(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn album_detail(&self, _: &AlbumId) -> ProviderResult<AlbumDetailResponse> {
            unimplemented!()
        }
        async fn tracks(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn artists(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn artist_detail(&self, _: &ArtistId) -> ProviderResult<ArtistDetailResponse> {
            unimplemented!()
        }
        async fn playlists(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Playlist>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn playlist_detail(&self, _: &PlaylistId) -> ProviderResult<PlaylistDetail> {
            unimplemented!()
        }
        async fn stream(&self, _: &TrackId) -> ProviderResult<StreamDescriptor> {
            unimplemented!()
        }
        async fn search(&self, _: &str) -> ProviderResult<SearchResults> {
            unimplemented!()
        }
        async fn image_bytes(&self, _: ImageRequest) -> ProviderResult<ImageBytes> {
            unimplemented!()
        }
        async fn set_favorite(&self, _: FavoriteItemId, _: bool) -> ProviderResult<()> {
            unimplemented!()
        }
        async fn lyrics(&self, _: &TrackId, _: bool) -> ProviderResult<Option<Lyrics>> {
            unimplemented!()
        }
        async fn report_playback(&self, _: PlaybackReport) -> ProviderResult<()> {
            unimplemented!()
        }
    }

    let server = server_id();
    let provider = EmptyProvider {
        identity: ProviderIdentity {
            provider_id: "empty".into(),
            server_id: server.clone(),
            server_name: "empty".into(),
            user_id: "u".into(),
            username: "u".into(),
        },
    };
    let store = Store::open_memory().expect("open_memory");

    sinfonic_lib::sync_library_data(&provider, &store, &server)
        .await
        .expect("empty sync must succeed");

    let (albums, artists, tracks, _) = store.server_counts(&server).expect("counts");
    assert_eq!((albums, artists, tracks), (0, 0, 0));
}

/// Regression for the chunked-sync fix: `sync_library_data` used to
/// issue exactly one `PagedRequest::new(0, 200)` per entity. On a
/// library with >200 artists or >200 albums, anything past page 1
/// was silently dropped — and `replace_albums` then raised
/// `FOREIGN KEY constraint failed` because albums past #200
/// referenced artists past #200 (sorted by name) that the previous
/// page had not inserted.
///
/// This test stubs a provider with 357 albums and 357 artists (the
/// same shape the user's NAS library had), and asserts that ALL of
/// them land in the cache. If `sync_library_data` regresses to a
/// single 200-item fetch, the post-condition catches it.
#[tokio::test]
async fn sync_library_data_fetches_every_page_not_just_the_first() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Stub provider with 357 artists and 357 albums. Each album's
    /// `artist_id` points to a corresponding artist row so the FK
    /// holds once both pages have been written.
    struct BigProvider {
        identity: ProviderIdentity,
        artists: Vec<Artist>,
        albums: Vec<Album>,
        tracks: Vec<Track>,
        artist_calls: Arc<AtomicUsize>,
        album_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl MusicProvider for BigProvider {
        fn identity(&self) -> &ProviderIdentity {
            &self.identity
        }
        fn capabilities(&self) -> &ProviderCapabilities {
            unimplemented!()
        }
        async fn albums(&self, req: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
            self.album_calls.fetch_add(1, Ordering::SeqCst);
            let total = self.albums.len();
            let start = req.offset.min(total);
            let end = (start + req.limit).min(total);
            Ok(PagedResponse::new(self.albums[start..end].to_vec(), total))
        }
        async fn album_detail(&self, _: &AlbumId) -> ProviderResult<AlbumDetailResponse> {
            unimplemented!()
        }
        async fn tracks(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
            Ok(PagedResponse::new(self.tracks.clone(), self.tracks.len()))
        }
        async fn artists(&self, req: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
            self.artist_calls.fetch_add(1, Ordering::SeqCst);
            let total = self.artists.len();
            let start = req.offset.min(total);
            let end = (start + req.limit).min(total);
            Ok(PagedResponse::new(self.artists[start..end].to_vec(), total))
        }
        async fn artist_detail(&self, _: &ArtistId) -> ProviderResult<ArtistDetailResponse> {
            unimplemented!()
        }
        async fn playlists(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Playlist>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn playlist_detail(&self, _: &PlaylistId) -> ProviderResult<PlaylistDetail> {
            unimplemented!()
        }
        async fn stream(&self, _: &TrackId) -> ProviderResult<StreamDescriptor> {
            unimplemented!()
        }
        async fn search(&self, _: &str) -> ProviderResult<SearchResults> {
            unimplemented!()
        }
        async fn image_bytes(&self, _: ImageRequest) -> ProviderResult<ImageBytes> {
            unimplemented!()
        }
        async fn set_favorite(&self, _: FavoriteItemId, _: bool) -> ProviderResult<()> {
            unimplemented!()
        }
        async fn lyrics(&self, _: &TrackId, _: bool) -> ProviderResult<Option<Lyrics>> {
            unimplemented!()
        }
        async fn report_playback(&self, _: PlaybackReport) -> ProviderResult<()> {
            unimplemented!()
        }
    }

    const TOTAL: usize = 250;
    let mut artists: Vec<Artist> = Vec::with_capacity(TOTAL);
    let mut albums: Vec<Album> = Vec::with_capacity(TOTAL);
    for i in 0..TOTAL {
        let pad = format!("{:04}", i);
        let artist_id = ArtistId::new(format!("artist-{pad}"));
        let album_id = AlbumId::new(format!("album-{pad}"));
        artists.push(Artist {
            id: artist_id.clone(),
            name: format!("Artist {pad}"),
            album_count: 1,
            track_count: 1,
            favorite: false,
            image_ref: None,
        });
        albums.push(Album {
            id: album_id.clone(),
            title: format!("Album {pad}"),
            artist: format!("Artist {pad}"),
            artist_id: Some(artist_id),
            year: None,
            track_count: 1,
            duration_seconds: 1,
            favorite: false,
            image_ref: None,
            genres: vec![],
        });
    }

    let server = ServerId::new("server-big");
    let provider = BigProvider {
        identity: ProviderIdentity {
            provider_id: "stub".into(),
            server_id: server.clone(),
            server_name: "stub".into(),
            user_id: "u".into(),
            username: "u".into(),
        },
        artists,
        albums,
        tracks: Vec::new(),
        artist_calls: Arc::new(AtomicUsize::new(0)),
        album_calls: Arc::new(AtomicUsize::new(0)),
    };

    let store = Store::open_memory().expect("open_memory");

    sinfonic_lib::sync_library_data(&provider, &store, &server)
        .await
        .expect("sync_library_data must drain every page; the FK from \
                 albums to artists only holds once the trailing artist \
                 pages have been written");

    let (album_count, artist_count, _track_count, _) =
        store.server_counts(&server).expect("counts");
    assert_eq!(artist_count, TOTAL as i64, "every artist page must be written");
    assert_eq!(album_count, TOTAL as i64, "every album page must be written");

    // Sanity check on the loop itself: 250 / 200 = 2 full pages.
    assert!(
        provider.artist_calls.load(Ordering::SeqCst) >= 2,
        "fetch_all_pages must loop at least twice for 250 artists"
    );
    assert!(
        provider.album_calls.load(Ordering::SeqCst) >= 2,
        "fetch_all_pages must loop at least twice for 250 albums"
    );
}

/// Regression for the playlist sync step: `sync_library_data` used to
/// only pull artists / albums / tracks, leaving the `playlists` table
/// empty even when the provider exposes user playlists (Subsonic /
/// Jellyfin). After the fix, every playlist returned by
/// `provider.playlists()` is fetched in detail and persisted via
/// `library.replace_playlist`.
#[tokio::test]
async fn sync_library_data_persists_playlists() {
    struct PlaylistProvider {
        identity: ProviderIdentity,
        playlists: Vec<Playlist>,
        playlist_tracks: Vec<(PlaylistId, Vec<TrackId>)>,
    }

    #[async_trait]
    impl MusicProvider for PlaylistProvider {
        fn identity(&self) -> &ProviderIdentity {
            &self.identity
        }
        fn capabilities(&self) -> &ProviderCapabilities {
            unimplemented!()
        }
        async fn albums(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn album_detail(
            &self,
            _: &AlbumId,
        ) -> ProviderResult<AlbumDetailResponse> {
            unimplemented!()
        }
        async fn tracks(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn artists(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
            Ok(PagedResponse::new(vec![], 0))
        }
        async fn artist_detail(
            &self,
            _: &ArtistId,
        ) -> ProviderResult<ArtistDetailResponse> {
            unimplemented!()
        }
        async fn playlists(
            &self,
            _: PagedRequest,
        ) -> ProviderResult<PagedResponse<Playlist>> {
            Ok(PagedResponse::new(self.playlists.clone(), self.playlists.len()))
        }
        async fn playlist_detail(
            &self,
            id: &PlaylistId,
        ) -> ProviderResult<PlaylistDetail> {
            let playlist = self
                .playlists
                .iter()
                .find(|p| p.id == *id)
                .cloned()
                .ok_or(ProviderError::NotFound)?;
            let tracks: Vec<Track> = self
                .playlist_tracks
                .iter()
                .find(|(pid, _)| pid == id)
                .map(|(_, tids)| {
                    tids.iter()
                        .map(|tid| Track {
                            id: tid.clone(),
                            album_id: AlbumId::new("album-stub"),
                            title: "Stub".into(),
                            artist: "Stub".into(),
                            artist_id: None,
                            album: "Stub".into(),
                            duration_seconds: 1,
                            track_number: 1,
                            disc_number: 1,
                            favorite: false,
                            image_ref: None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(PlaylistDetail { playlist, tracks })
        }
        async fn stream(&self, _: &TrackId) -> ProviderResult<StreamDescriptor> {
            unimplemented!()
        }
        async fn search(&self, _: &str) -> ProviderResult<SearchResults> {
            unimplemented!()
        }
        async fn image_bytes(&self, _: ImageRequest) -> ProviderResult<ImageBytes> {
            unimplemented!()
        }
        async fn set_favorite(&self, _: FavoriteItemId, _: bool) -> ProviderResult<()> {
            unimplemented!()
        }
        async fn lyrics(
            &self,
            _: &TrackId,
            _: bool,
        ) -> ProviderResult<Option<Lyrics>> {
            unimplemented!()
        }
        async fn report_playback(&self, _: PlaybackReport) -> ProviderResult<()> {
            unimplemented!()
        }
    }

    let p1 = PlaylistId::new("playlist-mix");
    let p2 = PlaylistId::new("playlist-chill");
    let t1 = TrackId::new("track-1");
    let t2 = TrackId::new("track-2");
    let t3 = TrackId::new("track-3");
    let playlists = vec![
        Playlist {
            id: p1.clone(),
            name: "Mix".into(),
            track_count: 2,
            duration_seconds: 600,
            owner: Some("u".into()),
            public: false,
            image_ref: None,
        },
        Playlist {
            id: p2.clone(),
            name: "Chill".into(),
            track_count: 1,
            duration_seconds: 300,
            owner: Some("u".into()),
            public: false,
            image_ref: None,
        },
    ];
    let playlist_tracks = vec![
        (p1.clone(), vec![t1.clone(), t2.clone()]),
        (p2.clone(), vec![t3.clone()]),
    ];

    let server = ServerId::new("server-pl");
    let provider = PlaylistProvider {
        identity: ProviderIdentity {
            provider_id: "stub".into(),
            server_id: server.clone(),
            server_name: "stub".into(),
            user_id: "u".into(),
            username: "u".into(),
        },
        playlists,
        playlist_tracks,
    };

    let store = Store::open_memory().expect("open_memory");

    sinfonic_lib::sync_library_data(&provider, &store, &server)
        .await
        .expect("sync_library_data must persist playlists");

    let stored = store.list_playlists(&server).expect("list_playlists");
    assert_eq!(stored.len(), 2, "both playlists must be persisted");

    let mix = stored.iter().find(|p| p.id == p1).expect("mix");
    assert_eq!(mix.name, "Mix");
    assert_eq!(mix.track_count, 2);

    let mix_tracks = store
        .list_playlist_tracks(&server, &p1)
        .expect("list_playlist_tracks");
    assert_eq!(mix_tracks, vec![t1, t2]);

    let chill_tracks = store
        .list_playlist_tracks(&server, &p2)
        .expect("list_playlist_tracks");
    assert_eq!(chill_tracks, vec![t3]);
}

// Silence unused-import warnings for items used only inside
// `#[async_trait]` blocks (the macro re-orders them).