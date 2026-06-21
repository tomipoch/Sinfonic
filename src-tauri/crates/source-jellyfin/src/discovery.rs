//! UDP broadcast discovery for Jellyfin servers.
//!
//! Jellyfin advertises itself on UDP port 7359 with a JSON envelope
//! (`DiscoveryEnvelope`) every 500ms while the server is running. We
//! listen on the broadcast address for `timeout` and collect the
//! unique responses. The format is documented in the Jellyfin
//! `Jellyfin.Api/Controllers/DiscoveryController.cs` source.
//!
//! On networks that block broadcast (corporate Wi-Fi, some IPv6
//! configs) we fall back to probing a few well-known local addresses:
//! `127.0.0.1:8096`, `[::1]:8096` and the broadcast-receiving port.
//! The probe path is best-effort: anything that answers with a 200
//! from `/System/Info/Public` is included.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::Value;
use sinfonic_source::{ProviderError, ProviderResult};
use tokio::net::UdpSocket;
use tokio::time;

use super::client::{JellyfinClient, JELLYFIN_DISCOVERY_PORT};
use super::dto::{DiscoveryEnvelope, PublicSystemInfo};

/// One server discovered on the LAN.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveredJellyfinServer {
    pub name: String,
    pub base_url: String,
    pub server_id: String,
}

/// Listen on the Jellyfin UDP discovery port and collect unique
/// responses for `timeout`. Falls back to local probes if no
/// responses arrive.
pub async fn discover_jellyfin_servers(timeout: Duration) -> Vec<DiscoveredJellyfinServer> {
    let mut found = broadcast_discover(timeout).await.unwrap_or_default();
    if found.is_empty() {
        found = local_probe().await;
    }
    // Stable ordering helps the UI.
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found.dedup_by(|a, b| a.server_id == b.server_id);
    found
}

async fn broadcast_discover(timeout: Duration) -> ProviderResult<Vec<DiscoveredJellyfinServer>> {
    let bind_addr: SocketAddr = SocketAddr::from(([0, 0, 0, 0], JELLYFIN_DISCOVERY_PORT));
    let socket = UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| ProviderError::Network(format!("bind {bind_addr}: {e}")))?;
    socket
        .set_broadcast(true)
        .map_err(|e| ProviderError::Network(format!("set_broadcast: {e}")))?;

    let deadline = time::Instant::now() + timeout;
    let mut found = Vec::new();

    while time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(time::Instant::now());
        let mut buf = vec![0u8; 8192];
        let recv = match time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, origin))) => (len, origin),
            _ => break,
        };
        let (len, origin) = recv;
        if let Some(server) = parse_envelope(&buf[..len], origin.ip()) {
            found.push(server);
        }
    }

    Ok(found)
}

fn parse_envelope(payload: &[u8], origin: IpAddr) -> Option<DiscoveredJellyfinServer> {
    // Jellyfin broadcasts `{"Address":"...","Port":...,"Id":"...","Name":"..."}`.
    let env: DiscoveryEnvelope = serde_json::from_slice(payload).ok()?;
    let host = if env.address.is_empty() {
        origin.to_string()
    } else {
        env.address
    };
    let base_url = format!("http://{}:{}", host, env.port);
    Some(DiscoveredJellyfinServer {
        name: env.name,
        base_url,
        server_id: env.id,
    })
}

/// Best-effort fallback for environments without UDP broadcast.
/// Probes the loopback address at the default Jellyfin port and
/// returns every server that answers `/System/Info/Public`.
async fn local_probe() -> Vec<DiscoveredJellyfinServer> {
    let candidates: [SocketAddr; 2] = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8096),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8096),
    ];
    let mut found = Vec::new();
    for addr in candidates {
        if let Some(server) = probe_one(&addr.to_string()).await {
            found.push(server);
        }
    }
    found
}

async fn probe_one(addr: &str) -> Option<DiscoveredJellyfinServer> {
    let url = match url::Url::parse(&format!("http://{addr}")) {
        Ok(u) => u,
        Err(_) => return None,
    };
    let client = JellyfinClient::new(url).ok()?;
    // Use the super::client::AuthContext with no token — `/System/Info/Public`
    // is unauthenticated.
    let auth = super::client::AuthContext {
        device_id: "sinfonic-discovery".into(),
        access_token: None,
    };
    let info: PublicSystemInfo = match client.get_json("System/Info/Public", &auth).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    Some(DiscoveredJellyfinServer {
        name: info.server_name,
        base_url: format!("http://{addr}"),
        server_id: info.id,
    })
}

/// Parse a single envelope from raw bytes (used by tests).
pub fn parse_envelope_for_test(payload: &[u8], origin: IpAddr) -> Option<DiscoveredJellyfinServer> {
    parse_envelope(payload, origin)
}

/// Extract a `PublicSystemInfo` from a JSON value (used by tests that
/// don't want to spin up an HTTP server).
pub fn parse_public_info(value: &Value) -> Option<PublicSystemInfo> {
    serde_json::from_value(value.clone()).ok()
}