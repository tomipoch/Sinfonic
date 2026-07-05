// Pre-warm the JS-side album-art cache for the active server's
// library. The Rust-side filesystem cache already absorbs the
// per-byte cost — this hook's job is to eliminate the per-cell
// IPC roundtrip + blob-URL creation by issuing one bulk fetch
// for every visible cover and warming the blob URL map up front.
//
// `provider_image_bytes_bulk` resolves cache hits on the Rust
// side and fans out the misses in parallel, so even the first
// paint after login lands with all artwork decoded.
//
// Phase 8 of feature/direct-fetch-providers: track covers are no
// longer prewarmed separately. The `TrackTable` now prefers
// `album?.imageRef` over `track.imageRef`, so tracks resolve via
// the cached album row without a second network round-trip. The
// track image_ref field stays on the row for any future provider
// that genuinely differentiates per-track art, but in practice
// (Subsonic, Navidrome, etc.) tracks and their album share the
// same image. Dropping the track prewarm roughly halves the
// per-page request count.
//
// Fire-and-forget: errors are logged but never surfaced. AlbumCover
// falls back to the gradient on a missing hit, so a partial
// pre-warm is invisible to the user.

import { useEffect } from "react";
import { buildBlobUrl, getCached, setCached } from "@/lib/albumArtCache";
import { type AlbumArtRequest, providerImageBytesBulk } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";

/// How many of the *first* covers to prewarm. Phase 8: dropped
/// from the full page (200) to 24 so the initial prewarm lands
/// in ~3 s on the user's Subsonic server. The rest of the page's
/// covers are still served by the cache (album dedup means the
/// first 24 hits cover the common case of one cover per visible
/// row) and by per-cell `providerImageBytes` calls (cache hits
/// after the album is in the cache) when the user scrolls.
const PREWARM_SAMPLE = 24;

export function useAlbumArtPrewarm(): void {
  const albums = useLibraryStore((s) => s.albums);
  const loaded = useLibraryStore((s) => s.loaded);

  useEffect(() => {
    if (!loaded) return;

    // Collect distinct (item_id, tag) pairs. Track covers are no
    // longer prewarmed (see module comment); only the album page's
    // first 24 covers hit the bulk.
    const seen = new Set<string>();
    const targets: AlbumArtRequest[] = [];

    const push = (itemId: string | undefined, tag: string | null | undefined) => {
      if (!itemId) return;
      const cacheKey = `${itemId}|${tag ?? ""}`;
      if (seen.has(cacheKey)) return;
      seen.add(cacheKey);
      targets.push({ albumId: itemId, tag: tag ?? null });
    };

    for (const album of albums.slice(0, PREWARM_SAMPLE)) {
      push(album.imageRef?.itemId, album.imageRef?.tag);
    }

    // Skip any target that is already in the JS cache — the bulk
    // fetch is wasted work if the blob URL is sitting in memory.
    const uncached = targets.filter((t) => getCached(t.albumId, t.tag) === null);
    if (uncached.length === 0) return;

    let cancelled = false;
    void (async () => {
      try {
        const res = await providerImageBytesBulk(uncached);
        if (cancelled) return;
        for (const image of res.images) {
          const url = buildBlobUrl(image.bytes, image.contentType);
          setCached(image.albumId, image.tag, url);
        }
      } catch (err) {
        console.warn("album-art bulk prewarm failed", err);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [loaded, albums]);
}
