// Pre-warm the album-art cache on the active server's first page
// of albums. The cache lookup is already keyed by
// `(provider, image_id, tag)` so this is effectively a no-op for
// albums whose bytes are already on disk — it just primes the
// filesystem for the first 24 covers the user is likely to see.
//
// Fire-and-forget: errors are logged but never surfaced. The
// AlbumCover component handles a missing cache hit on its own by
// falling back to the gradient, so a partial pre-warm is invisible
// to the user.

import { useEffect } from "react";

import { providerImageBytes } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";

const PREWARM_LIMIT = 24;

export function useAlbumArtPrewarm(): void {
  const albums = useLibraryStore((s) => s.albums);
  const loaded = useLibraryStore((s) => s.loaded);

  useEffect(() => {
    if (!loaded) return;
    const targets = albums
      .filter((album) => album.imageRef?.itemId)
      .slice(0, PREWARM_LIMIT);
    if (targets.length === 0) return;

    let cancelled = false;
    void (async () => {
      for (const album of targets) {
        if (cancelled) return;
        const itemId = album.imageRef?.itemId;
        if (!itemId) continue;
        try {
          await providerImageBytes(itemId, album.imageRef?.tag ?? null);
        } catch (err) {
          console.warn("album-art prewarm failed", album.id, err);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [loaded, albums]);
}
