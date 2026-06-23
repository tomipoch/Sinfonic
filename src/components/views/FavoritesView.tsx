// FavoritesView — tabbed view of favorited tracks / albums / artists.

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { getFavorites } from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";
import { formatDuration } from "@/lib/format";
import { encodeDragData } from "@/lib/queueDnD";
import { cn } from "@/lib/cn";
import type { Album, Artist, Track } from "@/types/domain";

type FavoritesTab = "tracks" | "albums" | "artists";

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
  const [tab, setTab] = useState<FavoritesTab>("tracks");
  const [draggingId, setDraggingId] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getFavorites();
      setData(result);
    } catch (e) {
      setError((e as Error).message ?? String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (activeServerId) void load();
  }, [activeServerId]);

  if (!activeServerId) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">No server connected</div>
        <p className="text-sm text-foreground-subtle">Connect a server in Settings to see favorites.</p>
      </div>
    );
  }

  if (loading) {
    return <p className="text-foreground-subtle text-sm" role="status">Loading favorites…</p>;
  }

  if (error) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
        <div className="text-base font-medium text-red-400">Failed to load favorites</div>
        <p className="text-sm text-red-300">{error}</p>
        <button type="button" onClick={() => void load()} className="btn-ghost text-sm">Retry</button>
      </div>
    );
  }

  if (!data) return null;

  const tabs: { key: FavoritesTab; label: string; count: number }[] = [
    { key: "tracks", label: "Tracks", count: data.tracks.length },
    { key: "albums", label: "Albums", count: data.albums.length },
    { key: "artists", label: "Artists", count: data.artists.length },
  ];

  return (
    <section className="flex flex-col gap-4 p-6">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="text-2xl font-semibold">Favorites</h1>
        <div className="flex gap-1 rounded-md border border-border bg-muted p-1">
          {tabs.map((t) => (
            <button
              key={t.key}
              type="button"
              onClick={() => setTab(t.key)}
              className={`rounded px-3 py-1 text-sm transition-colors ${
                tab === t.key ? "bg-card text-foreground" : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {t.label} <span className="ml-1 text-foreground-muted">({t.count})</span>
            </button>
          ))}
        </div>
      </header>

      {tab === "tracks" && (
        data.tracks.length === 0 ? (
          <EmptyState message="No favorited tracks yet." />
        ) : (
          <ol className="divide-y divide-border rounded-md border border-border">
            {data.tracks.map((track) => (
              <li
                key={track.id}
                draggable
                onDragStart={(e) => {
                  setDraggingId(track.id);
                  e.dataTransfer.setData("application/json", encodeDragData({ tracks: [track], source: "favorites" }));
                  e.dataTransfer.effectAllowed = "copy";
                }}
                onDragEnd={() => setDraggingId(null)}
                className={cn(
                  "grid grid-cols-[2.5rem_1fr_auto_auto] items-center gap-3 px-3 py-2 text-sm",
                  draggingId === track.id && "opacity-30",
                )}
              >
                <div className="text-right font-mono text-xs text-foreground-muted">{track.trackNumber || "—"}</div>
                <div className="min-w-0">
                  <div className="truncate font-medium text-foreground">{track.title}</div>
                  <div className="truncate text-xs text-foreground-subtle">{track.artist}</div>
                </div>
                <div className="text-xs text-foreground-muted">{formatDuration(track.durationSeconds)}</div>
                <FavoriteButton kind="track" itemId={track.id} initialFavorite={track.favorite} />
              </li>
            ))}
          </ol>
        )
      )}

      {tab === "albums" && (
        data.albums.length === 0 ? (
          <EmptyState message="No favorited albums yet." />
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-4">
            {data.albums.map((album) => (
              <Link
                key={album.id}
                to={`/library/album/${encodeURIComponent(album.id)}`}
                className="flex flex-col gap-2 rounded-md border border-border bg-muted p-4 transition-colors hover:border-primary/50 hover:bg-card"
              >
                <AlbumCover album={album} />
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-foreground">{album.title}</div>
                  <div className="truncate text-xs text-foreground-subtle">{album.artist}</div>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-xs text-foreground-muted">
                    {album.trackCount} tracks · {formatDuration(album.durationSeconds)}
                  </span>
                  <FavoriteButton kind="album" itemId={album.id} initialFavorite={album.favorite} />
                </div>
              </Link>
            ))}
          </div>
        )
      )}

      {tab === "artists" && (
        data.artists.length === 0 ? (
          <EmptyState message="No favorited artists yet." />
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-4">
            {data.artists.map((artist) => (
              <Link
                key={artist.id}
                to={`/library/artist/${encodeURIComponent(artist.id)}`}
                className="flex flex-col items-center gap-2 rounded-md border border-border bg-muted p-4 transition-colors hover:border-primary/50 hover:bg-card"
              >
                <div className="flex h-24 w-24 items-center justify-center rounded-full bg-card text-3xl font-bold text-white/40">
                  {artist.name.charAt(0).toUpperCase()}
                </div>
                <div className="text-center text-sm font-medium text-foreground">{artist.name}</div>
                <div className="text-xs text-foreground-muted">
                  {artist.albumCount} albums · {artist.trackCount} tracks
                </div>
                <FavoriteButton kind="artist" itemId={artist.id} initialFavorite={artist.favorite} />
              </Link>
            ))}
          </div>
        )
      )}
    </section>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
      <div className="text-base font-medium text-foreground">{message}</div>
    </div>
  );
}
