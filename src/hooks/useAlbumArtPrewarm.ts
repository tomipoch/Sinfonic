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
// Tracks that don't carry their own `imageRef` (Subsonic, mostly)
// fall back to the album row by `albumId`; the same lookup table
// that `AlbumCover` uses is updated as a side effect, so a track
// row whose album arrives on a later page still renders the right
// cover once `useAlbumLookup` resolves it.
//
// Fire-and-forget: errors are logged but never surfaced. AlbumCover
// falls back to the gradient on a missing hit, so a partial
// pre-warm is invisible to the user.

import { useEffect } from "react";

import {
  providerImageBytesBulk,
  type AlbumArtRequest,
} from "@/lib/tauri";
import { buildBlobUrl, getCached, setCached } from "@/lib/albumArtCache";
import { useAlbumLookup } from "@/hooks/useAlbumLookup";
import { useLibraryStore } from "@/stores/libraryStore";

export function useAlbumArtPrewarm(): void {
  const albums = useLibraryStore((s) => s.albums);
  const tracks = useLibraryStore((s) => s.tracks);
  const loaded = useLibraryStore((s) => s.loaded);
  const { albumById, ensureLoaded } = useAlbumLookup();

  useEffect(() => {
    if (!loaded) return;

    // Collect distinct (item_id, tag) pairs so duplicate refs across
    // albums / tracks only fetch once.
    const seen = new Set<string>();
    const targets: AlbumArtRequest[] = [];

    const push = (itemId: string | undefined, tag: string | null | undefined) => {
      if (!itemId) return;
      const cacheKey = `${itemId}|${tag ?? ""}`;
      if (seen.has(cacheKey)) return;
      seen.add(cacheKey);
      targets.push({ albumId: itemId, tag: tag ?? null });
    };

    // Cover every album in the current page. The library store only
    // holds the visible page so this is bounded; later pages get
    // warmed by their own component mounts.
    for (const album of albums) {
      push(album.imageRef?.itemId, album.imageRef?.tag);
    }

    // Resolve the first batch of tracks' parent albums through the
    // shared lookup. `ensureLoaded` is a no-op for albums that are
    // already in the store, and a fire-and-forget fetch for those
    // that aren't — we collect their ids and wait one tick before
    // firing the byte fetch.
    const trackSample = tracks.slice(0, 96);
    const pendingAlbumIds: string[] = [];
    for (const track of trackSample) {
      if (track.imageRef?.itemId) {
        push(track.imageRef.itemId, track.imageRef.tag);
      } else {
        if (!albumById.has(track.albumId)) {
          ensureLoaded(track.albumId);
          pendingAlbumIds.push(track.albumId);
        }
        const album = albumById.get(track.albumId);
        push(album?.imageRef?.itemId, album?.imageRef?.tag);
      }
    }

    // Skip any target that is already in the JS cache — the bulk
    // fetch is wasted work if the blob URL is sitting in memory.
    const uncached = targets.filter(
      (t) => getCached(t.albumId, t.tag) === null,
    );
    if (uncached.length === 0) return;

    let cancelled = false;
    void (async () => {
      if (pendingAlbumIds.length > 0) {
        // One tick for the lazy album lookups to land in the store
        // before we fire the byte fetch.
        await new Promise((r) => setTimeout(r, 0));
        if (cancelled) return;
      }
      try {
        const res = await providerImageBytesBulk(uncached);
        if (cancelled) return;
        // The bulk response carries the (albumId, tag) for each
        // resolved image so the cache write does not depend on
        // request/response ordering.
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
  }, [loaded, albums, tracks, albumById, ensureLoaded]);
}
