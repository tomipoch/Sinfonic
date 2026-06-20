//! Opaque ID types and the `opaque_id!` macro that generates them.

/// Generates an opaque, prefixed ID newtype around a `String`.
///
/// The wrapper provides:
/// - `Clone`, `Debug`, `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd`
/// - `Serialize`/`Deserialize` (transparent)
/// - `new(value)` (panics on empty)
/// - `fake(n)` (test helper, uses prefix)
/// - `as_str()` accessor
/// - `From<&str>` / `From<String>` / `Display`
///
/// # Example
/// ```
/// sinfonic_domain::opaque_id!(AlbumId, "album-");
/// let id = AlbumId::new("album-123");
/// assert_eq!(id.as_str(), "album-123");
/// ```
#[macro_export]
macro_rules! opaque_id {
    ($name:ident, $prefix:expr) => {
        #[derive(
            ::serde::Deserialize,
            ::serde::Serialize,
            Clone,
            Debug,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates a new ID from any string-like value.
            ///
            /// # Panics
            /// Panics if `value` is empty.
            pub fn new(value: impl Into<String>) -> Self {
                let value = value.into();
                assert!(
                    !value.is_empty(),
                    concat!(stringify!($name), " must not be empty")
                );
                Self(value)
            }

            /// Test helper: returns a synthetic ID like `"prefix-42"`.
            pub fn fake(n: u32) -> Self {
                Self(format!("{}{}", $prefix, n))
            }

            /// Returns the underlying string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self::new(s)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

opaque_id!(AlbumId, "album-");
opaque_id!(TrackId, "track-");
opaque_id!(ArtistId, "artist-");
opaque_id!(GenreId, "genre-");
opaque_id!(PlaylistId, "playlist-");
opaque_id!(SmartPlaylistId, "smart-playlist-");
opaque_id!(ServerId, "server-");
opaque_id!(MusicFolderId, "music-folder-");
opaque_id!(FolderId, "folder-");

/// `QueueEntryId` has no prefix (matches Rufin design).
#[derive(
    serde::Deserialize, serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct QueueEntryId(String);

impl QueueEntryId {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.is_empty(), "QueueEntryId must not be empty");
        Self(value)
    }

    pub fn fake(n: u32) -> Self {
        Self(format!("queue-{n}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for QueueEntryId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for QueueEntryId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for QueueEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
