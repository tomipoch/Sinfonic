//! In-app navigation routes.
//!
//! The frontend uses React Router; the backend doesn't currently consume
//! these (it only fires events). Keeping the enum here means the data
//! model is self-describing and we can later support deep-link payloads
//! from the OS.

use serde::{Deserialize, Serialize};

use super::ids::{AlbumId, ArtistId, GenreId, PlaylistId, SmartPlaylistId};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Route {
    Home,
    Favorites,
    Albums,
    AlbumDetail(AlbumId),
    Tracks,
    Artists,
    ArtistDetail(ArtistId),
    Genres,
    GenreDetail(GenreId),
    Playlists,
    PlaylistDetail(PlaylistId),
    SmartPlaylists,
    SmartPlaylistDetail(SmartPlaylistId),
    Search,
    Settings,
}
