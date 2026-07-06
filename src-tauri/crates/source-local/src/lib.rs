//! Local-files `MusicProvider` (Phase 8).
//!
//! Walk a music directory with `walkdir`, parse metadata with
//! `lofty`, and serve the deduplicated snapshot through the
//! `MusicProvider` trait. No remote API, no auth — just the
//! filesystem and the same SQLite cache the other providers use.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use sinfonic_domain::{
    Album, AlbumDetail, AlbumId, Artist, ArtistDetail, ArtistId, FolderDetail, FolderId, Genre,
    GenreDetail, GenreId, ImageKind, MusicFolder, MusicFolderId, PagedRequest, PagedResponse,
    Playlist, PlaylistDetail, PlaylistId, SearchResults, ServerId, StreamDescriptor, Track,
    TrackId,
};
use sinfonic_source::{
    AlbumDetailResponse, ArtistDetailResponse, Capabilities, FavoriteItemId, HomeSection, Identity,
    ImageBytes, ImageMetadata, ImageRequest, Lyrics, MusicProvider, PlaybackReport, ProviderCapabilities,
    ProviderError, ProviderIdentity, ProviderResult, RandomTrackRequest,
};

pub mod scanner;

pub use scanner::{scan, EmbeddedArt, FileError, ScanResult};

/// Bridge the scanner's error type into the provider layer. The
/// music-provider trait returns `ProviderError`, but the scanner's
/// filesystem errors don't map cleanly onto any of its variants —
/// they get surfaced as a generic `Other` so the UI still gets a
/// human-readable message.
impl From<scanner::ScanError> for ProviderError {
    fn from(e: scanner::ScanError) -> Self {
        ProviderError::Other(format!("local scan failed: {e}"))
    }
}

pub const LOCAL_PROVIDER_ID: &str = "local";
pub const LOCAL_SERVER_NAME: &str = "Local files";
pub const LOCAL_SERVER_ID: &str = "server-local";

/// Inner state guarded by a `parking_lot::RwLock` so the trait methods
/// can read the scan snapshot without an async hop. The scan itself
/// is synchronous (filesystem-bound) and runs outside any lock.
#[derive(Default)]
struct LocalState {
    scan: Option<ScanResult>,
}

/// Owns the music root + the in-memory scan result. Cheap to clone
/// (it's just an `Arc<RwLock<…>>`).
pub struct LocalProvider {
    root: PathBuf,
    state: Arc<RwLock<LocalState>>,
    identity: ProviderIdentity,
    /// Wrapped in an Arc so the trait `capabilities()` can return
    /// a reference that outlives any temporary clone of `self`.
    /// Matches the convention used by Jellyfin / Subsonic providers.
    capabilities: Arc<ProviderCapabilities>,
}

fn local_capabilities() -> ProviderCapabilities {
    // Every field listed explicitly so the capability surface is
    // obvious at a glance.
    ProviderCapabilities {
        albums: true,
        tracks: true,
        artists: true,
        playlists: false,
        favorites: false,
        lyrics: true,
        playback_reporting: false,
        playlist_mutations: false,
        playlist_delete: false,
        favorite_mutations: false,
        search: true,
    }
}

impl LocalProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        // Canonicalize so `path_for_track` can strip_prefix reliably
        // on platforms where the parent dir is a symlink (macOS
        // resolves `/tmp` -> `/private/tmp`, Linux is usually fine).
        let root = root.canonicalize().unwrap_or(root);
        let identity = ProviderIdentity {
            provider_id: LOCAL_PROVIDER_ID.to_string(),
            server_id: ServerId::new(LOCAL_SERVER_ID),
            server_name: LOCAL_SERVER_NAME.to_string(),
            user_id: LOCAL_PROVIDER_ID.to_string(),
            username: LOCAL_PROVIDER_ID.to_string(),
        };
        Self {
            root,
            state: Arc::new(RwLock::new(LocalState::default())),
            identity,
            capabilities: Arc::new(local_capabilities()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Rescan the music root synchronously and replace the in-memory
    /// snapshot. Cheap to call repeatedly; the SQLite cache writes
    /// happen in the Tauri command layer.
    pub fn rescan(&self) -> Result<ScanStats, scanner::ScanError> {
        let result = scan(&self.root)?;
        let stats = ScanStats::from(&result);
        *self.state.write() = LocalState { scan: Some(result) };
        Ok(stats)
    }

    /// Trigger a scan if the in-memory snapshot hasn't been populated
    /// yet. The `LocalProvider` is reconstructed without a scan on
    /// every app start (see `try_restore_provider` /
    /// `provider_set_active`), so the first data access after a
    /// restart would otherwise fail with `local provider not scanned`.
    /// This keeps the user-facing fix invisible — the LoadingView's
    /// "caching your library" spinner covers the scan.
    ///
    /// The result is cached in the existing `state.scan` slot, so
    /// subsequent calls are O(1). The scan itself is synchronous
    /// and filesystem-bound; calling it from an async method blocks
    /// the async task but does not pin a worker thread.
    pub fn ensure_scanned(&self) -> Result<(), scanner::ScanError> {
        let needs_scan = self.state.read().scan.is_none();
        if needs_scan {
            self.rescan()?;
        }
        Ok(())
    }

    /// Try to expose the in-memory scan (used by tests and the Tauri
    /// command that does the SQLite write).
    pub fn snapshot(&self) -> Option<ScanResult> {
        self.state.read().scan.clone()
    }

    /// Look up an embedded picture by `album_id` (the key format the
    /// scanner emits). Used by `image_bytes` to satisfy the
    /// `provider_image_bytes` IPC command.
    fn lookup_embedded_art(&self, album_id: &str) -> Option<EmbeddedArt> {
        let state = self.state.read();
        match state.scan.as_ref() {
            None => {
                tracing::debug!(
                    target: "sinfonic::source_local",
                    album_id,
                    "lookup_embedded_art called before rescan"
                );
                None
            }
            Some(s) => {
                let n = s.embedded_art.len();
                let hit = s.embedded_art.get(album_id).cloned();
                tracing::trace!(
                    target: "sinfonic::source_local",
                    album_id,
                    entries = n,
                    hit = hit.is_some(),
                    "embedded art lookup"
                );
                hit
            }
        }
    }

    /// Resolve a `track_id` to its absolute filesystem path. Returns
    /// `None` for unknown tracks or tracks whose stored relative path
    /// escapes the configured music root (a safety net against
    /// malicious or corrupt cache rows).
    fn path_for_track(&self, track_id: &TrackId) -> Option<PathBuf> {
        let relative_relaxed = track_id.as_str().strip_prefix("track-")?;
        let relative = percent_decode(relative_relaxed);
        let candidate = self.root.join(&relative);
        candidate
            .canonicalize()
            .ok()
            .and_then(|abs| abs.strip_prefix(&self.root).ok().map(|p| p.to_path_buf()))
            .map(|_| candidate)
    }
}

#[derive(Debug, Clone)]
pub struct ScanStats {
    pub tracks: usize,
    pub albums: usize,
    pub artists: usize,
    pub errors: usize,
}

impl From<&ScanResult> for ScanStats {
    fn from(result: &ScanResult) -> Self {
        Self {
            tracks: result.tracks.len(),
            albums: result.albums.len(),
            artists: result.artists.len(),
            errors: result.errors.len(),
        }
    }
}

#[async_trait]
impl MusicProvider for LocalProvider {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn albums(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Album>> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let total = scan.albums.len();
        let items = paginate(&scan.albums, request.offset, request.limit);
        Ok(PagedResponse::new(items, total))
    }

    async fn album_detail(
        &self,
        album_id: &AlbumId,
    ) -> ProviderResult<AlbumDetailResponse> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let album = scan
            .albums
            .iter()
            .find(|a| a.id == *album_id)
            .cloned()
            .ok_or(ProviderError::NotFound)?;
        let tracks = scan
            .tracks
            .iter()
            .filter(|t| t.album_id == *album_id)
            .cloned()
            .collect();
        Ok(AlbumDetailResponse {
            detail: AlbumDetail {
                album,
                tracks,
            },
        })
    }

    async fn tracks(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Track>> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let total = scan.tracks.len();
        let items = paginate(&scan.tracks, request.offset, request.limit);
        Ok(PagedResponse::new(items, total))
    }

    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let total = scan.artists.len();
        let items = paginate(&scan.artists, request.offset, request.limit);
        Ok(PagedResponse::new(items, total))
    }

    async fn artist_detail(
        &self,
        artist_id: &ArtistId,
    ) -> ProviderResult<ArtistDetailResponse> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let artist = scan
            .artists
            .iter()
            .find(|a| a.id == *artist_id)
            .cloned()
            .ok_or(ProviderError::NotFound)?;
        let albums = scan
            .albums
            .iter()
            .filter(|al| al.artist_id.as_ref() == Some(artist_id))
            .cloned()
            .collect();
        Ok(ArtistDetailResponse {
            detail: ArtistDetail { artist, albums },
        })
    }

    async fn playlists(
        &self,
        _request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Playlist>> {
        Err(ProviderError::Unsupported("playlists (local)"))
    }

    async fn playlist_detail(
        &self,
        _id: &PlaylistId,
    ) -> ProviderResult<PlaylistDetail> {
        Err(ProviderError::Unsupported("playlist_detail (local)"))
    }

    async fn stream(&self, track_id: &TrackId) -> ProviderResult<StreamDescriptor> {
        let path = self
            .path_for_track(track_id)
            .ok_or_else(|| ProviderError::NotFound)?;
        let uri = format!("file://{}", path.to_string_lossy());
        Ok(StreamDescriptor::with_redacted(uri, path.to_string_lossy()))
    }

    async fn search(&self, query: &str) -> ProviderResult<SearchResults> {
        // Cheap substring scan across the in-memory snapshot. FTS5
        // (which the SQLite cache already populates) is what the UI
        // uses; this fallback covers the case where the provider is
        // asked directly.
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let needle = query.to_lowercase();
        let limit = 20;
        let mut albums = Vec::new();
        let mut artists = Vec::new();
        let mut tracks = Vec::new();
        for album in &scan.albums {
            if album.title.to_lowercase().contains(&needle)
                || album.artist.to_lowercase().contains(&needle)
            {
                albums.push(album.clone());
            }
            if albums.len() >= limit {
                break;
            }
        }
        for artist in &scan.artists {
            if artist.name.to_lowercase().contains(&needle) {
                artists.push(artist.clone());
            }
            if artists.len() >= limit {
                break;
            }
        }
        for track in &scan.tracks {
            if track.title.to_lowercase().contains(&needle)
                || track.artist.to_lowercase().contains(&needle)
                || track.album.to_lowercase().contains(&needle)
            {
                tracks.push(track.clone());
            }
            if tracks.len() >= limit {
                break;
            }
        }
        Ok(SearchResults {
            albums,
            artists,
            tracks,
            playlists: Vec::new(),
        })
    }

    async fn image_bytes(&self, request: ImageRequest) -> ProviderResult<ImageBytes> {
        let art = self
            .lookup_embedded_art(&request.item_id)
            .ok_or(ProviderError::NotFound)?;
        Ok(ImageBytes {
            bytes: art.bytes,
            content_type: Some(art.content_type),
        })
    }

    async fn set_favorite(
        &self,
        _item: FavoriteItemId,
        _favorite: bool,
    ) -> ProviderResult<()> {
        // Favorites live in the SQLite cache; the Tauri command
        // layer writes them. The provider can't observe or mutate
        // that state from here, so we surface it as unsupported and
        // let callers use the cache directly.
        Err(ProviderError::Unsupported("set_favorite (local)"))
    }

    async fn lyrics(
        &self,
        id: &TrackId,
        _allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>> {
        let Some(audio_path) = self.path_for_track(id) else {
            return Ok(None);
        };
        // Candidate 1: `<stem>.lrc` — universal LRC sidecar
        // convention (foobar2000, VLC, the LRCLIB downloader, etc.).
        let stem_lrc = audio_path.with_extension("lrc");
        // Candidate 2: `<audio_filename>.lrc` — covers files like
        // `song.flac` becoming `song.flac.lrc`, used by some
        // MusicBrainz Picard setups.
        let sibling_lrc = audio_path.with_file_name(sibling_lrc_name(&audio_path));
        let Some(content) = read_first_existing(&[&stem_lrc, &sibling_lrc]) else {
            return Ok(None);
        };
        let (plain, synced) = split_lrc_or_plain(&content);
        if plain.is_none() && synced.is_none() {
            return Ok(None);
        }
        Ok(Some(Lyrics {
            plain,
            synced,
            source: Some("local-lrc".to_string()),
        }))
    }

    async fn report_playback(&self, _report: PlaybackReport) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("report_playback (local)"))
    }
}

fn paginate<T: Clone>(items: &[T], offset: usize, limit: usize) -> Vec<T> {
    items.iter().skip(offset).take(limit).cloned().collect()
}

/// Hard cap on a sidecar file we'll read into memory. Defends
/// against a runaway LRC file (some tools dump full discographies
/// into one sidecar by mistake).
const MAX_SIDECAR_BYTES: u64 = 512 * 1024;

/// Read the first path in `candidates` that exists and is smaller
/// than `MAX_SIDECAR_BYTES`. Returns `None` when none of them
/// exists or all are oversized.
fn read_first_existing(candidates: &[&Path]) -> Option<String> {
    for path in candidates {
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        };
        if metadata.len() > MAX_SIDECAR_BYTES {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            return Some(content);
        }
    }
    None
}

/// Build the "`<audio_filename>.lrc`" sibling name. For
/// `/a/b/song.flac` returns `song.flac.lrc`.
fn sibling_lrc_name(audio_path: &Path) -> std::ffi::OsString {
    let file_name = audio_path
        .file_name()
        .map(|n| n.to_owned())
        .unwrap_or_default();
    let mut s = file_name;
    s.push(".lrc");
    s
}

/// Detect whether the file content looks like LRC. We treat it as
/// LRC if **any** non-blank line begins with an `[mm:ss]` (with or
/// without fractional digits) timestamp — that's the canonical
/// LRC syncing marker. Lines that look like `[ar: Artist]` or
/// `[ti: Title]` (metadata tags) have non-digit prefixes so they
/// don't trip the matcher.
///
/// Returns `(plain, synced)`: when both fields are `None` the file
/// was empty.
fn split_lrc_or_plain(content: &str) -> (Option<String>, Option<String>) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    let has_lrc_timestamps = trimmed.lines().any(lrc_line_is_timestamp);
    if has_lrc_timestamps {
        (None, Some(trimmed.to_string()))
    } else {
        (Some(trimmed.to_string()), None)
    }
}

/// Returns `true` when the trimmed line begins with
/// `[<digits>:<digits>(.<digits>)?]`. Other `[…]`-prefixed
/// constructs (like `[ar: Artist]` metadata tags) are ignored
/// because their prefix isn't all digits.
fn lrc_line_is_timestamp(line: &str) -> bool {
    let line = line.trim_start();
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'[') {
        return false;
    }
    let close = match line.find(']') {
        Some(i) => i,
        None => return false,
    };
    let inside = &line[1..close];
    let (min_part, rest) = match inside.split_once(':') {
        Some(p) => p,
        None => return false,
    };
    if min_part.is_empty() || !min_part.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let (sec_part, _) = match rest.split_once('.') {
        Some(p) => p,
        None => (rest, ""),
    };
    !sec_part.is_empty() && sec_part.chars().all(|c| c.is_ascii_digit())
}

/// Mirror of the scanner's percent-encoding so `path_for_track` can
/// recover the original relative path from a `track-` id. Only the
/// subset of escapes the scanner emits is handled — anything else
/// is treated as the literal char.
///
/// Decoding is byte-wise: the output may contain invalid UTF-8 if
/// the input does, but real scanner output is well-formed because
/// the encoder pushes each input char's UTF-8 bytes one at a time.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) =
                (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2]))
            {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// Re-export for the integration tests + tauri command layer.

/// A picture embedded in one of the album's tracks.
pub type EmbeddedPicture = EmbeddedArt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_capabilities_match_phase8_contract() {
        let provider = LocalProvider::new("/tmp");
        assert_eq!(provider.identity().provider_id, "local");
        assert_eq!(provider.identity().server_id.as_str(), "server-local");
        assert_eq!(provider.identity().server_name, "Local files");
        let caps = provider.capabilities();
        assert!(caps.albums);
        assert!(caps.tracks);
        assert!(caps.artists);
        assert!(caps.search);
        assert!(caps.lyrics);
        assert!(!caps.playlists);
        assert!(!caps.favorites);
    }

    #[tokio::test]
    async fn stream_descriptor_uses_file_uri() {
        // The provider needs a real file on disk to canonicalize the
        // path; for the unit test we only check the URI construction.
        let dir = tempfile::tempdir().unwrap();
        let provider = LocalProvider::new(dir.path());
        // Insert a sentinel track by hand so we don't need a full
        // lofty round-trip here.
        let rel = "song.mp3";
        let track_id = TrackId::new(format!("track-{rel}"));
        let abs = dir.path().join(rel);
        std::fs::write(&abs, b"fake").unwrap();
        let descriptor = provider.stream(&track_id).await.unwrap();
        assert!(descriptor.uri().starts_with("file://"));
        assert!(descriptor.uri().ends_with("song.mp3"));
    }

    #[tokio::test]
    async fn stream_rejects_unknown_track() {
        let dir = tempfile::tempdir().unwrap();
        let provider = LocalProvider::new(dir.path());
        let track_id = TrackId::new("track-unknown.mp3");
        let err = provider.stream(&track_id).await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound));
    }

    #[test]
    fn paginate_honours_offset_and_limit() {
        let items: Vec<i32> = (0..10).collect();
        assert_eq!(paginate(&items, 0, 3), vec![0, 1, 2]);
        assert_eq!(paginate(&items, 3, 3), vec![3, 4, 5]);
        assert_eq!(paginate(&items, 8, 100), vec![8, 9]);
        assert!(paginate::<i32>(&[], 0, 100).is_empty());
    }

    #[test]
    fn percent_decode_round_trips_scanner_encoding() {
        let cases = [
            ("plain", "plain"),
            ("album%201/track.mp3", "album 1/track.mp3"),
            ("caf%C3%A9.mp3", "caf\u{00e9}.mp3"),
            ("%E2%9C%93.wav", "\u{2713}.wav"),
        ];
        for (encoded, expected) in cases {
            assert_eq!(percent_decode(encoded), expected, "input: {encoded}");
        }
    }
}
