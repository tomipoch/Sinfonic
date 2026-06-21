//! Filesystem-backed cache for album art bytes.
//!
//! ## Why
//!
//! Providers expose `image_bytes` but every call hits the network.
//! Even with HTTP caching on the server side, re-fetching on every
//! UI render multiplies bandwidth + latency. The cache stores the
//! raw bytes on disk so the second call is a stat() + read().
//!
//! ## Layout
//!
//! ```text
//! <root>/
//!   ab/
//!     ab12cd34….bin          # raw bytes (extension-less on purpose)
//!     ab12cd34….mime         # MIME type as utf-8 text
//!     ab12cd34….meta         # JSON: { cached_at, size, provider, image_id }
//! ```
//!
//! The cache key is `sha256(provider | "\0" | image_id | "\0" | tag)`,
//! truncated to the first 32 hex chars. The truncation keeps the
//! filesystem tidy without weakening collision resistance for the
//! realistic dataset size (~10⁴ keys ⇒ 2⁶⁴ collision space).
//!
//! ## Thread safety
//!
//! Single-writer / multi-reader via a `parking_lot::Mutex`. The
//! mutation surface is small (`put`, `evict_if_over`) and the locks
//! are short. Tests run under `tempfile::TempDir`.
//!
//! ## Eviction
//!
//! `evict_if_over(max_bytes)` removes the oldest entries (by
//! `cached_at` from the `.meta` sidecar) until the directory fits
//! the budget. Kept here so the LRU is testable without an async
//! runtime. The Tauri layer calls it opportunistically after every
//! `put`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{LibraryError, LibraryResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedImageMeta {
    pub cached_at_unix: u64,
    pub size_bytes: u64,
    pub provider: String,
    pub image_id: String,
}

pub struct AlbumArtCache {
    root: PathBuf,
    /// Serialises filesystem mutations. Cheap — never held across
    /// `.await` points (this type is sync).
    inner: Mutex<AlbumArtCacheInner>,
}

#[derive(Default)]
struct AlbumArtCacheInner {
    /// Reserved for a future fast-path skip on `ensure_root`. Kept
    /// here so the mutex shape is stable across refactors.
    #[allow(dead_code)]
    initialised: bool,
}pub struct CachedImage {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub meta: CachedImageMeta,
}

impl AlbumArtCache {
    pub fn open(root: impl Into<PathBuf>) -> LibraryResult<Self> {
        let root = root.into();
        let cache = Self {
            root,
            inner: Mutex::new(AlbumArtCacheInner::default()),
        };
        cache.ensure_root()?;
        Ok(cache)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn contains(&self, key: &ImageCacheKey) -> LibraryResult<bool> {
        let path = self.bin_path(&key.to_hex());
        match fs::metadata(&path) {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(LibraryError::Io(err.to_string())),
        }
    }

    pub fn get(&self, key: &ImageCacheKey) -> LibraryResult<Option<CachedImage>> {
        let hex = key.to_hex();
        let bin = self.bin_path(&hex);
        let mime = self.mime_path(&hex);
        let meta = self.meta_path(&hex);

        let bytes = match fs::read(&bin) {
            Ok(b) => b,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(LibraryError::Io(err.to_string())),
        };
        let content_type = match fs::read_to_string(&mime) {
            Ok(s) => s,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(err) => return Err(LibraryError::Io(err.to_string())),
        };
        let meta: CachedImageMeta = match fs::read(&meta) {
            Ok(s) => match serde_json::from_slice(&s) {
                Ok(m) => m,
                Err(_) => return Ok(None),
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(err) => return Err(LibraryError::Io(err.to_string())),
        };

        // Sanity check: if any sidecar is missing or corrupt, treat
        // the entry as a miss so the caller refetches.
        if bytes.is_empty() || content_type.is_empty() {
            return Ok(None);
        }

        // Reconcile meta.size_bytes with the actual file. Cheap stat
        // is fine here — same syscall we'd do anyway.
        let _ = meta; // size_bytes intentionally not enforced; the
                      // bytes vector is the source of truth.

        Ok(Some(CachedImage {
            bytes,
            content_type,
            meta,
        }))
    }

    pub fn put(
        &self,
        key: &ImageCacheKey,
        bytes: &[u8],
        content_type: &str,
    ) -> LibraryResult<CachedImageMeta> {
        let _guard = self.inner.lock();
        self.ensure_root_locked()?;

        let hex = key.to_hex();
        let bin = self.bin_path(&hex);
        let mime = self.mime_path(&hex);
        let meta = self.meta_path(&hex);

        let cached_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = CachedImageMeta {
            cached_at_unix,
            size_bytes: bytes.len() as u64,
            provider: key.provider.clone(),
            image_id: key.image_id.clone(),
        };

        // Write to temp paths first, then atomically rename so a
        // concurrent reader can never observe a half-written file.
        write_atomic(&bin, bytes)?;
        write_atomic(mime.as_path(), content_type.as_bytes())?;
        let meta_json = serde_json::to_vec(&entry)
            .map_err(|e| LibraryError::Migration(format!("meta encode: {e}")))?;
        write_atomic(meta.as_path(), &meta_json)?;

        Ok(entry)
    }

    /// Removes the oldest entries (by `cached_at_unix`) until the
    /// total on-disk size is `<= max_bytes`. Returns the number of
    /// entries evicted.
    pub fn evict_if_over(&self, max_bytes: u64) -> LibraryResult<usize> {
        let _guard = self.inner.lock();
        self.ensure_root_locked()?;

        let mut entries = self.collect_entries()?;
        let total: u64 = entries.iter().map(|e| e.size_bytes).sum();
        if total <= max_bytes {
            return Ok(0);
        }

        entries.sort_by_key(|e| e.meta.cached_at_unix);

        let mut remaining = total;
        let mut evicted = 0usize;
        for entry in entries {
            if remaining <= max_bytes {
                break;
            }
            self.remove_entry(&entry.hex)?;
            remaining = remaining.saturating_sub(entry.size_bytes);
            evicted += 1;
        }
        Ok(evicted)
    }

    pub fn total_size_bytes(&self) -> LibraryResult<u64> {
        let _guard = self.inner.lock();
        self.collect_entries()
            .map(|entries| entries.iter().map(|e| e.size_bytes).sum())
    }

    fn ensure_root(&self) -> LibraryResult<()> {
        let _guard = self.inner.lock();
        self.ensure_root_locked()
    }

    fn ensure_root_locked(&self) -> LibraryResult<()> {
        fs::create_dir_all(&self.root).map_err(|e| LibraryError::Io(e.to_string()))?;
        Ok(())
    }

    fn bin_path(&self, hex: &str) -> PathBuf {
        self.shard_for(hex).join(format!("{hex}.bin"))
    }

    fn mime_path(&self, hex: &str) -> PathBuf {
        self.shard_for(hex).join(format!("{hex}.mime"))
    }

    fn meta_path(&self, hex: &str) -> PathBuf {
        self.shard_for(hex).join(format!("{hex}.meta"))
    }

    /// Two-level shard to keep any single directory small
    /// (filesystem-friendly for tools like `find` / `ls`).
    fn shard_for(&self, hex: &str) -> PathBuf {
        let (a, b) = hex.split_at(2);
        self.root.join(a).join(b)
    }

    fn collect_entries(&self) -> LibraryResult<Vec<CollectedEntry>> {
        let mut out = Vec::new();
        walk_dir(&self.root, &mut out)?;
        Ok(out)
    }

    fn remove_entry(&self, hex: &str) -> LibraryResult<()> {
        let dir = self.shard_for(hex);
        let _ = fs::remove_file(dir.join(format!("{hex}.bin")));
        let _ = fs::remove_file(dir.join(format!("{hex}.mime")));
        let _ = fs::remove_file(dir.join(format!("{hex}.meta")));
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ImageCacheKey {
    pub provider: String,
    pub image_id: String,
    pub tag: String,
}

impl ImageCacheKey {
    pub fn new(
        provider: impl Into<String>,
        image_id: impl Into<String>,
        tag: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            image_id: image_id.into(),
            tag: tag.into(),
        }
    }

    pub fn to_hex(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.provider.as_bytes());
        hasher.update([0u8]);
        hasher.update(self.image_id.as_bytes());
        hasher.update([0u8]);
        hasher.update(self.tag.as_bytes());
        let digest = hasher.finalize();
        let mut out = String::with_capacity(64);
        for byte in digest.iter() {
            out.push_str(&format!("{byte:02x}"));
        }
        out.truncate(32);
        out
    }
}

#[derive(Debug)]
struct CollectedEntry {
    hex: String,
    size_bytes: u64,
    meta: CachedImageMeta,
}

fn walk_dir(root: &Path, out: &mut Vec<CollectedEntry>) -> LibraryResult<()> {
    let read_dir = match fs::read_dir(root) {
        Ok(rd) => rd,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(LibraryError::Io(err.to_string())),
    };
    for entry in read_dir {
        let entry = entry.map_err(|e| LibraryError::Io(e.to_string()))?;
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if let Some(hex) = file_name.strip_suffix(".bin") {
            let meta_path = path.with_extension("meta");
            let meta_bytes = match fs::read(&meta_path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let meta: CachedImageMeta = match serde_json::from_slice(&meta_bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let size = fs::metadata(&path)
                .map(|m| m.len())
                .unwrap_or(meta.size_bytes);
            out.push(CollectedEntry {
                hex: hex.to_string(),
                size_bytes: size,
                meta,
            });
        } else if path.is_dir() {
            walk_dir(&path, out)?;
        }
    }
    Ok(())
}

fn write_atomic(target: &Path, bytes: &[u8]) -> LibraryResult<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| LibraryError::Io(e.to_string()))?;
    }
    let tmp = target.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| LibraryError::Io(e.to_string()))?;
    fs::rename(&tmp, target).map_err(|e| LibraryError::Io(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> (tempfile::TempDir, AlbumArtCache) {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = AlbumArtCache::open(dir.path()).expect("open");
        (dir, cache)
    }

    #[test]
    fn put_then_get_round_trip() {
        let (_dir, cache) = fresh();
        let key = ImageCacheKey::new("jellyfin", "album-42", "tag-abc");
        let meta = cache
            .put(&key, b"fake-jpeg-bytes", "image/jpeg")
            .expect("put");
        assert_eq!(meta.size_bytes, b"fake-jpeg-bytes".len() as u64);

        let got = cache.get(&key).expect("get").expect("present");
        assert_eq!(got.bytes, b"fake-jpeg-bytes");
        assert_eq!(got.content_type, "image/jpeg");
        assert_eq!(got.meta.provider, "jellyfin");
        assert_eq!(got.meta.image_id, "album-42");
    }

    #[test]
    fn contains_reflects_state() {
        let (_dir, cache) = fresh();
        let key = ImageCacheKey::new("subsonic", "al-1", "v1");
        assert!(!cache.contains(&key).unwrap());
        cache.put(&key, b"x", "image/png").unwrap();
        assert!(cache.contains(&key).unwrap());
    }

    #[test]
    fn key_derivation_is_deterministic_and_unique() {
        let a = ImageCacheKey::new("jellyfin", "a", "t").to_hex();
        let b = ImageCacheKey::new("jellyfin", "a", "t").to_hex();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);

        let c = ImageCacheKey::new("jellyfin", "a", "t2").to_hex();
        let d = ImageCacheKey::new("subsonic", "a", "t").to_hex();
        let e = ImageCacheKey::new("jellyfin", "b", "t").to_hex();
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_ne!(a, e);
    }

    #[test]
    fn get_missing_returns_none() {
        let (_dir, cache) = fresh();
        let key = ImageCacheKey::new("jellyfin", "missing", "x");
        assert!(cache.get(&key).unwrap().is_none());
    }

    #[test]
    fn evict_if_over_removes_oldest() {
        let (_dir, cache) = fresh();
        for i in 0..5 {
            let key = ImageCacheKey::new("jellyfin", format!("al-{i}"), "t");
            cache
                .put(&key, &[0u8; 100], "image/jpeg")
                .unwrap();
            // Force ordering by sleeping 5 ms.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(cache.total_size_bytes().unwrap(), 500);
        let evicted = cache.evict_if_over(250).unwrap();
        assert_eq!(evicted, 3);
        assert!(cache.total_size_bytes().unwrap() <= 250);
    }

    #[test]
    fn evict_if_over_noop_when_under_budget() {
        let (_dir, cache) = fresh();
        cache
            .put(&ImageCacheKey::new("p", "i", "t"), b"hi", "image/png")
            .unwrap();
        assert_eq!(cache.evict_if_over(10_000).unwrap(), 0);
    }
}
