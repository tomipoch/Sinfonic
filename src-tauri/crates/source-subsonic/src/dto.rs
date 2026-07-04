//! Subsonic REST API DTOs.
//!
//! The Subsonic JSON envelope is shaped like:
//!
//! ```json
//! { "subsonic-response": { "status": "ok", … } }
//! ```
//!
//! Every endpoint returns the same envelope; the data field is
//! different per call (`albums`, `artists`, `searchResult3`, etc.).
//! `SubsonicEnvelope<T>` captures the union shape and the
//! `into_result` method unwraps it or translates a `status: failed`
//! into a `ProviderError`.
//!
//! The wire format is liberal (newer servers add fields; some
//! servers omit fields) so every field is `Option<T>` or
//! `Vec::default()`. We do the strict mapping from `Option<…>` to
//! `String` / `u32` at the `mapping` boundary so the rest of the
//! codebase only sees whole values.

use serde::{Deserialize, Serialize};
use sinfonic_source::ProviderError;

/// The outer envelope every Subsonic JSON response shares.
/// `T` is the data payload (e.g. `AlbumList`).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicEnvelope<T> {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: SubsonicBody<T>,
}

/// The contents of the envelope. Either `status: "ok"` with a
/// `payload` field, or `status: "failed"` with an `error` block.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicBody<T> {
    pub status: String,
    #[serde(default)]
    pub error: Option<SubsonicError>,
    #[serde(flatten)]
    pub payload: T,
}

impl<T> SubsonicEnvelope<T> {
    /// Translate a `status: failed` body into a typed
    /// `ProviderError::Auth` (code 10/40/41) / `NotFound` (code 70)
    /// / `Other` (anything else). On `status: ok` returns the inner
    /// payload.
    pub fn into_result(self, path: &str) -> Result<T, ProviderError> {
        if self.subsonic_response.status == "ok" {
            return Ok(self.subsonic_response.payload);
        }
        let err = self
            .subsonic_response
            .error
            .unwrap_or_else(|| SubsonicError {
                code: 0,
                message: "(no error block)".into(),
            });
        match err.code {
            10 | 40 | 41 => Err(ProviderError::Auth(format!(
                "{path}: code {} — {}",
                err.code, err.message
            ))),
            70 => Err(ProviderError::NotFound),
            _ => Err(ProviderError::Other(format!(
                "{path}: code {} — {}",
                err.code, err.message
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicError {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
}

/// `/rest/ping` — server health check. Returns the server name
/// and type.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub server_type: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub open_subsonic: bool,
}

/// `/rest/getMusicFolders`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFoldersPayload {
    #[serde(default)]
    pub music_folders: MusicFoldersList,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFoldersList {
    #[serde(default)]
    pub music_folder: Vec<MusicFolderDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFolderDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// `/rest/getIndexes` (alphabetical index of artists). The `indexes`
/// block groups artists under a letter.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexesPayload {
    #[serde(default)]
    pub indexes: Option<IndexesBody>,
    #[serde(default)]
    pub children: Vec<ChildDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexesBody {
    #[serde(default)]
    pub last_modified: Option<u64>,
    #[serde(default)]
    pub index: Vec<IndexGroup>,
    #[serde(default)]
    pub child: Vec<ChildDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexGroup {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub artist: Vec<ArtistDto>,
}

/// `/rest/getArtists` — flat artist list (OpenSubsonic). We prefer
/// this over `getIndexes` because it carries a stable, paginated
/// `totalCount` (added in OpenSubsonic 1.16.1).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistsPayload {
    #[serde(default)]
    pub artists: Option<ArtistsBody>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistsBody {
    #[serde(default)]
    pub index: Vec<IndexGroup>,
    #[serde(default)]
    pub total_count: usize,
}

/// `/rest/getArtist` — one artist with all their albums.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetailPayload {
    #[serde(default)]
    pub artist: ArtistDto,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub album_count: u32,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub artist_image_url: Option<String>,
    #[serde(default)]
    pub starred: Option<String>,
}

/// `/rest/getAlbumList2` — paginated album view.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListPayload {
    #[serde(default)]
    pub album_list2: Option<AlbumListBody>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListBody {
    #[serde(default)]
    pub album: Vec<AlbumDto>,
}

/// `/rest/getAlbum` — one album with all its tracks.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetailPayload {
    #[serde(default)]
    pub album: AlbumDto,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub artist_id: Option<String>,
    #[serde(default)]
    pub year: Option<u16>,
    #[serde(default)]
    pub song_count: u16,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub genres: Vec<GenreRefDto>,
    #[serde(default)]
    pub starred: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub song: Vec<ChildDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenreRefDto {
    #[serde(default)]
    pub name: String,
}

/// `/rest/getSong` / entries in `album.song` and `searchResult.song`.
/// A "child" of an album or a search result.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub track: Option<u16>,
    #[serde(default)]
    pub disc_number: Option<u16>,
    #[serde(default)]
    pub year: Option<u16>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub bit_rate: Option<u32>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub artist_id: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub starred: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub genres: Vec<GenreRefDto>,
    #[serde(default)]
    pub user_rating: Option<u8>,
}

/// `/rest/search3` — combined search across albums / artists / tracks.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3Payload {
    #[serde(default)]
    pub search_result3: Option<SearchResult3Body>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3Body {
    #[serde(default)]
    pub artist: Vec<ArtistDto>,
    #[serde(default)]
    pub album: Vec<AlbumDto>,
    #[serde(default)]
    pub song: Vec<ChildDto>,
}

/// `/rest/getPlaylists`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistsPayload {
    #[serde(default)]
    pub playlists: PlaylistsBody,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistsBody {
    #[serde(default)]
    pub playlist: Vec<PlaylistDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub r#public: bool,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub changed: Option<String>,
    /// Subsonic returns the playlist's cover art id here. Empty /
    /// missing means "no cover" — the frontend falls back to the
    /// gradient placeholder.
    #[serde(default)]
    pub cover_art: Option<String>,
}

/// `/rest/getPlaylist` — one playlist with all its tracks.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetailPayload {
    #[serde(default)]
    pub playlist: PlaylistDetailDto,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetailDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub r#public: bool,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub entry: Vec<PlaylistEntryDto>,
    /// Subsonic includes the playlist's cover id on `getPlaylist`.
    #[serde(default)]
    pub cover_art: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntryDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub track: Option<u16>,
    #[serde(default)]
    pub disc_number: Option<u16>,
    #[serde(default)]
    pub year: Option<u16>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub artist_id: Option<String>,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub genres: Vec<GenreRefDto>,
}

/// `/rest/getGenres`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenresPayload {
    #[serde(default)]
    pub genres: GenresBody,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenresBody {
    #[serde(default)]
    pub genre: Vec<GenreDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenreDto {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub album_count: u32,
}

/// `/rest/getRandomSongs`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSongsPayload {
    #[serde(default)]
    pub random_songs: RandomSongsBody,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSongsBody {
    #[serde(default)]
    pub song: Vec<ChildDto>,
}

/// `/rest/getStarred2` — favorites (starred items).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Starred2Payload {
    #[serde(default)]
    pub starred2: Starred2Body,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Starred2Body {
    #[serde(default)]
    pub artist: Vec<ArtistDto>,
    #[serde(default)]
    pub album: Vec<AlbumDto>,
    #[serde(default)]
    pub song: Vec<ChildDto>,
}

/// `/rest/getLyrics` — song lyrics. Optional in Subsonic, present
/// in Navidrome with `openSubsonic=true`.
///
/// Wire shape per OpenSubsonic spec:
/// ```json
/// "lyrics": {
///   "value": "Plain text fallback…",
///   "struct": [
///     { "lang": "en", "synced": true,
///       "line": [ { "value": "L1", "start": 1000 }, … ] }
///   ]
/// }
/// ```
///
/// `value` is the plain-text fallback. `struct` is an array of
/// per-language entries; each entry groups its `line` array of
/// timed lines. Servers that only ship unsynced lyrics omit
/// `struct` entirely.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsPayload {
    #[serde(default)]
    pub lyrics: LyricsBody,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsBody {
    /// Plain-text lyrics. Present on most providers, including the
    /// legacy Subsonic flavour that doesn't ship `struct`.
    #[serde(default)]
    pub value: Option<String>,
    /// Synced lyrics, one entry per language variant. Each entry
    /// groups a flat `line` array of timed lines.
    #[serde(default)]
    pub r#struct: Vec<LyricsStructEntryDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsStructEntryDto {
    #[serde(default)]
    pub lang: Option<String>,
    /// `true` when this entry's lines carry timestamps. Servers
    /// that only publish unsynced lyrics omit the flag — we treat
    /// the absence as `false`.
    #[serde(default)]
    pub synced: Option<bool>,
    #[serde(default)]
    pub line: Vec<LyricsLineDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLineDto {
    /// Plain text of this line.
    #[serde(default)]
    pub value: String,
    /// Milliseconds from track start. `None` for unsynced lines.
    #[serde(default)]
    pub start: Option<u64>,
}

/// `/rest/createPlaylist` returns the created playlist wrapped in
/// a `playlist` field. The wrapper is required so `SubsonicBody::payload`
/// can be the wrapping struct (the protocol returns a single
/// `playlist` key, not the playlist fields flattened into the
/// response body).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistResponse {
    #[serde(default)]
    pub playlist: PlaylistDetailDto,
}
