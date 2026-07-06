//! End-to-end orchestration tests for `commands::lookup_lyrics`.
//!
//! We exercise the **layer 1 (provider) → layer 2 (LRCLIB)** flow
//! without spinning up a Tauri runtime by calling the public
//! `lookup_lyrics` helper that `commands::get_lyrics` delegates to.
//!
//! Each test wires:
//!  * an in-memory `Store` (`sinfonic_library::Store::open_memory`) —
//!    needed so LRCLIB has the `(artist, title, album, duration)`
//!    to query with;
//!  * a stub `MusicProvider` whose `lyrics()` behaviour we control
//!    per test;
//!  * a `wiremock` server fronting an `LrclibClient` so the test
//!    counts how many HTTP requests the orchestrator issued.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, ImageKind, PagedRequest, PagedResponse, ServerId, Track,
    TrackId,
};
use sinfonic_lib::lookup_lyrics;
use sinfonic_library::Store;
use sinfonic_lyrics::LrclibClient;
use sinfonic_source::{
    AlbumDetailResponse, ArtistDetailResponse, FavoriteItemId, HomeSection, ImageBytes,
    ImageMetadata, ImageRequest, Lyrics, MusicProvider, PlaybackReport, ProviderCapabilities,
    ProviderError, ProviderIdentity, ProviderResult, RandomTrackRequest, StreamRequest,
};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TRACK_ID: &str = "track-eagles-001";

/// Stub provider that delegates `lyrics()` to a closure. Other
/// methods are unimplemented; tests only touch the `lyrics`
/// surface.
struct StubProvider<F>
where
    F: Fn(&TrackId, bool) -> ProviderResult<Option<Lyrics>> + Send + Sync,
{
    identity: ProviderIdentity,
    capabilities: ProviderCapabilities,
    lyrics_fn: F,
    calls: Arc<AtomicUsize>,
}

impl<F> StubProvider<F>
where
    F: Fn(&TrackId, bool) -> ProviderResult<Option<Lyrics>> + Send + Sync,
{
    fn new(server_id: ServerId, lyrics_fn: F) -> Self {
        Self {
            identity: ProviderIdentity {
                provider_id: "stub".to_string(),
                server_id,
                server_name: "stub".to_string(),
                user_id: "stub".to_string(),
                username: "stub".to_string(),
            },
            capabilities: ProviderCapabilities::default(),
            lyrics_fn,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl<F> MusicProvider for StubProvider<F>
where
    F: Fn(&TrackId, bool) -> ProviderResult<Option<Lyrics>> + Send + Sync,
{
    fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }
    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }
    async fn albums(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
        unimplemented!()
    }
    async fn album_detail(&self, _: &AlbumId) -> ProviderResult<AlbumDetailResponse> {
        unimplemented!()
    }
    async fn tracks(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
        unimplemented!()
    }
    async fn artists(&self, _: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        unimplemented!()
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
    ) -> ProviderResult<PagedResponse<sinfonic_domain::Playlist>> {
        unimplemented!()
    }
    async fn playlist_detail(
        &self,
        _: &sinfonic_domain::PlaylistId,
    ) -> ProviderResult<sinfonic_domain::PlaylistDetail> {
        unimplemented!()
    }
    async fn stream(&self, _: &TrackId) -> ProviderResult<sinfonic_domain::StreamDescriptor> {
        unimplemented!()
    }
    async fn search(&self, _: &str) -> ProviderResult<sinfonic_domain::SearchResults> {
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
        track_id: &TrackId,
        allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        (self.lyrics_fn)(track_id, allow_remote)
    }
    async fn report_playback(&self, _: PlaybackReport) -> ProviderResult<()> {
        unimplemented!()
    }
}

/// Insert a single track into the in-memory store so the LRCLIB
/// fallback can build its `(artist, title, album, duration)` query.
/// The store's foreign keys require the parent artist and album
/// rows to exist.
fn seed_track(library: &Store, server_id: &ServerId) {
    library
        .upsert_artist(
            server_id,
            &Artist {
                id: ArtistId::new("artist-1"),
                name: "Eagles".into(),
                album_count: 1,
                track_count: 1,
                favorite: false,
                image_ref: None,
            },
        )
        .expect("upsert artist");
    library
        .upsert_album(
            server_id,
            &Album {
                id: AlbumId::new("album-1"),
                title: "Hotel California".into(),
                artist: "Eagles".into(),
                artist_id: Some(ArtistId::new("artist-1")),
                year: None,
                track_count: 1,
                duration_seconds: 390,
                favorite: false,
                image_ref: None,
                genres: vec![],
            },
        )
        .expect("upsert album");
    library
        .upsert_track(
            server_id,
            &Track {
                id: TrackId::new(TRACK_ID),
                album_id: AlbumId::new("album-1"),
                title: "Hotel California".into(),
                artist: "Eagles".into(),
                artist_id: Some(ArtistId::new("artist-1")),
                album: "Hotel California".into(),
                duration_seconds: 390,
                track_number: 1,
                disc_number: 1,
                favorite: false,
                image_ref: None,
            },
        )
        .expect("upsert track");
}

async fn client_for(server: &MockServer) -> Arc<LrclibClient> {
    let url: reqwest::Url = server.uri().parse().expect("valid mock uri");
    Arc::new(LrclibClient::new(url, "test".to_string()).expect("client builds"))
}

#[tokio::test]
async fn provider_returns_then_lrclib_is_skipped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let library = Store::open_memory().expect("memory store");
    let server_id = ServerId::new("server-stub");
    seed_track(&library, &server_id);

    let provider = StubProvider::new(
        server_id.clone(),
        |_, _| {
            Ok(Some(Lyrics {
                plain: Some("On a dark desert highway".into()),
                synced: Some("[00:12.00] On a dark desert highway".into()),
                source: Some("subsonic".into()),
            }))
        },
    );
    let provider_calls = provider.calls.clone();

    let lyrics = lookup_lyrics(
        Some(Arc::new(provider)),
        client_for(&server).await,
        &library,
        server_id,
        &TrackId::new(TRACK_ID),
        true,
    )
    .await
    .expect("ok")
    .expect("some lyrics");
    assert_eq!(lyrics.source.as_deref(), Some("subsonic"));
    assert_eq!(provider_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn provider_returns_none_then_lrclib_runs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .and(query_param("artist_name", "Eagles"))
        .and(query_param("track_name", "Hotel California"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 99,
            "syncedLyrics": "[00:12.00] first\n[00:16.00] second",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let library = Store::open_memory().expect("memory store");
    let server_id = ServerId::new("server-stub");
    seed_track(&library, &server_id);

    let provider = StubProvider::new(server_id.clone(), |_, _| Ok(None));

    let lyrics = lookup_lyrics(
        Some(Arc::new(provider)),
        client_for(&server).await,
        &library,
        server_id,
        &TrackId::new(TRACK_ID),
        true,
    )
    .await
    .expect("ok")
    .expect("some lyrics");
    assert_eq!(lyrics.source.as_deref(), Some("lrclib"));
    assert!(lyrics.synced.unwrap().starts_with("[00:12.00]"));
}

/// A provider may return real lyrics without tagging the source.
/// The orchestrator must stamp the active provider's `provider_id`
/// onto the response so the UI can render the provenance chip
/// even when the provider forgot to. This guards the
/// audit-flagged regression where the lyric panel would show
/// "unknown" for any provider that returned `source: None`.
#[tokio::test]
async fn provider_lyrics_without_source_gets_tagged_with_provider_id() {
    let server = MockServer::start().await;
    // The provider returns Some(...); LRCLIB must NOT be hit.
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let library = Store::open_memory().expect("memory store");
    let server_id = ServerId::new("server-stub");
    seed_track(&library, &server_id);

    let provider = StubProvider::new(
        server_id.clone(),
        |_, _| {
            // Note: source deliberately omitted — the orchestrator
            // is responsible for stamping it.
            Ok(Some(Lyrics {
                plain: Some("On a dark desert highway".into()),
                synced: None,
                source: None,
            }))
        },
    );

    let lyrics = lookup_lyrics(
        Some(Arc::new(provider)),
        client_for(&server).await,
        &library,
        server_id,
        &TrackId::new(TRACK_ID),
        true,
    )
    .await
    .expect("ok")
    .expect("some lyrics");
    // The StubProvider declares provider_id = "stub" in StubProvider::new.
    assert_eq!(lyrics.source.as_deref(), Some("stub"));
}

#[tokio::test]
async fn provider_returns_unsupported_then_lrclib_runs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 7,
            "plainLyrics": "fallback lyrics",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let library = Store::open_memory().expect("memory store");
    let server_id = ServerId::new("server-stub");
    seed_track(&library, &server_id);

    let provider = StubProvider::new(server_id.clone(), |_, _| {
        Err(ProviderError::Unsupported("lyrics (jellyfin)"))
    });

    let lyrics = lookup_lyrics(
        Some(Arc::new(provider)),
        client_for(&server).await,
        &library,
        server_id,
        &TrackId::new(TRACK_ID),
        true,
    )
    .await
    .expect("ok")
    .expect("some lyrics");
    assert_eq!(lyrics.source.as_deref(), Some("lrclib"));
}

#[tokio::test]
async fn allow_remote_false_blocks_lrclib_even_when_provider_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "syncedLyrics": "[00:00.00] nope",
        })))
        .expect(0)
        .mount(&server)
        .await;

    let library = Store::open_memory().expect("memory store");
    let server_id = ServerId::new("server-stub");
    seed_track(&library, &server_id);

    let provider = StubProvider::new(server_id.clone(), |_, _| Ok(None));

    let result = lookup_lyrics(
        Some(Arc::new(provider)),
        client_for(&server).await,
        &library,
        server_id,
        &TrackId::new(TRACK_ID),
        false,
    )
    .await
    .expect("ok");
    assert!(result.is_none());
}

#[tokio::test]
async fn all_layers_return_none_then_lyrics_is_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let library = Store::open_memory().expect("memory store");
    let server_id = ServerId::new("server-stub");
    seed_track(&library, &server_id);

    let provider = StubProvider::new(server_id.clone(), |_, _| Ok(None));

    let result = lookup_lyrics(
        Some(Arc::new(provider)),
        client_for(&server).await,
        &library,
        server_id,
        &TrackId::new(TRACK_ID),
        true,
    )
    .await
    .expect("ok");
    assert!(result.is_none());
}
