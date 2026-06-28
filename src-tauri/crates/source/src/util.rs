//! Small string-utility helpers shared by every `MusicProvider`
//! implementation. These used to be duplicated across
//! `sinfonic-source-jellyfin`, `sinfonic-source-subsonic` and
//! `sinfonic-source-local` with byte-identical bodies; centralising
//! them means a fix or a new helper is a one-place change.

/// Strip `prefix` from `s` if present; otherwise return `s` unchanged.
///
/// Equivalent to `s.strip_prefix(prefix).unwrap_or(s)` — provided as
/// a free function so call sites read as intent ("strip this prefix
/// if you can") rather than as the underlying generic library call.
#[inline]
pub fn strip_prefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

/// Split an `ImageRef::item_id` of the form `"<kind>:<id>"` into
/// `(kind, id)`. If no colon is present, returns `("", id)` so the
/// caller can fall back to a default kind.
#[inline]
pub fn split_image_id(item_id: &str) -> (&str, &str) {
    match item_id.split_once(':') {
        Some((kind, id)) => (kind, id),
        None => ("", item_id),
    }
}

/// Slugify a server / playlist / user name so it can safely be used
/// as part of an identifier (we glue slugs into `ServerId`/`PlaylistId`
/// values). Lowercases, replaces non-alphanumeric runs with `-`, and
/// trims leading/trailing dashes.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true; // suppress leading dashes
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_prefix_matches_when_present() {
        assert_eq!(strip_prefix("server-foo", "server-"), "foo");
        assert_eq!(strip_prefix("server-foo", "jellyfin-"), "server-foo");
    }

    #[test]
    fn split_image_id_splits_on_colon() {
        assert_eq!(split_image_id("Primary:abc"), ("Primary", "abc"));
        assert_eq!(split_image_id(""), ("", ""));
        assert_eq!(split_image_id("no-colon-here"), ("", "no-colon-here"));
    }

    #[test]
    fn slugify_lowercases_and_dashes() {
        assert_eq!(slugify("My Server"), "my-server");
        assert_eq!(slugify("  weird!!name  "), "weird-name");
        assert_eq!(slugify("CamelCase"), "camelcase");
        assert_eq!(slugify("---"), "unnamed");
        assert_eq!(slugify("foo123"), "foo123");
    }
}