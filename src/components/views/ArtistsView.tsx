// ArtistsView — top-level /artists route. List of artists with
// album/track counts, a star for favorites, and a play button that
// queues the artist's tracks. Sorts alphabetically. Reads from the
// library cache populated by `useLibraryAutoLoad`.
//
// P1: real pagination via `useInfiniteScroll` sentinel at the end
// of the list. Also: the "Play artist" branch parallelises album
// fetches with `Promise.all` instead of sequentially awaiting each
// one (was O(N) IPC round-trips for any artist with ≥3 albums).

import { PlayIcon, StarIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { toast } from "sonner";

import { EmptyState } from "@/components/ui/EmptyState";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { useInfiniteScroll } from "@/hooks/useInfiniteScroll";
import { extractError } from "@/lib/errors";
import { compareNumberDesc, compareString } from "@/lib/sort";
import { getAlbumDetail, playAlbum } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import { usePlaybackStore } from "@/stores/playbackStore";
import { useServerStore } from "@/stores/serverStore";
import type { Artist } from "@/types/domain";

type SortKey = "name" | "albumCount" | "trackCount";

const SORT_KEYS: readonly { key: SortKey; label: string }[] = [
  { key: "name", label: "Name" },
  { key: "albumCount", label: "Albums" },
  { key: "trackCount", label: "Tracks" },
];

function compareArtists(a: Artist, b: Artist, key: SortKey): number {
  if (key === "name") {
    return compareString(a.name, b.name);
  }
  return compareNumberDesc(a[key], b[key]);
}

export function ArtistsView() {
  const artists = useLibraryStore((s) => s.artists);
  const artistsTotal = useLibraryStore((s) => s.artistsTotal);
  const tracks = useLibraryStore((s) => s.tracks);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
  const loadingMore = useLibraryStore((s) => s.loadingMoreArtists);
  const loadMoreArtists = useLibraryStore((s) => s.loadMoreArtists);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);
  const setIsPlaying = usePlaybackStore((s) => s.setIsPlaying);

  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  const sorted = useMemo(
    () => [...artists].sort((a, b) => compareArtists(a, b, sortKey)),
    [artists, sortKey],
  );

  const hasMore = artists.length < artistsTotal;
  const sentinelRef = useInfiniteScroll<HTMLDivElement>({
    onIntersect: useCallback(() => {
      void loadMoreArtists();
    }, [loadMoreArtists]),
    enabled: hasMore && !loadingMore && activeServerId !== null,
  });

  const onPlayArtist = async (artist: Artist) => {
    setBusyId(artist.id);
    setBusy(true);
    try {
      // Prefer the cached track list when the artist is fully
      // covered by the library cache. Otherwise fan out the album
      // fetches in parallel — was an O(N) sequential await loop
      // before P1 and could take many seconds for prolific artists.
      const local = tracks.filter((t) => t.artistId === artist.id);
      if (local.length > 0) {
        await playAlbum(local);
        setIsPlaying(true);
        return;
      }
      const albums = useLibraryStore.getState().albums;
      const artistAlbums = albums.filter((a) => a.artistId === artist.id);
      if (artistAlbums.length === 0) {
        toast.error("No albums found for this artist");
        return;
      }
      const details = await Promise.all(artistAlbums.map((a) => getAlbumDetail(a.id)));
      const allTracks = details.flatMap((d) => d?.tracks ?? []);
      if (allTracks.length === 0) {
        toast.error("No tracks found for this artist");
        return;
      }
      await playAlbum(allTracks);
      setIsPlaying(true);
    } catch (err) {
      toast.error(`Couldn't play artist: ${extractError(err, "unknown error")}`);
    } finally {
      setBusyId(null);
      setBusy(false);
    }
  };

  if (!activeServerId) {
    return <p className="text-sm text-muted-foreground">Connect a server to see your artists.</p>;
  }

  if (loading && artists.length === 0) {
    return (
      <p className="text-sm text-muted-foreground" role="status">
        Loading artists…
      </p>
    );
  }

  if (loaded && artists.length === 0) {
    return (
      <EmptyState
        title="No artists yet"
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
          <h1 className="text-2xl font-semibold text-foreground">Artists</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {hasMore ? `${artists.length} of ${artistsTotal} artists` : `${artists.length} artists`}
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
        </div>
      </header>

      <ul
        className="divide-y divide-border overflow-hidden rounded-md border border-border"
        aria-label="Artists"
      >
        {sorted.map((artist) => (
          <li
            key={artist.id}
            className="flex items-center gap-3 px-3 py-2 transition-colors hover:bg-muted"
          >
            <button
              type="button"
              onClick={() => void onPlayArtist(artist)}
              disabled={busy && busyId !== artist.id}
              aria-label={`Play ${artist.name}`}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-card hover:text-foreground disabled:opacity-50"
            >
              <HugeiconsIcon icon={PlayIcon} size={14} strokeWidth={1.75} />
            </button>
            <Link
              to={`/artists/${encodeURIComponent(artist.id)}`}
              className="flex min-w-0 flex-1 items-center justify-between gap-3 focus:outline-none"
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2 truncate text-sm font-medium text-foreground">
                  {artist.name}
                  {artist.favorite && (
                    <HugeiconsIcon
                      icon={StarIcon}
                      size={12}
                      strokeWidth={2}
                      className="shrink-0 text-primary"
                    />
                  )}
                </div>
                <div className="truncate text-xs text-muted-foreground">
                  {artist.albumCount} {artist.albumCount === 1 ? "album" : "albums"}
                  {" · "}
                  {artist.trackCount} {artist.trackCount === 1 ? "track" : "tracks"}
                </div>
              </div>
            </Link>
            <FavoriteButton kind="artist" itemId={artist.id} initialFavorite={artist.favorite} />
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
        Showing {artists.length} of {artistsTotal} artists
      </p>
    </div>
  );
}
