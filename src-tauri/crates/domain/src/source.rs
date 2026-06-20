//! Types shared between the app and any `MusicProvider`.
//!
//! Kept in `domain` so providers depend on a single source of truth.

use serde::{Deserialize, Serialize};

use super::ids::TrackId;

/// Page request for collection-style provider methods.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct PagedRequest {
    pub offset: usize,
    pub limit: usize,
}

impl PagedRequest {
    pub fn new(offset: usize, limit: usize) -> Self {
        Self { offset, limit }
    }
}

/// Page response with total count.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PagedResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
}

impl<T> PagedResponse<T> {
    pub fn new(items: Vec<T>, total: usize) -> Self {
        Self { items, total }
    }
}

/// A `uri` for a streamable track, with a redacted copy safe for logging.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StreamDescriptor {
    uri: String,
    redacted_uri: String,
    source_start_millis: Option<u64>,
    source_end_millis: Option<u64>,
}

impl StreamDescriptor {
    pub fn new(uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let redacted_uri = redact_uri(&uri);
        Self {
            uri,
            redacted_uri,
            source_start_millis: None,
            source_end_millis: None,
        }
    }

    pub fn with_redacted(uri: impl Into<String>, redacted_uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            redacted_uri: redacted_uri.into(),
            source_start_millis: None,
            source_end_millis: None,
        }
    }

    pub fn with_source_window(mut self, start_millis: u64, end_millis: u64) -> Self {
        self.source_start_millis = Some(start_millis);
        self.source_end_millis = Some(end_millis);
        self
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn redacted_uri(&self) -> &str {
        &self.redacted_uri
    }

    pub fn source_start_millis(&self) -> Option<u64> {
        self.source_start_millis
    }

    pub fn source_end_millis(&self) -> Option<u64> {
        self.source_end_millis
    }
}

/// Strips query parameters that look like credentials (`api_key=`, `token=`,
/// `t=`, `s=`) from a URI for safe logging.
fn redact_uri(uri: &str) -> String {
    if let Some((scheme_and_host, query)) = uri.split_once('?') {
        let safe_query: Vec<&str> = query
            .split('&')
            .filter(|kv| {
                let key = kv.split('=').next().unwrap_or("");
                !matches!(key, "api_key" | "token" | "t" | "s" | "X-Emby-Token")
            })
            .collect();
        if safe_query.is_empty() {
            scheme_and_host.to_string()
        } else {
            format!("{scheme_and_host}?{}", safe_query.join("&"))
        }
    } else {
        uri.to_string()
    }
}

/// What the UI asked for when hitting a search endpoint.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum SearchKind {
    #[default]
    All,
    Albums,
    Tracks,
    Artists,
    Playlists,
}

/// Result of a search across all entity kinds.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SearchResults {
    pub albums: Vec<super::Album>,
    pub tracks: Vec<super::Track>,
    pub artists: Vec<super::Artist>,
    pub playlists: Vec<super::Playlist>,
}

impl SearchResults {
    pub fn is_empty(&self) -> bool {
        self.albums.is_empty()
            && self.tracks.is_empty()
            && self.artists.is_empty()
            && self.playlists.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ImageKind {
    Primary,
    Backdrop,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageRequest {
    pub item_id: String,
    pub kind: ImageKind,
    pub tag: Option<String>,
    pub size: u32,
}

impl ImageRequest {
    pub fn primary(item_id: impl Into<String>, size: u32) -> Self {
        Self {
            item_id: item_id.into(),
            kind: ImageKind::Primary,
            tag: None,
            size,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageBytes {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

#[allow(dead_code)]
fn _ensure_track_id_used(_: TrackId) {}
