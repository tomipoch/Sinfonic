//! `LrclibClient` — the public-facing HTTP client for LRCLIB.
//!
//! Holds a single `reqwest::Client` (cheap to share, expensive to
//! recreate) plus a bounded `lru::LruCache` keyed by
//! `(artist, title, duration_seconds)` to avoid hammering LRCLIB
//! when the user scrubs back and forth.
//!
//! The cache stores both `Some` (positive match) and `None`
//! ("definitively not found") so a 404 the first time doesn't
//! translate to a 404 every subsequent fetch.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::dto::{LrclibResponse, LyricsHit};
use crate::error::{LyricsError, LyricsResult};
use crate::LyricsQuery;

/// Number of distinct queries we remember across the session.
/// 200 covers a typical listening session comfortably while
/// bounding memory at ~50 KB of metadata (cache stores at most 200
/// tuples of small `LyricsHit`s).
const DEFAULT_CACHE_CAPACITY: usize = 200;

/// HTTP request timeout. LRCLIB is generous but we don't want a
/// hung socket to wedge the lyrics panel forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Public client. Cheap to clone (handles are `Arc` internally).
#[derive(Clone)]
pub struct LrclibClient {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    base_url: reqwest::Url,
    cache: Mutex<lru::LruCache<String, Option<LyricsHit>>>,
}

impl LrclibClient {
    /// Build a client. `user_agent` is the suffix appended to
    /// `Sinfonic/` so anyone running LRCLIB can identify us in
    /// their logs. `base_url` defaults to `https://lrclib.net`
    /// — tests override it to point at a `wiremock` server.
    pub fn new(base_url: reqwest::Url, user_agent: String) -> LyricsResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!(
                "Sinfonic/{} ({})",
                user_agent,
                "https://github.com/tomipoch/Sinfonic"
            ))
            .build()?;
        Ok(Self {
            inner: Arc::new(Inner {
                http,
                base_url,
                cache: Mutex::new(lru::LruCache::new(
                    DEFAULT_CACHE_CAPACITY
                        .try_into()
                        .expect("non-zero capacity"),
                )),
            }),
        })
    }

    /// Look up `q`. Returns `Ok(None)` when LRCLIB has no record
    /// for the query — that is the common path and is **not** an
    /// error. Real failures (network down, malformed body) become
    /// `Err(LyricsError::…)` so the caller can decide.
    pub async fn fetch(&self, q: &LyricsQuery<'_>) -> LyricsResult<Option<LyricsHit>> {
        let key = Self::cache_key(q);

        // Fast path: cache hit.
        if let Some(hit) = self.inner.cache.lock().get(&key).cloned() {
            tracing::trace!(query = %key, "lrclib cache hit");
            return Ok(hit);
        }

        let url = self.build_url(q)?;
        let resp = self
            .inner
            .http
            .get(url)
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            tracing::debug!(query = %key, "lrclib 404");
            let slot: Option<LyricsHit> = None;
            self.inner.cache.lock().put(key, slot.clone());
            return Ok(slot);
        }
        if !status.is_success() {
            return Err(LyricsError::Network(
                resp.error_for_status().expect_err("non-success"),
            ));
        }

        let body: LrclibResponse = resp
            .json()
            .await
            .map_err(|e| LyricsError::Decode(e.to_string()))?;

        let hit = body.into_hit();
        self.inner.cache.lock().put(key, hit.clone());
        Ok(hit)
    }

    /// Construct the canonical LRCLIB `/api/get` URL from a query.
    /// Centralised so tests can assert the wire format.
    fn build_url(&self, q: &LyricsQuery<'_>) -> LyricsResult<reqwest::Url> {
        let mut url = self
            .inner
            .base_url
            .join("/api/get")
            .map_err(|e| LyricsError::Decode(format!("base_url join: {e}")))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("artist_name", q.artist);
            pairs.append_pair("track_name", q.title);
            if let Some(album) = q.album {
                pairs.append_pair("album_name", album);
            }
            if let Some(d) = q.duration_seconds {
                pairs.append_pair("duration", &d.to_string());
            }
        }
        Ok(url)
    }

    /// Build the LRU key. We deliberately normalise to lowercase
    /// + trim so `"The Eagles"/"the eagles"` collide on the cache.
    fn cache_key(q: &LyricsQuery<'_>) -> String {
        format!(
            "{}|{}|{}",
            q.artist.trim().to_lowercase(),
            q.title.trim().to_lowercase(),
            q.duration_seconds.unwrap_or(0),
        )
    }
}
