//! Integration tests for the LRCLIB client.
//!
//! Each test stands up a `wiremock` server, points the client at
//! it, and asserts the de-serialisation / caching / error
//! behaviour. No real network is used.
//!
//! Wire format: see https://lrclib.net/api — `/api/get` returns
//! either a JSON body shaped like `LrclibResponse` or a 404
//! for "not found".

use serde_json::json;
use sinfonic_lyrics::{LrclibClient, LyricsHit, LyricsQuery};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn query_artist() -> LyricsQuery<'static> {
    LyricsQuery {
        artist: "Eagles",
        title: "Hotel California",
        album: Some("Hotel California"),
        duration_seconds: Some(390),
    }
}

async fn client_for(server: &MockServer) -> LrclibClient {
    let url: reqwest::Url = server.uri().parse().expect("valid mock uri");
    LrclibClient::new(url, "test".to_string()).expect("client builds")
}

#[tokio::test]
async fn fetch_returns_synced_and_plain() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .and(query_param("artist_name", "Eagles"))
        .and(query_param("track_name", "Hotel California"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 12345,
            "trackName": "Hotel California",
            "artistName": "Eagles",
            "albumName": "Hotel California",
            "duration": 390.0,
            "instrumental": false,
            "plainLyrics": "On a dark desert highway…",
            "syncedLyrics": "[00:12.00] On a dark desert highway…",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let hit = client.fetch(&query_artist()).await.expect("ok").expect("some");
    assert_eq!(hit.lrclib_id, Some(12345));
    assert_eq!(hit.plain.as_deref(), Some("On a dark desert highway…"));
    assert!(hit.synced.as_deref().unwrap().starts_with("[00:12.00]"));
    assert!(!hit.instrumental);
}

#[tokio::test]
async fn fetch_returns_plain_only() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1,
            "plainLyrics": "unsynced chorus line",
            "syncedLyrics": "",
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let hit = client
        .fetch(&LyricsQuery {
            artist: "X",
            title: "Y",
            album: None,
            duration_seconds: None,
        })
        .await
        .expect("ok")
        .expect("some");
    assert_eq!(hit.plain.as_deref(), Some("unsynced chorus line"));
    assert_eq!(hit.synced, None);
}

#[tokio::test]
async fn fetch_returns_synced_only() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 2,
            "syncedLyrics": "[00:00.50] first\n[00:01.50] second",
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let hit = client
        .fetch(&LyricsQuery {
            artist: "X",
            title: "Y",
            album: None,
            duration_seconds: None,
        })
        .await
        .expect("ok")
        .expect("some");
    assert_eq!(hit.plain, None);
    assert_eq!(
        hit.synced.as_deref(),
        Some("[00:00.50] first\n[00:01.50] second"),
    );
}

#[tokio::test]
async fn fetch_returns_none_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let hit = client
        .fetch(&LyricsQuery {
            artist: "X",
            title: "missing",
            album: None,
            duration_seconds: None,
        })
        .await
        .expect("ok (404 is not a transport error)");
    assert!(hit.is_none());
}

#[tokio::test]
async fn fetch_marks_instrumental() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42,
            "instrumental": true,
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let hit = client
        .fetch(&LyricsQuery {
            artist: "Yann Tiersen",
            title: "Comptine d'un autre été",
            album: None,
            duration_seconds: None,
        })
        .await
        .expect("ok")
        .expect("some");
    assert!(hit.instrumental);
    assert_eq!(hit.plain, None);
    assert_eq!(hit.synced, None);
    assert_eq!(hit.lrclib_id, Some(42));
}

#[tokio::test]
async fn fetch_sends_user_agent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .and(wiremock::matchers::header(
            "user-agent",
            "Sinfonic/test (https://github.com/tomipoch/Sinfonic)",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plainLyrics": "P",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let _ = client
        .fetch(&query_artist())
        .await
        .expect("ok")
        .expect("some");
}

#[tokio::test]
async fn cache_hits_dont_hit_network() {
    let server = MockServer::start().await;
    // `expect(1..=1)` strict — a second request would fail the test.
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1,
            "plainLyrics": "Plain",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let q = LyricsQuery {
        artist: "Eagles",
        title: "Hotel California",
        album: Some("Hotel California"),
        duration_seconds: Some(390),
    };
    let first: LyricsHit = client.fetch(&q).await.expect("ok").expect("some");
    let second = client.fetch(&q).await.expect("ok").expect("some");
    assert_eq!(first, second);
    // Second call must have been served from the LRU.
}

#[tokio::test]
async fn fetch_decodes_url_encoded_values() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/get"))
        .and(query_param("artist_name", "Sigur Rós"))
        .and(query_param("track_name", "Hoppípolla"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1,
            "plainLyrics": "Og ég fann þig",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let q = LyricsQuery {
        artist: "Sigur Rós",
        title: "Hoppípolla",
        album: None,
        duration_seconds: None,
    };
    let hit = client.fetch(&q).await.expect("ok").expect("some");
    assert_eq!(hit.plain.as_deref(), Some("Og ég fann þig"));
}
