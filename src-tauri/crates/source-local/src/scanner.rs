//! Filesystem scanner for the local-files provider.
//!
//! Walks a music directory recursively with `walkdir`, parses each
//! candidate file with `lofty`, and returns a `ScanResult` of
//! deduplicated `Track` / `Album` / `Artist` records ready to be
//! dropped into the SQLite library cache.
//!
//! ## ID derivation
//!
//! - **track id**   = `track-<percent-encoded relative path>` —
//!   stable across rescans because the path itself is the identity.
//! - **album id**   = `album-<sha256(lower(artist) + "\0" + lower(album))>`
//!   — case-insensitive, stable, doesn't depend on track ordering.
//! - **artist id**  = `artist-<sha256(lower(artist))>` — same idea.
//!
//! Case folding uses byte-wise lowercase so "MIX" and "mix" map to
//! the same album. Good enough for a personal library; i18n collation
//! is out of scope.
//!
//! ## Embedded album art
//!
//! `ScanResult::embedded_art` maps `album_id` → the raw bytes of the
//! first picture found on any track of that album. The Tauri layer
//! surfaces this through the existing `provider_image_bytes` command,
//! which already caches by `(provider_id, image_id, tag)` — the
//! `LocalProvider::image_bytes` impl just looks it up here.
//!
//! ## Errors
//!
//! Per-file parse failures are collected in `ScanResult::errors`
//! rather than aborting the whole scan. The library is read-only on
//! disk; a malformed MP3 doesn't affect its neighbours.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use sha2::{Digest, Sha256};
use sinfonic_domain::{
    Album, AlbumId, Artist, ArtistId, ImageRef, Track, TrackId,
};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("scan root does not exist: {0}")]
    NotFound(PathBuf),
    #[error("scan root is not a directory: {0}")]
    NotADirectory(PathBuf),
}

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "mp4", "wav", "aif", "aiff",
];

#[derive(Debug, Default, Clone)]
pub struct ScanResult {
    pub root: PathBuf,
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    /// Per-album embedded art (raw bytes — image content-type detected
    /// by the provider layer from the byte prefix).
    pub embedded_art: HashMap<String, EmbeddedArt>,
    /// Per-file parse failures. Surfaced in the UI as warnings but
    /// never abort the scan.
    pub errors: Vec<FileError>,
}

#[derive(Debug, Clone)]
pub struct EmbeddedArt {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

#[derive(Debug, Clone)]
pub struct FileError {
    pub path: PathBuf,
    pub error: String,
}

/// Walk `root` recursively, parse every audio file we recognise, and
/// return the deduplicated library snapshot.
pub fn scan(root: &Path) -> Result<ScanResult, ScanError> {
    let metadata = fs::metadata(root)?;
    if !metadata.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }

    let mut result = ScanResult {
        root: root.to_path_buf(),
        ..Default::default()
    };

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            // Always descend into the root itself (its file_name may
            // start with a dot — tempfile uses `.tmpXXXX` dirs on
            // macOS — but the user explicitly asked us to scan it).
            if entry.path() == root {
                return true;
            }
            !is_hidden(entry.path())
        });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                result.errors.push(FileError {
                    path: err.path().unwrap_or(root).to_path_buf(),
                    error: err.to_string(),
                });
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !has_audio_extension(path) {
            continue;
        }
        match scan_one(path, root) {
            Ok(TrackOutcome {
                track,
                embedded_art,
            }) => {
                if let Some(art) = embedded_art {
                    let album_id = track.album_id.as_str().to_string();
                    result.embedded_art.entry(album_id).or_insert(art);
                }
                result.tracks.push(track);
            }
            Err(err) => {
                result.errors.push(FileError {
                    path: path.to_path_buf(),
                    error: err.to_string(),
                });
            }
        }
    }

    // Dedupe + sort the three entities for stable output (and stable IDs).
    result.tracks.sort_by_key(|t| t.id.as_str().to_string());
    result.albums = aggregate_albums(&result.tracks, &result.embedded_art);
    result.artists = aggregate_artists(&result.tracks);

    Ok(result)
}

struct TrackOutcome {
    track: Track,
    embedded_art: Option<EmbeddedArt>,
}

/// Derive an album name from the parent directory when no tag
/// provides one. Returns "Unknown album" if the path has no usable
/// file name (e.g. the music root itself).
fn dir_album(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|p| p.parent().or(Some(p)))
        .and_then(|p| p.file_name())
        .and_then(|n| {
            let s = n.to_string_lossy();
            if s.is_empty() { None } else { Some(s.into_owned()) }
        })
        .unwrap_or_else(|| "Unknown album".to_string())
}

fn scan_one(path: &Path, root: &Path) -> Result<TrackOutcome, String> {
    let tagged = Probe::open(path)
        .map_err(|e| format!("probe: {e}"))?
        .guess_file_type()
        .map_err(|e| format!("guess: {e}"))?
        .read()
        .map_err(|e| format!("read: {e}"))?;

    // WAV/FLAC files commonly have no tag block at all; MP3s may
    // have only ID3v1 or only APE. Treat "no tag" as a valid signal
    // — the per-field accessors below all return `None`, which our
    // fallbacks below turn into "Unknown artist"/"Unknown album" +
    // the file stem as the title.
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let artist = tag
        .as_ref()
        .and_then(|t| t.artist())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown artist".to_string());

    let album = tag
        .as_ref()
        .and_then(|t| t.album())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dir_album(path, root));
    let title = tag
        .as_ref()
        .and_then(|t| t.title())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("Untitled")
                .to_string()
        });

    let track_number = tag
        .as_ref()
        .and_then(|t| t.track())
        .map(|n| n.min(u16::MAX as u32) as u16)
        .unwrap_or(0);
    let disc_number = tag
        .as_ref()
        .and_then(|t| t.disk())
        .map(|n| n.max(1).min(u16::MAX as u32) as u16)
        .unwrap_or(1);
    let year = tag
        .as_ref()
        .and_then(|t| t.year())
        .and_then(|y| u16::try_from(y).ok());
    let _year = year;
    let genre = tag
        .as_ref()
        .and_then(|t| t.genre())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let _ = genre; // consumed below if/when we wire genres into Album

    let duration_seconds = tagged
        .properties()
        .duration()
        .as_secs()
        .min(u32::MAX as u64) as u32;

    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let track_id = track_id_for_path(&relative);
    let album_id = album_id_for(&artist, &album);
    let artist_id = artist_id_for(&artist);

    let track = Track {
        id: TrackId::new(track_id),
        album_id: AlbumId::new(album_id),
        title,
        artist: artist.clone(),
        artist_id: Some(ArtistId::new(artist_id)),
        album: album.clone(),
        duration_seconds,
        track_number,
        disc_number,
        favorite: false,
        image_ref: None,
    };

    // Genre is captured here but not surfaced yet — the SQLite cache
    // has no `track_genres` table in Phase 8. Phase 9 will thread it
    // through. Suppress the unused warning.
    let _ = genre;

    let embedded_art = tag
        .as_ref()
        .map(|t| extract_first_picture(t.pictures()))
        .unwrap_or(None)
        .map(|bytes| EmbeddedArt {
            content_type: guess_picture_content_type(&bytes).to_string(),
            bytes,
        });

    Ok(TrackOutcome {
        track,
        embedded_art,
    })
}

fn extract_first_picture(pictures: &[lofty::picture::Picture]) -> Option<Vec<u8>> {
    pictures.first().map(|p| p.data().to_vec())
}

fn guess_picture_content_type(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        "image/jpeg"
    } else if bytes.len() >= 8
        && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    {
        "image/png"
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn track_id_for_path(relative: &str) -> String {
    // Stable across rescans and OS-agnostic (the relative path is
    // already using forward slashes). Percent-encode special chars so
    // the id is a single opaque token — no need for sha256 here, the
    // path is already unique enough within a single library root.
    let mut encoded = String::with_capacity(relative.len());
    encoded.push_str("track-");
    for ch in relative.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            encoded.push(ch);
        } else {
            let mut buf = [0u8; 4];
            let bytes = ch.encode_utf8(&mut buf).as_bytes();
            for byte in bytes {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

fn album_id_for(artist: &str, album: &str) -> String {
    let key = format!("{}\0{}", artist.to_lowercase(), album.to_lowercase());
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let hex = digest_to_hex(&digest);
    format!("album-{hex}")
}

fn artist_id_for(artist: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(artist.to_lowercase().as_bytes());
    let digest = hasher.finalize();
    let hex = digest_to_hex(&digest);
    format!("artist-{hex}")
}

fn digest_to_hex(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out.truncate(16); // 64 bits — plenty for ~10⁹ albums in one library
    out
}

fn aggregate_albums(
    tracks: &[Track],
    embedded_art: &HashMap<String, EmbeddedArt>,
) -> Vec<Album> {
    let mut by_id: HashMap<String, Album> = HashMap::new();
    for track in tracks {
        let id = track.album_id.as_str();
        let entry = by_id.entry(id.to_string()).or_insert_with(|| Album {
            id: track.album_id.clone(),
            title: track.album.clone(),
            artist: track.artist.clone(),
            artist_id: track.artist_id.clone(),
            year: None,
            track_count: 0,
            duration_seconds: 0,
            favorite: false,
            image_ref: embedded_art.get(id).map(|_| ImageRef {
                item_id: id.to_string(),
                kind: image_kind_label(),
                tag: Some("embedded".into()),
            }),
            genres: Vec::new(),
        });
        entry.track_count = entry.track_count.saturating_add(1);
        entry.duration_seconds = entry
            .duration_seconds
            .saturating_add(track.duration_seconds);
        if entry.year.is_none() {
            // Year isn't on Track yet; left as None for now. Phase 9
            // will add `Track::year` and thread it through.
        }
    }
    let mut albums: Vec<Album> = by_id.into_values().collect();
    albums.sort_by_key(|a| a.title.to_lowercase());
    albums
}

fn aggregate_artists(tracks: &[Track]) -> Vec<Artist> {
    let mut by_id: HashMap<String, Artist> = HashMap::new();
    for track in tracks {
        let Some(artist_id) = &track.artist_id else {
            continue;
        };
        let id = artist_id.as_str();
        let entry = by_id.entry(id.to_string()).or_insert_with(|| Artist {
            id: artist_id.clone(),
            name: track.artist.clone(),
            album_count: 0,
            track_count: 0,
            favorite: false,
            image_ref: None,
        });
        entry.track_count = entry.track_count.saturating_add(1);
    }
    // Compute album_count by re-walking tracks (cheap — happens once
    // per scan and the list is small relative to a disk walk).
    let mut albums_per_artist: HashMap<String, std::collections::HashSet<String>> =
        HashMap::new();
    for track in tracks {
        if let Some(artist_id) = &track.artist_id {
            albums_per_artist
                .entry(artist_id.as_str().to_string())
                .or_default()
                .insert(track.album_id.as_str().to_string());
        }
    }
    for artist in by_id.values_mut() {
        artist.album_count = albums_per_artist
            .get(artist.id.as_str())
            .map(|set| set.len() as u32)
            .unwrap_or(0);
    }
    let mut artists: Vec<Artist> = by_id.into_values().collect();
    artists.sort_by_key(|a| a.name.to_lowercase());
    artists
}

fn image_kind_label() -> sinfonic_domain::ImageKindHint {
    // Both Primary and Backdrop cache buckets map to the same
    // 'embedded' label here — the album-art cache keys on the byte
    // prefix regardless of which kind the caller asked for.
    sinfonic_domain::ImageKindHint::Embedded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_audio_extension_matches_case_insensitive() {
        assert!(has_audio_extension(Path::new("song.mp3")));
        assert!(has_audio_extension(Path::new("song.FLAC")));
        assert!(has_audio_extension(Path::new("album/track.m4a")));
        assert!(!has_audio_extension(Path::new("cover.jpg")));
        assert!(!has_audio_extension(Path::new("notes.txt")));
    }

    #[test]
    fn is_hidden_skips_dotfiles_and_dotdirs() {
        assert!(is_hidden(Path::new(".DS_Store")));
        // Walkdir calls filter_entry on each entry, so the parent
        // directory ".cache" is checked; once we skip it we never
        // see the files inside.
        assert!(is_hidden(Path::new("/music/.cache")));
        assert!(!is_hidden(Path::new("song.mp3")));
        assert!(!is_hidden(Path::new("/music/visible.mp3")));
        assert!(!is_hidden(Path::new("/music/.cache/song.mp3")));
    }

    #[test]
    fn track_id_is_stable_across_normalisation() {
        let a = track_id_for_path("album 1/01 - track one.mp3");
        let b = track_id_for_path("album 1/01 - track one.mp3");
        assert_eq!(a, b);
        let c = track_id_for_path("album 1/02 - track two.mp3");
        assert_ne!(a, c);
    }

    #[test]
    fn album_id_collapses_case() {
        assert_eq!(album_id_for("Pink Floyd", "The Wall"), album_id_for("PINK FLOYD", "the wall"));
        assert_ne!(album_id_for("Pink Floyd", "The Wall"), album_id_for("Pink Floyd", "Animals"));
    }

    #[test]
    fn artist_id_collapses_case() {
        assert_eq!(artist_id_for("Opeth"), artist_id_for("opeth"));
        assert_ne!(artist_id_for("Opeth"), artist_id_for("Porcupine Tree"));
    }

    #[test]
    fn extract_first_picture_returns_none_for_no_pictures() {
        let pictures: Vec<lofty::picture::Picture> = Vec::new();
        assert!(extract_first_picture(&pictures).is_none());
    }

    #[test]
    fn guess_picture_content_type_detects_jpeg_png_webp() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let webp = {
            let mut v = b"RIFF".to_vec();
            v.extend_from_slice(&[0; 4]);
            v.extend_from_slice(b"WEBP");
            v
        };
        assert_eq!(guess_picture_content_type(&jpeg), "image/jpeg");
        assert_eq!(guess_picture_content_type(&png), "image/png");
        assert_eq!(guess_picture_content_type(&webp), "image/webp");
        assert_eq!(
            guess_picture_content_type(b"random"),
            "application/octet-stream"
        );
    }
}
