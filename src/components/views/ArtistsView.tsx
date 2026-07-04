// ArtistsView — top-level /artists route. List of artists with
// album/track counts, a star for favorites, and a play button that
// queues the artist's tracks. Sorts alphabetically by name. Reads
// from the library cache populated by `useLibraryAutoLoad`.
//
// Each row shows a small circular artist photo on the left, the
// artist name, then the track count prominently and the album count
// as a smaller follow-up.

import { PlayIcon, StarIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { toast } from "sonner";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { EmptyState } from "@/components/ui/EmptyState";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { MarqueeText } from "@/components/ui/MarqueeText";
import { useInfiniteScroll } from "@/hooks/useInfiniteScroll";
import { extractError } from "@/lib/errors";
import { compareString } from "@/lib/sort";
import { playAlbum, providerAlbumDetail } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
import type { Artist } from "@/types/domain";

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

  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  const sorted = useMemo(
    () => [...artists].sort((a, b) => compareString(a.name, b.name)),
    [artists],
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
      const local = tracks.filter((t) => t.artistId === artist.id);
      if (local.length > 0) {
        await playAlbum(local);
        return;
      }
      const cachedAlbums = useLibraryStore.getState().albums;
      const artistAlbums = cachedAlbums.filter((a) => a.artistId === artist.id);
      if (artistAlbums.length === 0) {
        toast.error("No albums found for this artist");
        return;
      }
      const details = await Promise.all(artistAlbums.map((a) => providerAlbumDetail(a.id)));
      const allTracks = details.flatMap((d) => d?.tracks ?? []);
      if (allTracks.length === 0) {
        toast.error("No tracks found for this artist");
        return;
      }
      await playAlbum(allTracks);
    } catch (err) {
      toast.error(`Couldn't play artist: ${extractError(err, "unknown error")}`);
    } finally {
      setBusyId(null);
      setBusy(false);
    }
  };

  if (!activeServerId) {
    return (
      <p className="p-6 text-sm text-muted-foreground">Connect a server to see your artists.</p>
    );
  }

  if (loading && artists.length === 0) {
    return (
      <p className="p-6 text-sm text-muted-foreground" role="status">
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
      <header>
        <h1 className="text-2xl font-semibold text-foreground">Artists</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {hasMore ? `${artists.length} of ${artistsTotal} artists` : `${artists.length} artists`}
        </p>
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
              className="group/artist relative h-10 w-10 shrink-0 overflow-hidden rounded-full disabled:opacity-50"
            >
              <AlbumCover
                source={artist}
                initial={artist.name.charAt(0).toUpperCase()}
                className="h-10 w-10 rounded-full shadow-none ring-0"
                ariaLabel={`Photo of ${artist.name}`}
              />
              <span
                aria-hidden
                className="absolute inset-0 flex items-center justify-center bg-black/40 text-primary-foreground opacity-0 transition-opacity group-hover/artist:opacity-100"
              >
                <HugeiconsIcon icon={PlayIcon} size={14} strokeWidth={2} />
              </span>
            </button>
            <Link
              to={`/artists/${encodeURIComponent(artist.id)}`}
              className="flex min-w-0 flex-1 items-center justify-between gap-3 focus:outline-none"
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2 text-sm font-medium text-foreground">
                  <MarqueeText>{artist.name}</MarqueeText>
                  {artist.favorite && (
                    <HugeiconsIcon
                      icon={StarIcon}
                      size={12}
                      strokeWidth={2}
                      className="shrink-0 text-primary"
                    />
                  )}
                </div>
                <div className="text-xs text-muted-foreground">
                  <span className="font-medium text-foreground/80">
                    {artist.trackCount} {artist.trackCount === 1 ? "track" : "tracks"}
                  </span>
                  <span className="mx-1.5">·</span>
                  {artist.albumCount} {artist.albumCount === 1 ? "album" : "albums"}
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
