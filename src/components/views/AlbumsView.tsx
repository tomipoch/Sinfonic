// AlbumsView — top-level /albums route. Grid of album cards.
// Real pagination via IntersectionObserver.
//
// Phase 5 of feature/direct-fetch-providers: triggers the
// album-art prewarm on mount so the visible page's covers are
// warmed via a single bulk fetch as soon as the route is visible,
// rather than landing each one as the grid cells mount.

import { useMemo } from "react";

import { AlbumCard } from "@/components/ui/AlbumCard";
import { EmptyState } from "@/components/ui/EmptyState";
import { useInfiniteScroll } from "@/hooks/useInfiniteScroll";
import { compareString } from "@/lib/sort";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";

export function AlbumsView() {
  const albums = useLibraryStore((s) => s.albums);
  const albumsTotal = useLibraryStore((s) => s.albumsTotal);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
  const loadingMore = useLibraryStore((s) => s.loadingMoreAlbums);
  const loadMoreAlbums = useLibraryStore((s) => s.loadMoreAlbums);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  // The shared `useAlbumArtPrewarm` hook fires as soon as the
  // library store has pages of albums + tracks, so mounting this
  // view implicitly triggers the warmup via the `Layout`-level
  // useEffect. Nothing extra needed here.

  const hasMore = albums.length < albumsTotal;
  const sentinelRef = useInfiniteScroll<HTMLDivElement>({
    onIntersect: () => {
      void loadMoreAlbums();
    },
    enabled: hasMore && !loadingMore && activeServerId !== null,
  });

  const sorted = useMemo(
    () => [...albums].sort((a, b) => compareString(a.title, b.title)),
    [albums],
  );

  if (!activeServerId) {
    return (
      <p className="p-6 text-sm text-muted-foreground">Connect a server to see your albums.</p>
    );
  }

  if (loading && albums.length === 0) {
    return (
      <p className="p-6 text-sm text-muted-foreground" role="status">
        Loading albums…
      </p>
    );
  }

  if (loaded && albums.length === 0) {
    return (
      <EmptyState
        title="No albums yet"
        description="Sync your library to populate this view."
        syncLabel="Sync library"
        syncing={lastSync === "syncing"}
        onSync={() => syncLibrary()}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4 p-6">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">Albums</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {albums.length} {albums.length === 1 ? "album" : "albums"}
          </p>
        </div>
      </header>

      <ul
        className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
        aria-label="Albums"
      >
        {sorted.map((album) => (
          <li key={album.id}>
            <AlbumCard album={album} />
          </li>
        ))}
      </ul>
      {hasMore && (
        <div
          ref={sentinelRef}
          aria-hidden
          className="flex h-12 items-center justify-center text-xs text-muted-foreground"
        >
          {loadingMore ? "Loading more…" : ""}
        </div>
      )}
      <p className="text-xs text-muted-foreground">
        Showing {albums.length} of {albumsTotal} albums
      </p>
    </div>
  );
}
