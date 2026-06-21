//! Integration tests for the Subsonic provider.
//!
//! Spins up a `wiremock` server that returns canned Subsonic JSON,
//! then runs the full provider flow against it. Covers auth,
//! albums/artists/tracks paging, search, image fetch, scrobble,
//! favourites and playlist mutation.

use serde_json::json;
use sinfonic_domain::{PagedRequest, ServerId, TrackId};
use sinfonic_source::MusicProvider;
use sinfonic_source_subsonic::auth::{login, LoginRequest};
use sinfonic_source_subsonic::{SubsonicProvider, SubsonicSession};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn session_for(server: &MockServer) -> SubsonicSession {
    SubsonicSession {
        server_id: ServerId::new("server-subsonic-test"),
        base_url: server.uri(),
        username: "alice".into(),
        password: "hunter2".into(),
    }
}

fn envelope_ok(data: serde_json::Value) -> serde_json::Value {
    json!({ "subsonic-response": merge(json!({ "status": "ok" }), data) })
}

fn merge(a: serde_json::Value, b: serde_json::Value) -> serde_json::Value {
    let mut map = a.as_object().cloned().unwrap_or_default();
    if let Some(b_map) = b.as_object() {
        for (k, v) in b_map {
            map.insert(k.clone(), v.clone());
        }
    }
    serde_json::Value::Object(map)
}

fn envelope_failed(code: u16, message: &str) -> serde_json::Value {
    json!({
        "subsonic-response": {
            "status": "failed",
            "error": { "code": code, "message": message }
        }
    })
}

fn album_dto(id: &str, name: &str, artist: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "title": name,
        "artist": artist,
        "artistId": format!("ar-{id}"),
        "year": 2000,
        "songCount": 12,
        "duration": 3540,
        "coverArt": id,
        "genre": "Rock",
        "genres": [{"name": "Alternative"}],
        "starred": "2024-01-01T00:00:00Z",
        "created": "2023-01-01T00:00:00Z",
    })
}

fn child_dto(id: &str, album_id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "parent": album_id,
        "isDir": false,
        "title": format!("Track {id}"),
        "album": "OK Computer",
        "artist": "Radiohead",
        "track": 1,
        "discNumber": 1,
        "year": 1997,
        "duration": 240,
        "albumId": album_id,
        "artistId": "ar-1",
        "coverArt": album_id,
        "starred": "2024-01-01T00:00:00Z",
        "contentType": "audio/mpeg",
        "suffix": "mp3",
        "size": 4_000_000,
        "type": "music",
    })
}

fn artist_dto(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "albumCount": 5,
        "coverArt": id,
    })
}

#[tokio::test]
async fn login_returns_session_and_server_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/ping"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({
            "serverName": "Navidrome",
            "serverType": "navidrome",
            "version": "0.50.0",
            "openSubsonic": true,
        }))))
        .expect(1..)
        .mount(&server)
        .await;

    let req = LoginRequest {
        base_url: server.uri(),
        username: "alice".into(),
        password: "hunter2".into(),
    };
    let out = login(req).await.expect("login should succeed");
    assert_eq!(out.server_name, "Navidrome");
    assert_eq!(out.server_type, "navidrome");
    assert_eq!(
        out.session.server_id.as_str(),
        out.server_id.as_str()
    );
    assert!(out.server_id.as_str().starts_with("server-subsonic-"));
}

#[tokio::test]
async fn login_surfaces_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/ping"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_failed(10, "wrong password")))
        .mount(&server)
        .await;

    let req = LoginRequest {
        base_url: server.uri(),
        username: "alice".into(),
        password: "wrong".into(),
    };
    let err = login(req).await.expect_err("login must fail");
    assert!(matches!(err, sinfonic_source::ProviderError::Auth(_)));
}

#[tokio::test]
async fn fetch_albums_returns_paged_response() {
    let server = MockServer::start().await;
    let albums: Vec<serde_json::Value> = (0..3)
        .map(|i| album_dto(&format!("al-{i}"), "Album", "Artist"))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/getAlbumList2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({
            "albumList2": { "album": albums }
        }))))
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let page = provider
        .albums(PagedRequest::new(0, 50))
        .await
        .expect("albums ok");
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.items[0].title, "Album");
    assert!(page.items[0].favorite);
    assert_eq!(page.items[0].image_ref.as_ref().unwrap().kind, "coverArt");
}

#[tokio::test]
async fn fetch_artists_uses_get_artists() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getArtists"))
        .and(query_param("size", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({
            "artists": {
                "index": [
                    { "name": "A", "artist": [artist_dto("ar-1", "Radiohead"), artist_dto("ar-2", "Beatles")] }
                ],
                "totalCount": 2
            }
        }))))
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let page = provider
        .artists(PagedRequest::new(0, 50))
        .await
        .expect("artists ok");
    assert_eq!(page.total, 2);
    assert_eq!(page.items[0].name, "Radiohead");
    assert_eq!(page.items[0].id.as_str(), "artist-ar-1");
    assert_eq!(page.items[0].album_count, 5);
}

#[tokio::test]
async fn fetch_tracks_maps_child_to_track() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/search3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({
            "searchResult3": {
                "artist": [],
                "album": [],
                "song": [child_dto("t-1", "al-1"), child_dto("t-2", "al-1")],
            }
        }))))
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let page = provider
        .tracks(PagedRequest::new(0, 50))
        .await
        .expect("tracks ok");
    assert_eq!(page.items[0].id.as_str(), "track-t-1");
    assert_eq!(page.items[0].album_id.as_str(), "album-al-1");
    assert_eq!(page.items[0].duration_seconds, 240);
    assert_eq!(page.items[0].track_number, 1);
    assert!(page.items[0].favorite);
}

#[tokio::test]
async fn stream_url_signs_with_fresh_salt_and_token() {
    let server = MockServer::start().await;
    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let track_id = TrackId::new("track-abc123");
    let desc = provider.stream(&track_id).await.expect("stream ok");
    let uri = desc.uri();
    assert!(uri.contains("/rest/stream?id=abc123"));
    assert!(uri.contains("u=alice"));
    // Token = md5("hunter2" + salt) is hard to predict; we
    // assert the parameter is present and non-empty.
    assert!(uri.contains("&t="));
    assert!(!uri.contains("&s=&t="));
}

#[tokio::test]
async fn scrobble_posts_to_scrobble_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/scrobble"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({}))))
        .expect(1)
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let track_id = TrackId::new("track-abc");
    let report = sinfonic_source::PlaybackReport {
        kind: sinfonic_source::PlaybackReportKind::Started,
        track_id: track_id.clone(),
        position_seconds: 0,
        paused: false,
        muted: false,
        volume_percent: 80,
        shuffle: false,
        repeat_one: false,
        repeat_all: false,
        failed: false,
    };
    provider
        .report_playback(report)
        .await
        .expect("scrobble ok");
}

#[tokio::test]
async fn star_toggles_favorite() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/star"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({}))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/unstar"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({}))))
        .expect(1)
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let track_id = TrackId::new("track-1");
    provider
        .set_favorite(sinfonic_source::FavoriteItemId::Track(track_id.clone()), true)
        .await
        .expect("star ok");
    provider
        .set_favorite(sinfonic_source::FavoriteItemId::Track(track_id), false)
        .await
        .expect("unstar ok");
}

#[tokio::test]
async fn capabilities_advertise_what_subsonic_supports() {
    let server = MockServer::start().await;
    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let caps = provider.capabilities();
    assert!(caps.albums);
    assert!(caps.tracks);
    assert!(caps.artists);
    assert!(caps.search);
    assert!(caps.image_metadata);
    assert!(caps.playlist_mutations);
    assert!(caps.playlist_delete);
    assert!(caps.favorite_mutations);
    assert!(caps.playback_reporting);
    assert!(caps.random_tracks);
    assert!(caps.music_folders);
    assert!(!caps.lyrics);
    assert!(!caps.folder_browsing);
}

#[tokio::test]
async fn identity_carries_subsonic_marker_and_user() {
    let server = MockServer::start().await;
    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let id = provider.identity();
    assert_eq!(id.provider_id, "subsonic");
    assert_eq!(id.user_id, "alice");
    assert_eq!(id.username, "alice");
    assert_eq!(id.server_id.as_str(), "server-subsonic-test");
}

#[tokio::test]
async fn unauthenticated_call_becomes_provider_error_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getAlbumList2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_failed(40, "token expired")))
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let err = provider
        .albums(PagedRequest::new(0, 10))
        .await
        .expect_err("must fail");
    assert!(matches!(err, sinfonic_source::ProviderError::Auth(_)));
}

#[tokio::test]
async fn network_failure_becomes_provider_error_network() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getAlbumList2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("oops"))
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
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
async fn search_combines_artists_albums_and_songs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/search3"))
        .and(query_param("query", "radio"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({
            "searchResult3": {
                "artist": [artist_dto("ar-1", "Radiohead")],
                "album": [album_dto("al-1", "OK Computer", "Radiohead")],
                "song": [child_dto("t-1", "al-1")],
            }
        }))))
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let results = provider.search("radio").await.expect("search ok");
    assert_eq!(results.artists.len(), 1);
    assert_eq!(results.albums.len(), 1);
    assert_eq!(results.tracks.len(), 1);
    assert!(results.playlists.is_empty());
}

#[tokio::test]
async fn create_playlist_posts_with_song_ids() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/createPlaylist"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope_ok(json!({
            "playlist": {
                "id": "pl-1",
                "name": "My Mix",
                "songCount": 2,
                "duration": 480,
                "public": false,
                "owner": "alice",
                "entry": []
            }
        }))))
        .expect(1)
        .mount(&server)
        .await;

    let provider = SubsonicProvider::new(session_for(&server)).unwrap();
    let playlist_id = provider
        .create_playlist(
            "My Mix",
            &[TrackId::new("track-1"), TrackId::new("track-2")],
        )
        .await
        .expect("create ok");
    assert_eq!(playlist_id.as_str(), "playlist-pl-1");
}
