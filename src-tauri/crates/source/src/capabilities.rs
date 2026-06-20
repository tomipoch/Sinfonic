//! `ProviderCapabilities` declares what a `MusicProvider` can do.
//!
//! Defaults are conservative: providers must opt-in to optional features.
//! This keeps the SOLID Interface Segregation principle honest — the
//! frontend checks capability flags before invoking specialised methods.

use serde::{Deserialize, Serialize};

/// Alias for trait implementations that import the name with `Capabilities`.
pub type Capabilities = ProviderCapabilities;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderCapabilities {
    pub albums: bool,
    pub tracks: bool,
    pub artists: bool,
    pub album_artists: bool,
    pub genres: bool,
    pub playlists: bool,
    pub favorites: bool,
    pub lyrics: bool,
    pub playback_reporting: bool,
    pub playlist_mutations: bool,
    pub playlist_delete: bool,
    pub favorite_mutations: bool,
    pub auto_dj: bool,
    pub random_tracks: bool,
    pub random_played_filter: bool,
    pub search: bool,
    pub image_metadata: bool,
    pub music_folders: bool,
    pub folder_browsing: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            albums: true,
            tracks: true,
            artists: true,
            album_artists: true,
            genres: true,
            playlists: true,
            favorites: true,
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
            music_folders: false,
            folder_browsing: false,
        }
    }
}
