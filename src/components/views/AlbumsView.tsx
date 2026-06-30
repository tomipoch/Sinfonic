// AlbumsView — top-level /albums route. Grid of album cards with
// cover, title, artist, year, and a per-card Play button. Sorts
// alphabetically (case-insensitive). Reads from the library cache
// populated by `useLibraryAutoLoad`.
//
// P1: real pagination. An IntersectionObserver sentinel at the end
// of the grid triggers `loadMoreAlbums` until `albums.length`
// reaches `albumsTotal`. The `useInfiniteScroll` hook handles the
// observer; `loadingMoreAlbums` in the store guards re-entry.

import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { toast } from "sonner";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { EmptyState } from "@/components/ui/EmptyState";
import { PlayGlyph } from "@/components/ui/PlayGlyph";
import { useInfiniteScroll } from "@/hooks/useInfiniteScroll";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { compareNumberDesc, compareString } from "@/lib/sort";
import { playAlbum } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
import type { Album } from "@/types/domain";

type SortKey = "title" | "artist" | "year";

const SORT_KEYS: readonly { key: SortKey; label: string }[] = [
  { key: "title", label: "Title" },
  { key: "artist", label: "Artist" },
  { key: "year", label: "Year" },
];

function compareAlbums(a: Album, b: Album, key: SortKey): number {
  if (key === "year") {
    return compareNumberDesc(a.year, b.year);
  }
  return compareString(a[key], b[key]);
}

export function AlbumsView() {
  const albums = useLibraryStore((s) => s.albums);
  const tracks = useLibraryStore((s) => s.tracks);
  const albumsTotal = useLibraryStore((s) => s.albumsTotal);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
  const loadingMore = useLibraryStore((s) => s.loadingMoreAlbums);
  const loadMoreAlbums = useLibraryStore((s) => s.loadMoreAlbums);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  const [sortKey, setSortKey] = useState<SortKey>("title");
  const [busy, setBusy] = useState(false);

  const hasMore = albums.length < albumsTotal;
  const sentinelRef = useInfiniteScroll<HTMLDivElement>({
    onIntersect: useCallback(() => {
      void loadMoreAlbums();
    }, [loadMoreAlbums]),
    enabled: hasMore && !loadingMore && activeServerId !== null,
  });

  const sorted = useMemo(
    () => [...albums].sort((a, b) => compareAlbums(a, b, sortKey)),
    [albums, sortKey],
  );

  const onPlayAll = async () => {
    if (sorted.length === 0) return;
    setBusy(true);
    try {
      // `playAlbum` takes a list of tracks, not albums — translate
      // the visible album list to the track list using the cached
      // tracks (filtered to the currently sorted album ids so the
      // playback order matches the grid).
      const idSet = new Set(sorted.map((a) => a.id));
      const queue = tracks.filter((t) => idSet.has(t.albumId));
      if (queue.length === 0) {
        toast.error("No tracks cached for the visible albums yet. Try syncing.");
        return;
      }
      await playAlbum(queue);
    } catch (err) {
      toast.error(`Couldn't play all: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  if (!activeServerId) {
    return <p className="text-sm text-muted-foreground">Connect a server to see your albums.</p>;
  }

  if (loading && albums.length === 0) {
    return (
      <p className="text-sm text-muted-foreground" role="status">
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
        <div className="flex items-center gap-2">
          {SORT_KEYS.map((s) => (
            <button
              key={s.key}
              type="button"
              onClick={() => setSortKey(s.key)}
              className={
                "rounded-md px-2 py-1 text-xs font-medium transition-colors " +
                (sortKey === s.key
                  ? "bg-muted text-foreground"
                  : "text-muted-foreground hover:text-foreground")
              }
            >
              {s.label}
            </button>
          ))}
          <button
            type="button"
            onClick={() => void onPlayAll()}
            disabled={busy || sorted.length === 0}
            className="ml-2 inline-flex items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm transition-all hover:scale-105 hover:shadow-md hover:shadow-primary/20 disabled:hover:scale-100"
          >
            <PlayGlyph />
            Play all
          </button>
        </div>
      </header>

      <ul
        className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
        aria-label="Albums"
      >
        {sorted.map((album) => (
          <li key={album.id}>
            <Link
              to={`/albums/${encodeURIComponent(album.id)}`}
              className="group block focus:outline-none"
            >
              <AlbumCover source={album} />
              <div className="mt-2 truncate text-sm font-medium text-foreground group-hover:text-white">
                {album.title}
              </div>
              <div className="truncate text-xs text-muted-foreground">
                {album.artist}
                {album.year ? ` · ${album.year}` : ""}
              </div>
              <div className="text-xs text-muted-foreground/80">
                {album.trackCount} {album.trackCount === 1 ? "track" : "tracks"}
                {" · "}
                {formatDuration(album.durationSeconds)}
              </div>
            </Link>
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
