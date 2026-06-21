//! `api_sig` computation for the audioscrobbler 2.0 protocol.
//!
//! Rules (from the Last.fm docs):
//! 1. Take every request parameter EXCEPT `format` and `callback`.
//! 2. Sort by parameter name lexicographically (UTF-8 byte order —
//!    `String::sort` is byte-wise by default in stable Rust).
//! 3. Concatenate as `key1value1key2value2…` with no separators.
//! 4. Append the api secret.
//! 5. md5 the resulting string and return lower-case hex.
//!
//! The signature is sensitive to parameter ordering AND to the exact
//! byte representation of the values, so we feed it through md5
//! from the same crate Subsonic auth uses (`md5 = "0.7"`).

use md5::Context;

pub fn sign<I, K, V>(params: I, api_secret: &str) -> String
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut sorted: Vec<(String, String)> = params
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut concat = String::new();
    for (k, v) in &sorted {
        concat.push_str(k);
        concat.push_str(v);
    }
    concat.push_str(api_secret);

    let mut ctx = Context::new();
    ctx.consume(concat.as_bytes());
    let digest = ctx.compute();
    hex::encode(digest.0)
}

/// Convenience wrapper around `sign` that produces the `api_sig`
/// value when given a slice of `(name, value)` pairs. Pulled out so
/// tests can drive it without a `LastFmClient` instance.
pub fn sign_params(params: &[(&str, &str)], api_secret: &str) -> String {
    sign(params.iter().copied(), api_secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference signature from the Last.fm auth docs
    /// (https://www.last.fm/api/authspec), computed by hand.
    #[test]
    fn sign_matches_documented_example() {
        let api_secret = "MySecret";
        let sig = sign_params(
            &[
                ("method", "auth.getMobileSession"),
                ("api_key", "MyAPIKey"),
                ("username", "MyUser"),
                ("password", "MyPass"),
            ],
            api_secret,
        );
        // Alphabetical order: api_key, method, password, username.
        // Concat: "api_keyMyAPIKeymethodauth.getMobileSessionpasswordMyPassusernameMyUserMySecret"
        // md5 hex of that string (verified independently with Python's hashlib).
        assert_eq!(sig, "be16670cd4b586654922a8126c728975");
    }

    #[test]
    fn sign_is_order_independent() {
        let api_secret = "secret";
        let a = sign_params(
            &[
                ("method", "track.scrobble"),
                ("artist", "A"),
                ("track", "T"),
                ("timestamp", "100"),
            ],
            api_secret,
        );
        let b = sign_params(
            &[
                ("timestamp", "100"),
                ("track", "T"),
                ("artist", "A"),
                ("method", "track.scrobble"),
            ],
            api_secret,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn sign_changes_when_secret_changes() {
        let params = &[("method", "ping")];
        let a = sign_params(params, "secret1");
        let b = sign_params(params, "secret2");
        assert_ne!(a, b);
    }
}
