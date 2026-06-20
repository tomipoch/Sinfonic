//! `MusicProvider` trait: the abstraction that every music source
//! (Jellyfin, Subsonic, local files) implements.
//!
//! v0.1 ships the full surface defined in `SINFONIC_ARCHITECTURE.md`.
//! Providers that don't support a capability must return
//! `ProviderError::Unsupported` and advertise the absence in
//! `ProviderCapabilities`.

pub mod capabilities;
pub mod error;
pub mod identity;
pub mod provider;
pub mod types;

pub use capabilities::{Capabilities, ProviderCapabilities};
pub use error::{ProviderError, ProviderResult};
pub use identity::{Identity, ProviderIdentity};
pub use provider::MusicProvider;
pub use types::{
    AlbumDetailResponse, ArtistDetailResponse, FavoriteItemId, HomeSection, HomeSectionKind,
    ImageBytes, ImageMetadata, ImageRequest, Lyrics, PlaybackReport, PlaybackReportKind,
    RandomTrackRequest, StreamRequest,
};
