//! Jellyfin REST API DTOs.
//!
//! These mirror the JSON returned by `/Users/AuthenticateByName`,
//! `/Items`, `/Artists`, `/Playlists`, `/System/Info/Public` and
//! `/System/Info`. Fields are `Option<T>` everywhere they aren't
//! guaranteed because Jellyfin's wire format is liberal — newer server
//! versions can return extra fields and we don't want a single missing
//! key to break a session.
//!
//! Every DTO implements `Debug, Deserialize`; `Clone` and `Serialize`
//! are added where the value is round-tripped (auth result, public
//! system info).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResult {
    pub user: AuthUser,
    pub access_token: String,
    pub server_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PublicSystemInfo {
    pub id: String,
    pub server_name: String,
    pub version: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

/// Subset of `/System/Info`. Includes everything from `Public` plus the
/// `OperatingSystem`, `StartupWizardCompleted`, etc. We only consume the
/// fields we currently need.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SystemInfo {
    #[serde(flatten)]
    pub public: PublicSystemInfo,
    #[serde(default)]
    pub operating_system: Option<String>,
}

/// Generic Jellyfin `/Items` envelope used by `/Items`, `/Artists` and
/// search endpoints.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ItemsResponse<T> {
    #[serde(default)]
    pub items: Vec<T>,
    #[serde(default)]
    pub total_record_count: usize,
    #[serde(default)]
    pub start_index: usize,
}

/// One row of `/Items`. The Jellyfin API returns a single DTO for every
/// entity kind and discriminates with `Type` ("MusicAlbum",
/// "MusicArtist", "Audio", "Playlist", "Genre", "MusicGenre").
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseItemDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub server_id: Option<String>,

    // Album-specific.
    #[serde(default)]
    pub album_artist: Option<String>,
    #[serde(default)]
    pub album_artists: Vec<NameIdPair>,
    #[serde(default)]
    pub production_year: Option<u16>,
    #[serde(default)]
    pub premiere_date: Option<String>,
    #[serde(default)]
    pub child_count: Option<u32>,
    #[serde(default)]
    pub cumulative_run_time_ticks: Option<u64>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub is_folder: Option<bool>,

    // Track-specific.
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub artist_items: Vec<NameIdPair>,
    #[serde(default)]
    pub index_number: Option<u16>,
    #[serde(default)]
    pub parent_index_number: Option<u16>,
    #[serde(default)]
    pub run_time_ticks: Option<u64>,

    // Image.
    #[serde(default)]
    pub image_tags: Option<ImageTags>,
    #[serde(default)]
    pub primary_image_aspect_ratio: Option<f64>,

    // Playback / metadata.
    #[serde(default)]
    pub user_data: Option<UserData>,
    #[serde(default)]
    pub can_download: Option<bool>,
    #[serde(default)]
    pub play_access: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NameIdPair {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ImageTags {
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub backdrop: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserData {
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub play_count: u32,
    #[serde(default)]
    pub last_played_date: Option<String>,
}

/// Playlist-specific fields. Jellyfin returns the metadata as a
/// `BaseItemDto`; the convenience wrapper below strips everything we
/// don't need. `ImageTags` carries the cover art id used by the
/// `/Items/{id}/Images/Primary` endpoint.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaylistDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub child_count: Option<u32>,
    #[serde(default)]
    pub cumulative_run_time_ticks: Option<u64>,
    #[serde(default)]
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub open_access: Option<bool>,
    #[serde(default)]
    pub image_tags: Option<super::dto::ImageTags>,
}

/// UDP discovery response payload. The Jellyfin server broadcasts a
/// JSON document every 500ms on port 7359 with the contents of
/// `PublicSystemInfo`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryEnvelope {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}