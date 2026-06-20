//! HTTP client for Jellyfin.
//!
//! Phase 0: stub struct. Phase 3 wires `reqwest::Client`, request signing
//! (MediaBrowser auth header), JSON DTOs and typed response handling.

use std::time::Duration;

pub(super) const JELLYFIN_DISCOVERY_PORT: u16 = 7359;
pub(super) const JELLYFIN_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const JELLYFIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub(super) const JELLYFIN_JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(super) const JELLYFIN_IMAGE_MAX_BYTES: usize = 32 * 1024 * 1024;

/// Placeholder HTTP client. Phase 3 replaces this with a real `reqwest`
/// client that signs every request with the MediaBrowser auth header.
pub(super) struct JellyfinClient;
