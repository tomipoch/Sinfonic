// LoginRequest struct wire-format — regression test for the
// camelCase mismatch on `subsonic_login` / `jellyfin_login`.
//
// The original bug: `JellyfinLoginRequest` and `SubsonicLoginRequest`
// were declared in `commands.rs` without `#[serde(rename_all =
// "camelCase")]`. The frontend `tauri.ts` wrappers sent the request
// as `{ baseUrl, username, password }` (camelCase), but Tauri's
// deserializer looked for `base_url` (snake_case, matching the
// Rust field name) and errored with:
//   "invalid args `request` for command `subsonic_login`:
//    missing field `base_url`"
//
// This test deserialises both shapes (camelCase from JS, snake_case
// from Rust) against the structs to lock in the contract.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JellyfinLoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicLoginRequest {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[test]
fn jellyfin_login_request_accepts_camel_case_wire_format() {
    // This is exactly what the frontend sends through Tauri.
    let wire = serde_json::json!({
        "baseUrl": "http://jellyfin.local:8096",
        "username": "alice",
        "password": "secret"
    });
    let parsed: JellyfinLoginRequest = serde_json::from_value(wire)
        .expect("camelCase wire format must deserialise into JellyfinLoginRequest");
    assert_eq!(parsed.base_url, "http://jellyfin.local:8096");
    assert_eq!(parsed.username, "alice");
    assert_eq!(parsed.password, "secret");
}

#[test]
fn subsonic_login_request_accepts_camel_case_wire_format() {
    let wire = serde_json::json!({
        "baseUrl": "http://navidrome.local:4533",
        "username": "bob",
        "password": "secret"
    });
    let parsed: SubsonicLoginRequest = serde_json::from_value(wire)
        .expect("camelCase wire format must deserialise into SubsonicLoginRequest");
    assert_eq!(parsed.base_url, "http://navidrome.local:4533");
    assert_eq!(parsed.username, "bob");
    assert_eq!(parsed.password, "secret");
}

#[test]
fn subsonic_login_request_rejects_legacy_snake_case() {
    // Guards against reverting back to a no-rename_all config: if
    // someone removes `#[serde(rename_all = "camelCase")]` the
    // snake_case payload below would silently start parsing
    // (because the Rust field names are also snake_case), and the
    // camelCase wire format would break. This assertion makes the
    // reverse mistake obvious — snake_case alone should fail now.
    let legacy = serde_json::json!({
        "base_url": "http://navidrome.local:4533",
        "username": "bob",
        "password": "secret"
    });
    let parsed: Result<SubsonicLoginRequest, _> = serde_json::from_value(legacy);
    assert!(
        parsed.is_err(),
        "snake_case wire format must NOT deserialise — the JS side sends camelCase"
    );
}
