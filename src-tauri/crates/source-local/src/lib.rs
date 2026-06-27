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
    ImageBytes, ImageMetadata, ImageRequest, Lyrics, MusicProvider, PlaybackReport, ProviderError,
    ProviderIdentity, ProviderResult, RandomTrackRequest,
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
    capabilities: ProviderCapabilities,
}

#[derive(Clone, Debug)]
pub struct ProviderCapabilities {
    flags: Capabilities,
}

impl ProviderCapabilities {
    fn local() -> Self {
        // Every field listed explicitly so clippy is happy and the
        // capability surface is obvious at a glance.
        let flags = Capabilities {
            albums: true,
            tracks: true,
            artists: true,
            album_artists: true,
            genres: false,
            playlists: false,
            favorites: false,
            lyrics: false,
            playback_reporting: false,
            playlist_mutations: false,
            playlist_delete: false,
            favorite_mutations: false,
            auto_dj: false,
            random_tracks: false,
            random_played_filter: false,
            search: true,
            image_metadata: true,
            music_folders: true,
            folder_browsing: false,
        };
        Self { flags }
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
            capabilities: ProviderCapabilities::local(),
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
                eprintln!(
                    "[local] lookup_embedded_art({album_id}) — in-memory scan is empty (provider not rescanned)"
                );
                None
            }
            Some(s) => {
                let n = s.embedded_art.len();
                let hit = s.embedded_art.get(album_id).cloned();
                eprintln!(
                    "[local] lookup_embedded_art({album_id}) — {n} entries, hit={}",
                    hit.is_some()
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
        &self.capabilities.flags
    }

    async fn home_sections(&self) -> ProviderResult<Vec<HomeSection>> {
        // Cheapest possible "home": the first few albums the scan
        // surfaced, labelled "Explore". The Tauri layer typically
        // calls the SQLite cache directly for home view, so this is
        // a courtesy fallback.
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let albums = snapshot
            .scan
            .as_ref()
            .map(|s| s.albums.iter().take(10).cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        Ok(vec![HomeSection {
            kind: sinfonic_source::HomeSectionKind::Explore,
            albums,
            tracks: Vec::new(),
        }])
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

    async fn track(&self, track_id: &TrackId) -> ProviderResult<Track> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        scan.tracks
            .iter()
            .find(|t| t.id == *track_id)
            .cloned()
            .ok_or(ProviderError::NotFound)
    }

    async fn music_folders(&self) -> ProviderResult<Vec<MusicFolder>> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let track_count = scan.tracks.len() as u32;
        let folder_name = self
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Music")
            .to_string();
        Ok(vec![MusicFolder {
            id: MusicFolderId::new("music-folder-local"),
            name: folder_name,
            track_count,
        }])
    }

    async fn tracks_in_music_folder(
        &self,
        _folder_id: &MusicFolderId,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Track>> {
        // Single-folder provider: same as `tracks`.
        self.tracks(request).await
    }

    async fn folder(
        &self,
        _id: Option<&FolderId>,
        _music_folder_id: Option<&MusicFolderId>,
    ) -> ProviderResult<FolderDetail> {
        Err(ProviderError::Unsupported("folder browsing"))
    }

    async fn artists(&self, request: PagedRequest) -> ProviderResult<PagedResponse<Artist>> {
        self.ensure_scanned()?;
        let snapshot = self.state.read();
        let scan = snapshot.scan.as_ref().expect("ensure_scanned set this");
        let total = scan.artists.len();
        let items = paginate(&scan.artists, request.offset, request.limit);
        Ok(PagedResponse::new(items, total))
    }

    async fn album_artists(
        &self,
        request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Artist>> {
        self.artists(request).await
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

    async fn genres(
        &self,
        _request: PagedRequest,
    ) -> ProviderResult<PagedResponse<Genre>> {
        Err(ProviderError::Unsupported("genres (local)"))
    }

    async fn genre_detail(&self, _id: &GenreId) -> ProviderResult<GenreDetail> {
        Err(ProviderError::Unsupported("genre_detail (local)"))
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

    async fn random_tracks(
        &self,
        _req: RandomTrackRequest,
    ) -> ProviderResult<Vec<Track>> {
        Err(ProviderError::Unsupported("random_tracks (local)"))
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

    async fn image_metadata(
        &self,
        item_id: &str,
        kind: ImageKind,
    ) -> ProviderResult<ImageMetadata> {
        // The local provider doesn't expose image URLs (cover art is
        // embedded inside the audio file, surfaced via `image_bytes`).
        // We still return the same `item_id` so callers can route
        // through the same `provider_image_bytes` IPC.
        Ok(ImageMetadata {
            item_id: item_id.to_string(),
            kind,
            tag: Some("embedded".into()),
            url: String::new(),
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

    async fn create_playlist(
        &self,
        _name: &str,
        _track_ids: &[TrackId],
    ) -> ProviderResult<PlaylistId> {
        Err(ProviderError::Unsupported("create_playlist (local)"))
    }

    async fn rename_playlist(&self, _id: &PlaylistId, _name: &str) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("rename_playlist (local)"))
    }

    async fn delete_playlist(&self, _id: &PlaylistId) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("delete_playlist (local)"))
    }

    async fn add_playlist_tracks(
        &self,
        _id: &PlaylistId,
        _track_ids: &[TrackId],
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("add_playlist_tracks (local)"))
    }

    async fn remove_playlist_entries(
        &self,
        _id: &PlaylistId,
        _entries: &[String],
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported(
            "remove_playlist_entries (local)",
        ))
    }

    async fn move_playlist_entry(
        &self,
        _id: &PlaylistId,
        _entry: &str,
        _new_index: usize,
    ) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("move_playlist_entry (local)"))
    }

    async fn lyrics(
        &self,
        _id: &TrackId,
        _allow_remote: bool,
    ) -> ProviderResult<Option<Lyrics>> {
        Err(ProviderError::Unsupported("lyrics (local)"))
    }

    async fn report_playback(&self, _report: PlaybackReport) -> ProviderResult<()> {
        Err(ProviderError::Unsupported("report_playback (local)"))
    }
}

fn paginate<T: Clone>(items: &[T], offset: usize, limit: usize) -> Vec<T> {
    items.iter().skip(offset).take(limit).cloned().collect()
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
        assert!(caps.image_metadata);
        assert!(caps.music_folders);
        assert!(!caps.playlists);
        assert!(!caps.genres);
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
