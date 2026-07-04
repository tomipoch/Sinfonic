// Artist detail view — header (square photo + name + meta +
// action buttons) and three stacked sections:
//
//   1. Top Songs   — the 15 tracks by this artist sorted by album
//                    release year (descending). Phase 2 will swap
//                    this for a real popularity ranking via
//                    Last.fm `artist.getTopTracks`.
//   2. Albums      — every album where the artist is the primary
//                    artist, newest release first.
//   3. Also appears in — tracks credited to this artist that live
//                    on albums belonging to other artists (the
//                    cache's `tracks.artist_id` joins to
//                    `albums.artist_id` of a *different* artist).
//
// Reads from the local SQLite cache so it works offline; the same
// `useLibraryAutoLoad` hook that powers the grid already populated
// it.

import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import {
  MoreHorizontalIcon,
  PlayIcon,
  ShuffleIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

import { AlbumCard } from "@/components/ui/AlbumCard";
import { AlbumCover } from "@/components/ui/AlbumCover";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import {
  playAlbumWithContext,
  playTrackWithContext,
  setShuffle,
} from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import type { Track } from "@/types/domain";

const TOP_SONGS_LIMIT = 15;

const TOP_SONGS_COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "title" },
  { kind: "album" },
  { kind: "time" },
  { kind: "favorite" },
  { kind: "menu" },
];

const FEATURED_COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "title" },
  { kind: "artist" },
  { kind: "album" },
  { kind: "time" },
  { kind: "favorite" },
  { kind: "menu" },
];

export function ArtistDetailView() {
  const { id } = useParams<{ id: string }>();
  const artists = useLibraryStore((s) => s.artists);
  const albums = useLibraryStore((s) => s.albums);
  const tracks = useLibraryStore((s) => s.tracks);
  const [busy, setBusy] = useState(false);

  const artist = useMemo(
    () => (id ? (artists.find((a) => a.id === id) ?? null) : null),
    [artists, id],
  );

  // Map of album id → artist id. Used to detect guest appearances:
  // a track is "featured" when its `artist_id` matches the current
  // artist but the album it lives on belongs to a different one.
  const albumArtistById = useMemo(() => {
    const map = new Map<string, string | undefined>();
    for (const a of albums) {
      map.set(a.id, a.artistId ?? undefined);
    }
    return map;
  }, [albums]);

  // Album year lookup for the Top Songs sort. Tracks don't carry
  // their album's year so we resolve it here. Year is `number | null
  // | undefined` — treat anything missing as 0 so the sort still
  // produces a stable order.
  const albumYearById = useMemo(() => {
    const map = new Map<string, number>();
    for (const a of albums) {
      if (a.year != null) map.set(a.id, a.year);
    }
    return map;
  }, [albums]);

  const tracksByArtist = useMemo(
    () => (id ? tracks.filter((t) => t.artistId === id) : []),
    [tracks, id],
  );

  const topSongs = useMemo(() => {
    return [...tracksByArtist]
      .sort((a, b) => {
        const ya = albumYearById.get(a.albumId) ?? 0;
        const yb = albumYearById.get(b.albumId) ?? 0;
        if (ya !== yb) return yb - ya;
        if (a.discNumber !== b.discNumber) return a.discNumber - b.discNumber;
        return a.trackNumber - b.trackNumber;
      })
      .slice(0, TOP_SONGS_LIMIT);
  }, [tracksByArtist, albumYearById]);

  const artistAlbums = useMemo(() => {
    if (!id) return [];
    return albums
      .filter((a) => a.artistId === id)
      .sort((a, b) => {
        const ya = a.year ?? 0;
        const yb = b.year ?? 0;
        if (ya !== yb) return yb - ya;
        return a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
      });
  }, [albums, id]);

  const featuredTracks = useMemo(() => {
    if (!id) return [];
    return tracksByArtist.filter((t) => {
      const albumArtistId = albumArtistById.get(t.albumId);
      // Only show tracks whose album belongs to a *different* artist
      // and where the album actually exists in the cache (albumArtistId
      // may be undefined for albums not yet synced).
      return albumArtistId !== undefined && albumArtistId !== id;
    });
  }, [tracksByArtist, albumArtistById, id]);

  if (!id) {
    return <p className="p-6 text-sm text-muted-foreground">Missing artist id.</p>;
  }

  if (!artist) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">Artist not found</div>
        <p className="text-sm text-muted-foreground">
          The library cache doesn't have this artist. Try syncing your library.
        </p>
      </div>
    );
  }

  const onPlayAll = async () => {
    if (busy || topSongs.length === 0) return;
    setBusy(true);
    try {
      await playAlbumWithContext(topSongs, null);
    } catch (err) {
      toast.error(`Couldn't play: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onShuffle = async () => {
    if (busy || topSongs.length === 0) return;
    setBusy(true);
    try {
      await setShuffle(true);
      await playAlbumWithContext(topSongs, null);
    } catch (err) {
      toast.error(`Couldn't shuffle: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayTrack = async (track: Track) => {
    setBusy(true);
    try {
      await playTrackWithContext(track, null);
    } catch (err) {
      toast.error(`Couldn't play: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const actionsDisabled = busy || artist.trackCount === 0;

  return (
    <section className="flex flex-col gap-8 p-6">
      {/* ─── Header ─────────────────────────────────────────── */}
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <div className="w-48 shrink-0">
          <AlbumCover
            source={artist}
            initial={artist.name.charAt(0).toUpperCase()}
            className="aspect-square w-full rounded-2xl shadow-md ring-1 ring-inset ring-white/5"
            ariaLabel={`Photo of ${artist.name}`}
          />
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="text-3xl font-semibold">{artist.name}</h1>
          <div className="text-sm text-muted-foreground">
            {artist.albumCount} {artist.albumCount === 1 ? "album" : "albums"}
            {" · "}
            {artist.trackCount} {artist.trackCount === 1 ? "track" : "tracks"}
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <button
              type="button"
              onClick={() => void onPlayAll()}
              disabled={actionsDisabled}
              className="btn-primary"
            >
              <HugeiconsIcon icon={PlayIcon} size={16} strokeWidth={2} />
              Play
            </button>
            <button
              type="button"
              onClick={() => void onShuffle()}
              disabled={actionsDisabled}
              className="btn-ghost"
              aria-label="Shuffle"
              title="Shuffle"
            >
              <HugeiconsIcon icon={ShuffleIcon} size={16} strokeWidth={1.75} />
              Shuffle
            </button>
            <FavoriteButton
              kind="artist"
              itemId={artist.id}
              initialFavorite={artist.favorite}
            />
            <button
              type="button"
              className="rounded p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
              aria-label="More actions"
              title="More actions"
            >
              <HugeiconsIcon icon={MoreHorizontalIcon} size={16} strokeWidth={1.75} />
            </button>
          </div>
        </div>
      </header>

      {/* ─── Top Songs ───────────────────────────────────────── */}
      {topSongs.length > 0 && (
        <div className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Top Songs
            </h2>
            <span className="text-xs text-muted-foreground">
              {topSongs.length}
              {tracksByArtist.length > topSongs.length ? ` of ${tracksByArtist.length}` : ""}
            </span>
          </div>
          <TrackTable
            tracks={topSongs}
            columns={TOP_SONGS_COLUMNS}
            onPlayTrack={onPlayTrack}
            dragSource="artist-top"
          />
        </div>
      )}

      {/* ─── Albums (newest first) ───────────────────────────── */}
      {artistAlbums.length > 0 && (
        <div className="flex flex-col gap-3">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Albums
          </h2>
          <ul
            className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
            aria-label={`${artist.name} albums`}
          >
            {artistAlbums.map((album) => (
              <li key={album.id}>
                <AlbumCard album={album} />
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* ─── Also appears in ────────────────────────────────── */}
      {featuredTracks.length > 0 && (
        <div className="flex flex-col gap-3">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Also appears in
          </h2>
          <p className="text-xs text-muted-foreground">
            Tracks credited to {artist.name} on albums by other artists.
          </p>
          <TrackTable
            tracks={featuredTracks}
            columns={FEATURED_COLUMNS}
            onPlayTrack={onPlayTrack}
            dragSource="artist-featured"
          />
        </div>
      )}

      {topSongs.length === 0 && artistAlbums.length === 0 && (
        <div className="rounded-md border border-dashed border-border bg-muted/40 p-8 text-center text-sm text-muted-foreground">
          No tracks or albums cached for this artist yet.
        </div>
      )}
    </section>
  );
}