// ArtistsView — top-level /artists route. List of artists with
// album/track counts, a star for favorites, and a play button that
// queues the artist's tracks. Sorts alphabetically. Reads from the
// library cache populated by `useLibraryAutoLoad`.

import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { StarIcon, PlayIcon } from "@hugeicons/core-free-icons";
import { toast } from "sonner";

import { EmptyState } from "@/components/ui/EmptyState";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
import { usePlaybackStore } from "@/stores/playbackStore";
import { playAlbum, getAlbumDetail } from "@/lib/tauri";
import type { Artist, Track } from "@/types/domain";

type SortKey = "name" | "albumCount" | "trackCount";

const SORT_KEYS: readonly { key: SortKey; label: string }[] = [
  { key: "name", label: "Name" },
  { key: "albumCount", label: "Albums" },
  { key: "trackCount", label: "Tracks" },
];

function compareArtists(a: Artist, b: Artist, key: SortKey): number {
  if (key === "name") {
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  }
  return b[key] - a[key];
}

export function ArtistsView() {
  const artists = useLibraryStore((s) => s.artists);
  const tracks = useLibraryStore((s) => s.tracks);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
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

  const onPlayArtist = async (artist: Artist) => {
    setBusyId(artist.id);
    setBusy(true);
    try {
      // Prefer the cached track list when every track for the artist
      // is in the cache. Fall back to fetching the first album and
      // walking the rest one at a time so the queue still fills.
      const local = tracks.filter((t) => t.artistId === artist.id);
      if (local.length > 0) {
        await playAlbum(local);
        setIsPlaying(true);
        return;
      }
      const albums = useLibraryStore.getState().albums;
      const first = albums.find((a) => a.artistId === artist.id);
      if (!first) {
        toast.error("No albums found for this artist");
        return;
      }
      const detail = await getAlbumDetail(first.id);
      const firstBatch: Track[] = detail?.tracks ?? [];
      const remaining = albums.filter(
        (a) => a.artistId === artist.id && a.id !== first.id,
      );
      const tail: Track[] = [];
      for (const album of remaining) {
        const d = await getAlbumDetail(album.id);
        if (d?.tracks) tail.push(...d.tracks);
      }
      await playAlbum([...firstBatch, ...tail]);
      setIsPlaying(true);
    } catch (err) {
      toast.error(`Couldn't play artist: ${(err as Error).message ?? String(err)}`);
    } finally {
      setBusyId(null);
      setBusy(false);
    }
  };

  if (!activeServerId) {
    return (
      <p className="text-sm text-muted-foreground">
        Connect a server to see your artists.
      </p>
    );
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
            {artists.length} {artists.length === 1 ? "artist" : "artists"}
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
              <HugeiconsIcon
                icon={PlayIcon}
                size={14}
                strokeWidth={1.75}
              />
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
            <FavoriteButton
              kind="artist"
              itemId={artist.id}
              initialFavorite={artist.favorite}
            />
          </li>
        ))}
      </ul>
    </div>
  );
}
