// useAlbumLookup — resolve a `Map<id, Album>` that includes albums
// not in the first page of `get_albums`.
//
// The library store only loads the first `PAGE_SIZE` albums and
// tracks. Views that need a track's parent album (SongsView,
// prewarm, PlayerBar) frequently hit tracks whose album is on a
// later page, so a plain `albumById` map misses them.
//
// `ensureLoaded(id)` is a fire-and-forget fetch: it's a no-op when
// the id is already in the store, already cached as an extra, or
// currently in flight. Consumers pass the returned `albumById` to
// the existing `useMemo` lookup; missing ids show the gradient
// placeholder while the fetch resolves, then re-render with the
// real cover on the next tick.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { getAlbum } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import type { Album } from "@/types/domain";

export interface AlbumLookup {
  albumById: ReadonlyMap<string, Album>;
  ensureLoaded: (albumId: string) => void;
}

export function useAlbumLookup(): AlbumLookup {
  const albums = useLibraryStore((s) => s.albums);
  const [extras, setExtras] = useState<ReadonlyMap<string, Album>>(
    () => new Map(),
  );

  const storeIds = useRef<Set<string>>(new Set());
  useEffect(() => {
    storeIds.current = new Set(albums.map((a) => a.id));
  }, [albums]);

  const inflight = useRef<Set<string>>(new Set());

  const ensureLoaded = useCallback((albumId: string) => {
    if (!albumId) return;
    if (storeIds.current.has(albumId)) return;
    if (extras.has(albumId)) return;
    if (inflight.current.has(albumId)) return;
    inflight.current.add(albumId);
    void getAlbum(albumId)
      .then((album) => {
        inflight.current.delete(albumId);
        if (album) {
          setExtras((current) => {
            if (current.has(albumId)) return current;
            const next = new Map(current);
            next.set(albumId, album);
            return next;
          });
        }
      })
      .catch(() => {
        inflight.current.delete(albumId);
      });
  }, [extras]);

  useEffect(() => {
    return () => {
      inflight.current.clear();
    };
  }, []);

  const albumById = useMemo(() => {
    const map = new Map<string, Album>();
    for (const a of albums) map.set(a.id, a);
    for (const [k, v] of extras) map.set(k, v);
    return map;
  }, [albums, extras]);

  return { albumById, ensureLoaded };
}
