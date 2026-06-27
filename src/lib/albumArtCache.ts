// JS-side album art cache.
//
// Maps `(itemId, tag)` -> `blob:URL` so subsequent renders are
// synchronous. The prewarm hook fills the cache for the visible
// grid; AlbumCover reads from the cache first and only falls
// through to the IPC layer on a miss.
//
// Blob URLs are owned by this module. `prune()` revokes and
// removes them when the active provider changes (e.g. logout)
// so we do not leak handles when the user switches libraries.

const cache = new Map<string, string>();

function keyOf(itemId: string, tag: string | null | undefined): string {
  return `${itemId}\0${tag ?? ""}`;
}

export function getCached(itemId: string, tag: string | null | undefined): string | null {
  return cache.get(keyOf(itemId, tag)) ?? null;
}

export function setCached(
  itemId: string,
  tag: string | null | undefined,
  blobUrl: string,
): void {
  cache.set(keyOf(itemId, tag), blobUrl);
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

export function size(): number {
  return cache.size;
}
