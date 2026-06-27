//! Opaque ID types and the `opaque_id!` macro that generates them.

/// Generates an opaque, prefixed ID newtype around a `String`.
///
/// The wrapper provides:
/// - `Clone`, `Debug`, `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd`
/// - `Serialize`/`Deserialize` (transparent)
/// - `new(value)` (panics on empty — use only for trusted internal values)
/// - `try_new(value) -> Result<Self, DomainError>` (preferred for IPC input)
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
            /// **Prefer [`Self::try_new`] for IPC input** — `new` panics on
            /// empty input, which can be triggered by a malformed frontend
            /// payload and would crash the process.
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

            /// Creates a new ID, returning an error if `value` is empty.
            ///
            /// Use this when parsing IDs received from the frontend or other
            /// untrusted sources to avoid panicking the process.
            pub fn try_new(value: impl Into<String>) -> $crate::DomainResult<Self> {
                let value = value.into();
                if value.is_empty() {
                    return ::core::result::Result::Err(
                        $crate::DomainError::InvalidId(concat!(stringify!($name), " must not be empty").to_owned()),
                    );
                }
                ::core::result::Result::Ok(Self(value))
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
    /// Creates a new `QueueEntryId`, panicking if `value` is empty.
    ///
    /// **Prefer [`Self::try_new`] for IPC input** — `new` panics on empty
    /// input, which can be triggered by a malformed frontend payload.
    ///
    /// # Panics
    /// Panics if `value` is empty.
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.is_empty(), "QueueEntryId must not be empty");
        Self(value)
    }

    /// Creates a new `QueueEntryId`, returning an error if `value` is empty.
    pub fn try_new(value: impl Into<String>) -> crate::DomainResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(crate::DomainError::InvalidId(
                "QueueEntryId must not be empty".to_owned(),
            ));
        }
        Ok(Self(value))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_accepts_non_empty() {
        let id = AlbumId::try_new("album-123").expect("non-empty should be ok");
        assert_eq!(id.as_str(), "album-123");
    }

    #[test]
    fn try_new_rejects_empty() {
        let err = AlbumId::try_new("").expect_err("empty should error");
        assert!(matches!(err, crate::DomainError::InvalidId(_)));
    }

    #[test]
    fn try_new_accepts_owned_string() {
        let id = TrackId::try_new(String::from("track-42")).expect("ok");
        assert_eq!(id.as_str(), "track-42");
    }

    #[test]
    fn queue_entry_try_new_rejects_empty() {
        assert!(QueueEntryId::try_new("").is_err());
        assert!(QueueEntryId::try_new("q-1").is_ok());
    }
}
