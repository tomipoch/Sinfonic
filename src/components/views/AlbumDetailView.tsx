// Album detail view — header (cover + title + artist + meta) and a
// track table. The "Play album" button replaces the queue with the
// album's tracks and starts playback from the first one.
//
// Tracks come from the local SQLite cache (server-scoped) so the
// view works offline after a sync. The `album_id` URL param is
// the same id we used in the grid; we round-trip it through
// `encodeURIComponent` so paths with special characters survive
// React Router.

import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { TrackTable, type TrackColumn } from "@/components/ui/TrackTable";
import { getAlbumDetail, playAlbum, playTrack } from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";
import { formatDuration } from "@/lib/format";
import type { Album, Track } from "@/types/domain";

const COLUMNS: TrackColumn[] = [
  { kind: "index", mode: "track-number" },
  { kind: "cover" },
  { kind: "title" },
  { kind: "time" },
  { kind: "favorite" },
  { kind: "menu" },
];

interface AlbumDetailData {
  album: Album;
  tracks: Track[];
}

export function AlbumDetailView() {
  const { id } = useParams<{ id: string }>();
  const activeServerId = useServerStore((s) => s.activeServerId);

  const [data, setData] = useState<AlbumDetailData | null>(null);
  const [loading, setLoading] = useState(true);
  const [notFound, setNotFound] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (!id || !activeServerId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setNotFound(false);
    void getAlbumDetail(id).then(
      (result) => {
        if (cancelled) return;
        if (result === null) {
          setNotFound(true);
        } else {
          setData(result);
        }
        setLoading(false);
      },
      (err) => {
        if (cancelled) return;
        toast.error(`Couldn't load album: ${(err as Error).message ?? String(err)}`);
        setLoading(false);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [id, activeServerId]);

  if (!id) {
    return <p className="text-muted-foreground text-sm">Missing album id.</p>;
  }

  if (loading) {
    return (
      <p className="text-muted-foreground text-sm" role="status">
        Loading album…
      </p>
    );
  }

  if (notFound || !data) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">Album not found</div>
        <p className="text-sm text-muted-foreground">
          The library cache doesn't have this album. Try syncing your library.
        </p>
      </div>
    );
  }

  const { album, tracks } = data;

  const onPlayAlbum = async () => {
    if (busy || tracks.length === 0) return;
    setBusy(true);
    try {
      await playAlbum(tracks);
    } catch (err) {
      toast.error(`Couldn't play album: ${(err as Error).message ?? String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayTrack = async (track: Track) => {
    setBusy(true);
    try {
      await playTrack(track);
    } catch (err) {
      toast.error(`Couldn't play track: ${(err as Error).message ?? String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="flex flex-col gap-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <div className="w-48 shrink-0">
          <AlbumCover source={album} />
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          <div className="text-xs uppercase tracking-wide text-muted-foreground">Album</div>
          <h1 className="truncate text-3xl font-semibold">{album.title}</h1>
          <div className="text-base text-muted-foreground">
            {album.artist}
            {album.year ? ` · ${album.year}` : ""}
          </div>
          <div className="text-xs text-muted">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"}
            {" · "}
            {formatDuration(album.durationSeconds)}
          </div>
          <div className="mt-3 flex items-center gap-3">
            <button
              type="button"
              onClick={onPlayAlbum}
              disabled={busy || tracks.length === 0}
              className="btn-primary"
            >
              Play album
            </button>
            <FavoriteButton kind="album" itemId={album.id} initialFavorite={album.favorite} />
          </div>
        </div>
      </header>

      <TrackTable
        tracks={tracks}
        columns={COLUMNS}
        onPlayTrack={onPlayTrack}
        dragSource="album-detail"
      />
    </section>
  );
}