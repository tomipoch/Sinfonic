//! Integration tests for `LastFmClient` against a wiremock-backed
//! audioscrobbler endpoint. These tests cover the auth + scrobble
//! happy paths and the error mapping for the most common error
//! codes.

use serde_json::json;
use sinfonic_lastfm::{LastFmClient, LastFmCredentials, Scrobble, ScrobbleSource};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn credentials() -> LastFmCredentials {
    LastFmCredentials {
        api_key: "TESTKEY".into(),
        api_secret: "TESTSECRET".into(),
        username: "alice".into(),
        password_md5: "0".repeat(32),
    }
}

#[tokio::test]
async fn authenticate_happy_path_stores_session_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/"))
        .and(body_string_contains("method=auth.getMobileSession"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "session": { "name": "alice", "key": "SESSION-KEY-123", "subscriber": 0 }
        })))
        .mount(&server)
        .await;

    let endpoint = format!("{}/2.0/", server.uri());
    let mut client = LastFmClient::with_endpoint(&endpoint, "TESTKEY".into(), "TESTSECRET".into())
        .expect("client");
    let session = client.authenticate(&credentials()).await.expect("auth");

    assert_eq!(session, "SESSION-KEY-123");
    assert!(client.is_authenticated());
}

#[tokio::test]
async fn auth_failure_returns_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": 14,
            "message": "Authentication failed"
        })))
        .mount(&server)
        .await;

    let endpoint = format!("{}/2.0/", server.uri());
    let mut client = LastFmClient::with_endpoint(&endpoint, "TESTKEY".into(), "TESTSECRET".into())
        .expect("client");
    let err = client
        .authenticate(&credentials())
        .await
        .expect_err("auth must fail");
    assert!(
        matches!(err, sinfonic_lastfm::LastFmError::Auth(_)),
        "expected Auth error, got {err:?}"
    );
    assert!(!client.is_authenticated());
}

#[tokio::test]
async fn scrobble_returns_accepted_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/"))
        .and(body_string_contains("method=track.scrobble"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "scrobbles": {
                "@attr": { "accepted": 1, "ignored": 0 },
                "scrobble": {
                    "track": { "corrected": "0", "#text": "T" },
                    "artist": { "corrected": "0", "#text": "A" }
                }
            }
        })))
        .mount(&server)
        .await;

    let endpoint = format!("{}/2.0/", server.uri());
    let client = LastFmClient::with_session_for_tests(
        &endpoint,
        "TESTKEY".into(),
        "TESTSECRET".into(),
        "sk".into(),
    )
    .expect("client");

    let scrobble = Scrobble {
        artist: "A".into(),
        track: "T".into(),
        album: Some("Al".into()),
        duration_seconds: Some(180),
        timestamp_unix: 1_700_000_000,
        mbid: None,
    };
    let accepted = client
        .scrobble(&scrobble, ScrobbleSource::User)
        .await
        .expect("scrobble");
    assert!(accepted);
}

#[tokio::test]
async fn now_playing_requires_session_key() {
    let client = LastFmClient::new("TESTKEY".into(), "TESTSECRET".into()).expect("client");
    let scrobble = Scrobble {
        artist: "A".into(),
        track: "T".into(),
        album: None,
        duration_seconds: None,
        timestamp_unix: 0,
        mbid: None,
    };
    let err = client
        .now_playing(&scrobble, ScrobbleSource::User)
        .await
        .expect_err("must error without session");
    assert!(matches!(err, sinfonic_lastfm::LastFmError::NotAuthenticated));
}

#[tokio::test]
async fn rate_limit_error_is_mapped() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/"))
        .and(body_string_contains("method=track.updateNowPlaying"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": 29,
            "message": "Rate limit exceeded"
        })))
        .mount(&server)
        .await;

    let endpoint = format!("{}/2.0/", server.uri());
    let client = LastFmClient::with_session_for_tests(
        &endpoint,
        "TESTKEY".into(),
        "TESTSECRET".into(),
        "sk".into(),
    )
    .expect("client");

    let scrobble = Scrobble {
        artist: "A".into(),
        track: "T".into(),
        album: None,
        duration_seconds: None,
        timestamp_unix: 0,
        mbid: None,
    };
    let err = client
        .now_playing(&scrobble, ScrobbleSource::User)
        .await
        .expect_err("rate limited");
    assert!(matches!(err, sinfonic_lastfm::LastFmError::RateLimited));
}
