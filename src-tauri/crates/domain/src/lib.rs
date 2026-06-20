//! Shared domain types for Sinfonic.
//!
//! This crate holds the *vocabulary* of the app: opaque IDs, music entities,
//! queue state, source-level types (paging, streams) and persisted settings.
//!
//! It depends only on `serde` for serialisation. No I/O, no async, no Tauri.
//! That makes it cheap to compile and impossible to leak runtime concerns
//! into the data model.

pub mod entities;
pub mod error;
pub mod ids;
pub mod playback;
pub mod queue;
pub mod route;
pub mod settings;
pub mod source;

pub use entities::{
    Album, AlbumDetail, Artist, ArtistDetail, FolderDetail, FolderEntry, FolderEntryKind, Genre,
    GenreDetail, MusicFolder, Playlist, PlaylistDetail, Track,
};
pub use error::{DomainError, DomainResult};
pub use ids::{
    AlbumId, ArtistId, FolderId, GenreId, MusicFolderId, PlaylistId, QueueEntryId, ServerId,
    SmartPlaylistId, TrackId,
};
pub use playback::PlaybackState;
pub use queue::{
    QueueEngine, QueueEntry, QueueEntryOrigin, QueueReplacement, QueueSnapshot, RepeatMode,
};
pub use route::Route;
pub use settings::AppSettings;
pub use source::{ImageBytes, ImageKind, PagedRequest, PagedResponse, SearchResults, StreamDescriptor};
