//! End-to-end scanner + `LocalProvider` tests driven by `hound`-
//! generated fixture WAVs. We don't embed any tag chunks — that
//! would require a separate metadata writer — but the scanner's
//! extension filter, directory walking, dotdir skip, error
//! collection, and track/album aggregation all exercise the same
//! code paths as a real library scan.

use std::fs;
use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};
use sinfonic_domain::PagedRequest;
use sinfonic_source::MusicProvider;
use sinfonic_source_local::{scan, LocalProvider};

fn write_wav(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir -p");
    }
    let spec = WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).expect("create wav");
    // 0.1 s of silence — gives the scanner a non-zero duration to read.
    let n = (spec.sample_rate / 10) as usize;
    for _ in 0..n {
        writer.write_sample(0i16).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir -p");
    }
    fs::write(path, b"not audio").expect("touch");
}

#[test]
fn scan_finds_wav_files_and_aggregates() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();

    // Album with 2 tracks, single artist ("Unknown artist" — no tags).
    write_wav(&root_path.join("album/01 first.wav"));
    write_wav(&root_path.join("album/02 second.wav"));
    // Separate album, same artist (still unknown), so we get 2 albums.
    write_wav(&root_path.join("album2/track.wav"));

    // Non-audio files must be skipped.
    touch(&root_path.join("album/cover.jpg"));
    touch(&root_path.join("notes.txt"));
    touch(&root_path.join("readme.md"));

    // Dotfiles and dotdirs must be skipped.
    touch(&root_path.join(".DS_Store"));
    write_wav(&root_path.join(".hidden/song.wav"));

    let result = scan(root_path).expect("scan");

    assert_eq!(result.tracks.len(), 3, "tracks");
    assert_eq!(result.albums.len(), 2, "albums");
    assert_eq!(result.artists.len(), 1, "artists");
    assert_eq!(result.errors.len(), 0, "errors");
}

#[test]
fn scan_collects_errors_without_aborting() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();

    // One good file.
    write_wav(&root_path.join("good.wav"));
    // One file with the right extension but garbage contents — lofty
    // should reject it and the scanner should continue.
    fs::write(root_path.join("bad.wav"), b"this is not a wav").unwrap();

    let result = scan(root_path).expect("scan");

    assert_eq!(result.tracks.len(), 1);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].path.ends_with("bad.wav"));
}

#[test]
fn scan_uses_filename_as_title_when_no_tags() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("My Song.wav");
    write_wav(&path);

    let result = scan(root.path()).unwrap();
    let track = &result.tracks[0];
    assert_eq!(track.title, "My Song");
    // No tag → "Unknown artist" / "Unknown album".
    assert_eq!(track.artist, "Unknown artist");
    assert_eq!(track.album, "Unknown album");
    // Duration is a property of the audio stream; the WAV we wrote has
    // ~0.1 s of samples so the scanner may read 0 (as_secs truncates).
    // The point of this test is title/artist/album from filename, not duration.
}

#[test]
fn provider_albums_and_tracks_page_correctly() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();
    for i in 0..5 {
        write_wav(&root_path.join(format!("track{i:02}.wav")));
    }

    let provider = LocalProvider::new(root_path);
    let stats = provider.rescan().expect("rescan");
    assert_eq!(stats.tracks, 5);
    // All 5 tracks collapse into one Unknown album.
    assert_eq!(stats.albums, 1);
    assert_eq!(stats.artists, 1);

    let albums = futures::executor::block_on(
        provider.albums(PagedRequest::new(0, 10)),
    )
    .unwrap();
    assert_eq!(albums.items.len(), 1);

    let tracks = futures::executor::block_on(
        provider.tracks(PagedRequest::new(0, 3)),
    )
    .unwrap();
    assert_eq!(tracks.items.len(), 3);
    assert_eq!(tracks.total, 5);

    let tracks = futures::executor::block_on(
        provider.tracks(PagedRequest::new(3, 5)),
    )
    .unwrap();
    assert_eq!(tracks.items.len(), 2);
}

#[test]
fn provider_artists_have_correct_album_count() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();
    // 2 albums × 2 tracks each. All by Unknown artist. So the artist
    // row should report album_count=2, track_count=4.
    for album_idx in 0..2 {
        let album_dir = root_path.join(format!("album{album_idx}"));
        fs::create_dir(&album_dir).unwrap();
        for track_idx in 0..2 {
            write_wav(&album_dir.join(format!("track{track_idx}.wav")));
        }
    }

    let provider = LocalProvider::new(root_path);
    provider.rescan().unwrap();
    let artists = futures::executor::block_on(
        provider.artists(PagedRequest::new(0, 10)),
    )
    .unwrap();
    assert_eq!(artists.items.len(), 1);
    assert_eq!(artists.items[0].album_count, 2);
    assert_eq!(artists.items[0].track_count, 4);
}

#[test]
fn provider_stream_returns_file_uri() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();
    let wav = root_path.join("song.wav");
    write_wav(&wav);

    let provider = LocalProvider::new(root_path);
    provider.rescan().unwrap();
    let track = &provider.snapshot().unwrap().tracks[0];
    let descriptor = futures::executor::block_on(provider.stream(&track.id)).unwrap();
    let uri = descriptor.uri();
    assert!(uri.starts_with("file://"), "uri: {uri}");
    // The redacted URI must be the absolute path (no API tokens for
    // local — there's nothing to redact).
    assert!(
        descriptor.redacted_uri().ends_with("song.wav"),
        "redacted_uri: {}",
        descriptor.redacted_uri()
    );
}

#[test]
fn provider_image_bytes_returns_not_found_when_no_art() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();
    write_wav(&root_path.join("song.wav"));

    let provider = LocalProvider::new(root_path);
    provider.rescan().unwrap();
    let album_id = &provider.snapshot().unwrap().albums[0].id;
    let request = sinfonic_source::ImageRequest {
        item_id: album_id.as_str().to_string(),
        kind: sinfonic_domain::ImageKind::Primary,
        tag: None,
        size: 600,
    };
    let result = futures::executor::block_on(provider.image_bytes(request));
    assert!(matches!(result, Err(sinfonic_source::ProviderError::NotFound)));
}

#[test]
fn provider_search_finds_substring() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path();
    // Two files with distinct names; no tags so the search hits the
    // title (the filename).
    write_wav(&root_path.join("alpha.wav"));
    write_wav(&root_path.join("beta.wav"));

    let provider = LocalProvider::new(root_path);
    provider.rescan().unwrap();
    let result = futures::executor::block_on(provider.search("alp")).unwrap();
    assert_eq!(result.tracks.len(), 1);
    assert!(result.tracks[0].title.contains("alpha"));
}
