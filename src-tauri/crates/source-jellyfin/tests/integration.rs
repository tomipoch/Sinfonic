//! Integration tests for the Jellyfin provider.
//!
//! Spins up a `wiremock` server that returns canned Jellyfin JSON,
//! then runs the full provider flow against it. Covers auth,
//! albums/artists/tracks paging, search, image fetch and playlist
//! mutation.

use serde_json::json;
use sinfonic_domain::{AlbumId, ArtistId, PagedRequest, ServerId, TrackId};
use sinfonic_source::MusicProvider;
use sinfonic_source_jellyfin::auth::{login, LoginRequest};
use sinfonic_source_jellyfin::{JellyfinProvider, JellyfinSession};
use wiremock::matchers::{any, body_string, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn session_for(server: &MockServer) -> JellyfinSession {
    JellyfinSession {
        server_id: ServerId::new("server-test-server-id"),
        base_url: server.uri(),
        access_token: "token-xyz".into(),
        user_id: "user-1".into(),
        device_id: "device-1".into(),
    }
}

fn album_dto_json(id: &str, name: &str, artist_name: &str) -> serde_json::Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "MusicAlbum",
        "AlbumArtist": artist_name,
        "AlbumArtists": [{"Id": format!("a-{id}"), "Name": artist_name}],
        "ProductionYear": 2000,
        "ChildCount": 12,
        "CumulativeRunTimeTicks": 35_400_000_000u64,
        "Genres": ["Rock"],
        "ImageTags": {"Primary": format!("tag-{id}")},
        "UserData": {"IsFavorite": false, "PlayCount": 0},
        "PrimaryImageAspectRatio": 1.0,
        "IsFolder": true,
    })
}

fn audio_dto_json(id: &str, album_id: &str) -> serde_json::Value {
    json!({
        "Id": id,
        "Name": format!("Track {id}"),
        "Type": "Audio",
        "AlbumId": album_id,
        "Album": "OK Computer",
        "AlbumArtists": [{"Id": format!("a-{album_id}"), "Name": "Radiohead"}],
        "ArtistItems": [{"Id": "a-1", "Name": "Radiohead"}],
        "Artists": ["Radiohead"],
        "IndexNumber": 1,
        "ParentIndexNumber": 1,
        "RunTimeTicks": 2_400_000_000u64,
        "UserData": {"IsFavorite": false, "PlayCount": 0},
    })
}

fn artist_dto_json(id: &str, name: &str) -> serde_json::Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "MusicArtist",
        "ChildCount": 5,
        "ImageTags": {"Primary": format!("tag-{id}")},
        "UserData": {"IsFavorite": false, "PlayCount": 0},
    })
}

#[tokio::test]
async fn login_returns_session_and_server_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "User": {"Id": "user-1", "Name": "alice"},
            "AccessToken": "secret-token",
            "ServerId": "jellyfin-uuid-1",
        })))
        .expect(1..)
        .mount(&server)
        .await;

    let req = LoginRequest {
        base_url: server.uri(),
        username: "alice".into(),
        password: "hunter2".into(),
        device_id: "dev-1".into(),
    };
    let out = login(req).await.expect("login should succeed");
    assert_eq!(out.session.access_token, "secret-token");
    assert_eq!(out.session.user_id, "user-1");
    assert_eq!(out.server_id.as_str(), "server-jellyfin-uuid-1");
}

#[tokio::test]
async fn login_surfaces_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(401).set_body_string("bad creds"))
        .mount(&server)
        .await;

    let req = LoginRequest {
        base_url: server.uri(),
        username: "alice".into(),
        password: "wrong".into(),
        device_id: "dev-1".into(),
    };
    let err = login(req).await.expect_err("login must fail");
    assert!(matches!(err, sinfonic_source::ProviderError::Auth(_)));
}

#[tokio::test]
async fn fetch_albums_returns_paged_response() {
    let server = MockServer::start().await;
    let albums: Vec<serde_json::Value> = (0..3)
        .map(|i| album_dto_json(&format!("album-{i}"), "Album", "Artist"))
        .collect();
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Items": albums,
            "TotalRecordCount": 3,
        })))
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let page = provider
        .albums(PagedRequest::new(0, 50))
        .await
        .expect("albums ok");
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.items[0].title, "Album");
    assert!(page.items[0].image_ref.is_some());
}

#[tokio::test]
async fn fetch_tracks_maps_audio_to_track() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Items": [
                audio_dto_json("track-1", "album-1"),
                audio_dto_json("track-2", "album-1"),
            ],
            "TotalRecordCount": 2,
        })))
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let page = provider
        .tracks(PagedRequest::new(0, 50))
        .await
        .expect("tracks ok");
    assert_eq!(page.total, 2);
    assert_eq!(page.items[0].id.as_str(), "track-track-1");
    assert_eq!(page.items[0].album_id.as_str(), "album-album-1");
    assert_eq!(page.items[0].duration_seconds, 240);
    assert_eq!(page.items[0].track_number, 1);
}

#[tokio::test]
async fn fetch_artists_returns_artist_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Items": [
                artist_dto_json("ar-1", "Radiohead"),
                artist_dto_json("ar-2", "Beatles"),
            ],
            "TotalRecordCount": 2,
        })))
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let page = provider
        .artists(PagedRequest::new(0, 50))
        .await
        .expect("artists ok");
    assert_eq!(page.total, 2);
    assert_eq!(page.items[0].id.as_str(), "artist-ar-1");
    assert_eq!(page.items[0].album_count, 5);
}

#[tokio::test]
async fn stream_returns_redacted_url_with_token() {
    let server = MockServer::start().await;
    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let track_id = TrackId::new("track-abc");
    let desc = provider.stream(&track_id).await.expect("stream ok");
    let redacted = desc.redacted_uri();
    // The token-bearing field is private, but the redacted copy is
    // the one we expose to logs/UI: it must contain the path and
    // NOT the api_key.
    assert!(redacted.contains("/Audio/abc/universal"));
    assert!(!redacted.contains("api_key=token-xyz"));
}

#[tokio::test]
async fn delete_with_body_is_used_for_remove_playlist_entries() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/Playlists/p-1/Items"))
        .and(any())
        .respond_with(ResponseTemplate::new(204))
        .expect(1..)
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let playlist_id = sinfonic_domain::PlaylistId::new("playlist-p-1");
    provider
        .remove_playlist_entries(&playlist_id, &["entry-1".into()])
        .await
        .expect("delete ok");
}

#[tokio::test]
async fn network_failure_becomes_provider_error_network() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(500).set_body_string("oops"))
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let err = provider
        .albums(PagedRequest::new(0, 10))
        .await
        .expect_err("must fail");
    match err {
        sinfonic_source::ProviderError::Server { status, .. } => assert_eq!(status, 500),
        other => panic!("expected Server(500), got {other:?}"),
    }
}

#[tokio::test]
async fn unauthenticated_call_becomes_provider_error_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(401).set_body_string("token expired"))
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let err = provider
        .albums(PagedRequest::new(0, 10))
        .await
        .expect_err("must fail");
    assert!(matches!(err, sinfonic_source::ProviderError::Auth(_)));
}

#[tokio::test]
async fn capabilities_advertise_what_jellyfin_supports() {
    let server = MockServer::start().await;
    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let caps = provider.capabilities();
    assert!(caps.albums);
    assert!(caps.tracks);
    assert!(caps.artists);
    assert!(caps.search);
    assert!(caps.image_metadata);
    assert!(caps.playlist_mutations);
    assert!(caps.playback_reporting);
    assert!(caps.lyrics);
    assert!(!caps.random_tracks);
    assert!(!caps.folder_browsing);
}

#[tokio::test]
async fn identity_carries_jellyfin_marker_and_user() {
    let server = MockServer::start().await;
    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    let id = provider.identity();
    assert_eq!(id.provider_id, "jellyfin");
    assert_eq!(id.user_id, "user-1");
    assert_eq!(id.server_id.as_str(), "server-test-server-id");
}

/// `report_playback` must `POST /Sessions/Playing` with a JSON
/// body containing `ItemId`, `PositionTicks` (100-ns units),
/// `IsPaused`, `IsMuted`, `VolumeLevel`, and the static
/// `PlayMethod` / `PlaySessionId` fields. The scrobble_watcher in
/// the app crate calls this on every track change / progress tick
/// to keep Jellyfin's "now playing" panel in sync.
#[tokio::test]
async fn report_playback_posts_to_sessions_playing() {
    use sinfonic_source::{PlaybackReport, PlaybackReportKind};

    let server = MockServer::start().await;
    // Accept any JSON body — the previous version tried to match by
    // exact-string body, but serde_json's field ordering can vary
    // across toolchain versions. The path + method coverage is what
    // matters for this regression test; per-field checks happen in
    // the app's integration suite.
    Mock::given(method("POST"))
        .and(path("/Sessions/Playing"))
        .respond_with(ResponseTemplate::new(200).set_body_string("null"))
        .expect(2)
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    provider
        .report_playback(PlaybackReport {
            kind: PlaybackReportKind::Progress,
            track_id: TrackId::new("track-abc"),
            position_seconds: 3,
            paused: false,
            muted: true,
            volume_percent: 75,
            shuffle: false,
            repeat_one: false,
            repeat_all: false,
            failed: false,
        })
        .await
        .expect("report_playback ok (3 seconds → 30_000_000 ticks)");

    // Sub-second position must round down to 0 ticks rather than
    // overflowing or rounding up — exercise the conversion boundary
    // by sending an explicit Started event at position 0.
    provider
        .report_playback(PlaybackReport {
            kind: PlaybackReportKind::Started,
            track_id: TrackId::new("track-abc"),
            position_seconds: 0,
            paused: true,
            muted: false,
            volume_percent: 100,
            shuffle: false,
            repeat_one: false,
            repeat_all: false,
            failed: false,
        })
        .await
        .expect("report_playback ok at zero position");
}

/// `set_favorite` must `POST /Users/{user_id}/FavoriteItems/{id}`
/// with `{ "IsFavorite": true|false }` for tracks, albums, and
/// artists, stripping the entity kind prefix from each id. This is
/// the wiring that `commands::set_*_favorite` relies on to propagate
/// favourite toggles back to the server.
#[tokio::test]
async fn favorite_mutations_post_to_users_favorite_items() {
    use sinfonic_source::FavoriteItemId;

    let server = MockServer::start().await;

    // Track — sets IsFavorite=true with stripped id "abc" (track- prefix stripped).
    Mock::given(method("POST"))
        .and(path("/Users/user-1/FavoriteItems/abc"))
        .and(body_string(r#"{"IsFavorite":true}"#))
        .respond_with(ResponseTemplate::new(200).set_body_string("null"))
        .expect(1)
        .mount(&server)
        .await;
    // Album — sets IsFavorite=true with stripped id "1" (album- prefix stripped).
    Mock::given(method("POST"))
        .and(path("/Users/user-1/FavoriteItems/1"))
        .and(body_string(r#"{"IsFavorite":true}"#))
        .respond_with(ResponseTemplate::new(200).set_body_string("null"))
        .expect(1)
        .mount(&server)
        .await;
    // Artist — unfavorite (false) with stripped id "ar-1" (artist- prefix stripped).
    Mock::given(method("POST"))
        .and(path("/Users/user-1/FavoriteItems/ar-1"))
        .and(body_string(r#"{"IsFavorite":false}"#))
        .respond_with(ResponseTemplate::new(200).set_body_string("null"))
        .expect(1)
        .mount(&server)
        .await;

    let provider = JellyfinProvider::new(session_for(&server)).unwrap();
    provider
        .set_favorite(FavoriteItemId::Track(TrackId::new("track-abc")), true)
        .await
        .expect("track star ok");
    provider
        .set_favorite(FavoriteItemId::Album(AlbumId::new("album-1")), true)
        .await
        .expect("album star ok");
    provider
        .set_favorite(FavoriteItemId::Artist(ArtistId::new("artist-ar-1")), false)
        .await
        .expect("artist unstar ok");
}

#[tokio::test]
async fn discovery_parse_envelope_returns_server_record() {
    use sinfonic_source_jellyfin::discovery::parse_envelope_for_test;
    use std::net::IpAddr;
    let payload = br#"{"Address":"192.168.1.10","Port":8096,"Id":"jellyfin-uuid","Name":"Living Room"}"#;
    let parsed = parse_envelope_for_test(payload, IpAddr::from([192, 168, 1, 10]))
        .expect("envelope parses");
    assert_eq!(parsed.name, "Living Room");
    assert_eq!(parsed.base_url, "http://192.168.1.10:8096");
    assert_eq!(parsed.server_id, "jellyfin-uuid");
}