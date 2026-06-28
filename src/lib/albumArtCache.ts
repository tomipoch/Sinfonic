// JS-side album art cache.
//
// Maps `(itemId, tag)` -> `blob:URL` so subsequent renders are
// synchronous. The prewarm hook fills the cache for the visible
// grid; AlbumCover reads from the cache first and only falls
// through to the IPC layer on a miss.
//
// Blob URLs are owned by this module. `setCached` revokes the
// prior URL for the same key (if any) before overwriting so
// repeated writes — e.g. on a prewarm re-fire — do not leak
// handles. `revokeAll()` drops every entry on a library switch.
//
// P2: bounded LRU. Browsing a long library can fill the cache
// with thousands of blob URLs that are never reused; the cache now
// evicts the least-recently-set entry when it grows past
// `MAX_ENTRIES`. `setCached` is also a "touch" — re-setting a key
// promotes it to most-recently-used. Evicted URLs are revoked.
//
// Implementation note: `Map` preserves insertion order, so the
// first key returned by `keys().next()` is the oldest entry. A
// "touch" is implemented as delete + set so the key moves to the
// tail.

const cache = new Map<string, string>();

/** Maximum number of blob URLs cached in memory. */
export const MAX_ENTRIES = 500;

function keyOf(itemId: string, tag: string | null | undefined): string {
  return `${itemId}\0${tag ?? ""}`;
}

export function getCached(itemId: string, tag: string | null | undefined): string | null {
  return cache.get(keyOf(itemId, tag)) ?? null;
}

export function setCached(itemId: string, tag: string | null | undefined, blobUrl: string): void {
  const key = keyOf(itemId, tag);
  const prior = cache.get(key);
  if (prior && prior !== blobUrl) {
    URL.revokeObjectURL(prior);
  }
  // Delete-then-set promotes an existing key to most-recently-used.
  cache.delete(key);
  cache.set(key, blobUrl);

  // Evict the oldest entries until we're at or below the cap. Cap
  // is generous enough that this loop runs at most a handful of
  // times per call (only when we cross the boundary).
  while (cache.size > MAX_ENTRIES) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey === undefined) break;
    const oldestUrl = cache.get(oldestKey);
    cache.delete(oldestKey);
    if (oldestUrl) URL.revokeObjectURL(oldestUrl);
  }
}

export function buildBlobUrl(bytes: number[], contentType: string): string {
  const u8 = new Uint8Array(bytes);
  const blob = new Blob([u8], { type: contentType });
  return URL.createObjectURL(blob);
}

export function revokeAll(): void {
  for (const url of cache.values()) {
    URL.revokeObjectURL(url);
  }
  cache.clear();
}

/**
 * Test seam — clears the cache WITHOUT revoking the underlying
 * blob URLs. Tests use this to reset state between cases without
 * polluting the `URL.revokeObjectURL` spy. Not exported from the
 * module's public surface; consumers should use `revokeAll`.
 *
 * @internal
 */
export function __resetForTests(): void {
  cache.clear();
}

export function size(): number {
  return cache.size;
}
