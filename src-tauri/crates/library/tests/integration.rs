//! Integration tests for the `sinfonic-library` crate, exercised
//! against a real on-disk database in a tempdir.

use sinfonic_domain::{Album, AlbumId, ServerId, Track, TrackId};
use sinfonic_library::{LibraryError, Store};
use tempfile::tempdir;

fn sample_album(id: &str, title: &str) -> Album {
    Album {
        id: AlbumId::new(id),
        title: title.into(),
        artist: "Radiohead".into(),
        artist_id: None,
        year: Some(2000),
        track_count: 1,
        duration_seconds: 200,
        favorite: false,
        image_ref: None,
        genres: vec!["Rock".into()],
    }
}

fn sample_track(id: &str, title: &str, album_id: &str) -> Track {
    Track {
        id: TrackId::new(id),
        album_id: AlbumId::new(album_id),
        title: title.into(),
        artist: "Radiohead".into(),
        artist_id: None,
        album: "Album".into(),
        duration_seconds: 200,
        track_number: 1,
        disc_number: 1,
        favorite: false,
        image_ref: None,
    }
}

#[test]
fn end_to_end_album_track_search_workflow() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lib.sqlite");
    let store = Store::open(&path).unwrap();
    let server = ServerId::new("server-1");

    store
        .replace_albums(
            &server,
            &[sample_album("a-1", "OK Computer"), sample_album("a-2", "Kid A")],
        )
        .unwrap();
    store
        .replace_tracks(
            &server,
            &[
                sample_track("t-1", "Karma Police", "a-1"),
                sample_track("t-2", "Idioteque", "a-2"),
            ],
        )
        .unwrap();

    // list_albums reflects the inserts.
    let page = store.list_albums(&server, 0, 10).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.items[0].title, "Kid A"); // alphabetical
    assert_eq!(page.items[1].title, "OK Computer");

    // Search by title fragment returns matching tracks.
    let results = store.search(&server, "Karma", 10).unwrap();
    assert_eq!(results.tracks.len(), 1);
    assert_eq!(results.tracks[0].title, "Karma Police");

    // Search by album name returns the album.
    let results = store.search(&server, "Computer", 10).unwrap();
    assert_eq!(results.albums.len(), 1);
    assert_eq!(results.albums[0].title, "OK Computer");

    // Close and reopen — the on-disk file is durable.
    drop(store);
    let store2 = Store::open(&path).unwrap();
    let page = store2.list_albums(&server, 0, 10).unwrap();
    assert_eq!(page.total, 2);
    let results = store2.search(&server, "Karma", 10).unwrap();
    assert_eq!(results.tracks.len(), 1);
}

#[test]
fn migrations_replay_on_reopen() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lib.sqlite");
    {
        let store = Store::open(&path).unwrap();
        let version: i64 = store
            .connection()
            .unwrap()
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 6);
    }
    {
        let store2 = Store::open(&path).unwrap();
        let version: i64 = store2
            .connection()
            .unwrap()
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        // Still 6 — migrations are not re-applied on reopen.
        assert_eq!(version, 6);
    }
}

#[test]
fn concurrent_queries_via_pool() {
    use std::sync::Arc;
    let dir = tempdir().unwrap();
    let path = dir.path().join("lib.sqlite");
    let store = Arc::new(Store::open(&path).unwrap());
    let server = ServerId::new("server-1");
    store
        .replace_albums(
            &server,
            &(0..50)
                .map(|i| sample_album(&format!("a-{i}"), &format!("Album {i}")))
                .collect::<Vec<_>>(),
        )
        .unwrap();

    let mut handles = Vec::new();
    for _ in 0..4 {
        let store = Arc::clone(&store);
        let server = server.clone();
        handles.push(std::thread::spawn(move || {
            let page = store.list_albums(&server, 0, 100).unwrap();
            assert_eq!(page.total, 50);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn delete_server_clears_fk_cascades() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lib.sqlite");
    let store = Store::open(&path).unwrap();
    let server = ServerId::new("server-1");
    store
        .replace_albums(&server, &[sample_album("a-1", "A")])
        .unwrap();
    store
        .replace_tracks(&server, &[sample_track("t-1", "T", "a-1")])
        .unwrap();
    store.delete_server(&server).unwrap();
    let (a, _, t, _) = store.server_counts(&server).unwrap();
    assert_eq!((a, t), (0, 0));
}

#[test]
fn library_error_displays_io_details() {
    let err = LibraryError::Validation("missing field".into());
    assert_eq!(err.to_string(), "invalid input: missing field");
    let err = LibraryError::NotFound("album-1".into());
    assert_eq!(err.to_string(), "entity not found: album-1");
}
