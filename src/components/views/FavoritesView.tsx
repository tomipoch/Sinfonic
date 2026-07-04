// FavoritesView — stacked sections of favorited tracks, albums and
// artists. The track table mirrors `PlaylistDetailView` exactly
// (cover | song | album | time | favorite | menu) so the two lists
// feel like the same component.

import { useEffect, useState } from "react";
import { toast } from "sonner";

import { AlbumCard } from "@/components/ui/AlbumCard";
import { ArtistCard } from "@/components/ui/ArtistCard";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { getFavorites, playTrackWithContext } from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";
import type { Album, Artist, Track } from "@/types/domain";

const COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "song" },
  { kind: "album" },
  { kind: "time" },
  { kind: "favorite" },
  { kind: "menu" },
];

interface FavoritesData {
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
}

export function FavoritesView() {
  const activeServerId = useServerStore((s) => s.activeServerId);
  const [data, setData] = useState<FavoritesData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getFavorites();
      setData(result);
    } catch (e) {
      setError(extractError(e, "couldn't load favorites"));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (activeServerId) void load();
  }, [activeServerId]);

  if (!activeServerId) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">No server connected</div>
        <p className="text-sm text-muted-foreground">
          Connect a server in Settings to see favorites.
        </p>
      </div>
    );
  }

  if (loading) {
    return (
      <p className="p-6 text-sm text-muted-foreground" role="status">
        Loading favorites…
      </p>
    );
  }

  if (error) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
        <div className="text-base font-medium text-red-400">Failed to load favorites</div>
        <p className="text-sm text-red-300">{error}</p>
        <button type="button" onClick={() => void load()} className="btn-ghost text-sm">
          Retry
        </button>
      </div>
    );
  }

  if (!data) return null;

  const onPlayTrack = async (track: Track) => {
    try {
      // Anchor the auto-fill to favourites so the queue extends with
      // the remaining favourited tracks instead of restarting.
      await playTrackWithContext(track, {
        kind: "favorites",
        serverId: activeServerId,
      });
    } catch (e) {
      toast.error(`Couldn't play: ${extractError(e, "unknown error")}`);
    }
  };

  const totalCount = data.tracks.length + data.albums.length + data.artists.length;

  return (
    <section className="flex flex-col gap-8 p-6">
      <header className="flex flex-col gap-1">
        <h1 className="text-2xl font-semibold">Favorites</h1>
        <p className="text-sm text-muted-foreground">{totalCount} items across your library</p>
      </header>

      {/* Tracks */}
      {data.tracks.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Tracks
            </h2>
            <span className="text-xs text-muted-foreground">{data.tracks.length}</span>
          </div>
          <TrackTable
            tracks={data.tracks}
            columns={COLUMNS}
            onPlayTrack={onPlayTrack}
            dragSource="favorites-tracks"
          />
        </section>
      )}

      {/* Albums */}
      {data.albums.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Albums
            </h2>
            <span className="text-xs text-muted-foreground">{data.albums.length}</span>
          </div>
          <ul
            className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
            aria-label="Favorite albums"
          >
            {data.albums.map((album) => (
              <li key={album.id}>
                <AlbumCard album={album} />
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Artists */}
      {data.artists.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Artists
            </h2>
            <span className="text-xs text-muted-foreground">{data.artists.length}</span>
          </div>
          <ul
            className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
            aria-label="Favorite artists"
          >
            {data.artists.map((artist) => (
              <li key={artist.id}>
                <ArtistCard artist={artist} />
              </li>
            ))}
          </ul>
        </section>
      )}

      {totalCount === 0 && (
        <p className="text-sm text-muted-foreground">
          You haven't favorited anything yet. Tap the heart icon on any track, album, or artist to
          start.
        </p>
      )}
    </section>
  );
}
