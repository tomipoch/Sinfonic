//! FTS5 helpers. Phase 0: stub. Phase 2: real FTS5 virtual table + queries.

#![allow(dead_code)]

use sinfonic_domain::{Album, Artist, SearchResults, Track};

pub fn search_albums(_query: &str) -> Result<Vec<Album>, String> {
    Ok(Vec::new())
}

pub fn search_tracks(_query: &str) -> Result<Vec<Track>, String> {
    Ok(Vec::new())
}

pub fn search_artists(_query: &str) -> Result<Vec<Artist>, String> {
    Ok(Vec::new())
}

pub fn search_all(_query: &str) -> Result<SearchResults, String> {
    Ok(SearchResults::default())
}
