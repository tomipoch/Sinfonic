//! `ProviderCapabilities` declares what a `MusicProvider` can do.
//!
//! After the `feature/cleanup-phase2` audit only the flags the
//! frontend actually checks against are kept here. Every other
//! flag used to advertise a `MusicProvider` method that the UI
//! never called (random tracks, folder browsing, music folders,
//! image metadata, …) has been removed alongside the trait
//! methods.

use serde::{Deserialize, Serialize};

/// Alias for trait implementations that import the name with `Capabilities`.
pub type Capabilities = ProviderCapabilities;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderCapabilities {
    pub albums: bool,
    pub tracks: bool,
    pub artists: bool,
    pub playlists: bool,
    pub favorites: bool,
    pub lyrics: bool,
    pub playback_reporting: bool,
    pub playlist_mutations: bool,
    pub playlist_delete: bool,
    pub favorite_mutations: bool,
    pub search: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            albums: true,
            tracks: true,
            artists: true,
            playlists: true,
            favorites: true,
            lyrics: false,
            playback_reporting: false,
            playlist_mutations: false,
            playlist_delete: false,
            favorite_mutations: false,
            search: true,
        }
    }
}
