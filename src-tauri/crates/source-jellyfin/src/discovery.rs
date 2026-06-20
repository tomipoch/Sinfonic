//! UDP broadcast discovery for Jellyfin servers.
//!
//! Phase 0: stub that returns an empty list. Real implementation in Phase 3.

use sinfonic_source::ProviderResult;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct DiscoveredJellyfinServer {
    pub name: String,
    pub base_url: String,
    pub server_id: String,
}

pub async fn discover_jellyfin_servers(
    _timeout: Duration,
) -> ProviderResult<Vec<DiscoveredJellyfinServer>> {
    // Real impl: send "Who is JellyfinServer?" to 255.255.255.255:7359 and
    // also probe 127.0.0.1:8096/System/Info/Public.
    Ok(Vec::new())
}
