// Album detail view — header (cover + title + artist + meta + genre
// chips) and a track table. The "Play album" button replaces the
// queue with the album's tracks and starts playback from the first
// one.
//
// Tracks come from the local SQLite cache (server-scoped) so the
// view works offline after a sync. The `album_id` URL param is
// the same id we used in the grid; we round-trip it through
// `encodeURIComponent` so paths with special characters survive
// React Router.

import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { toast } from "sonner";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { GenreChip } from "@/components/ui/GenreChip";
import { MarqueeText } from "@/components/ui/MarqueeText";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { getAlbumDetail, playAlbumWithContext, playTrackWithContext } from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";
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
        toast.error(`Couldn't load album: ${extractError(err, "unknown error")}`);
        setLoading(false);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [id, activeServerId]);

  if (!id) {
    return <p className="p-6 text-muted-foreground text-sm">Missing album id.</p>;
  }

  if (loading) {
    return (
      <p className="p-6 text-muted-foreground text-sm" role="status">
        Loading album…
      </p>
    );
  }

  if (notFound || !data) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
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
      await playAlbumWithContext(tracks, { kind: "album", albumId: album.id });
    } catch (err) {
      toast.error(`Couldn't play album: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayTrack = async (track: Track) => {
    setBusy(true);
    try {
      await playTrackWithContext(track, { kind: "album", albumId: album.id });
    } catch (err) {
      toast.error(`Couldn't play track: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <div className="w-48 shrink-0">
          <AlbumCover source={album} />
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="text-3xl font-semibold">
            <MarqueeText>{album.title}</MarqueeText>
          </h1>
          <div className="text-base text-muted-foreground">
            {album.artist}
            {album.year ? ` · ${album.year}` : ""}
          </div>
          <div className="text-xs text-muted">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"}
            {" · "}
            {formatDuration(album.durationSeconds)}
          </div>
          {album.genres.length > 0 && (
            <div className="mt-1 flex flex-wrap gap-1.5">
              {album.genres.map((genre) => (
                <Link key={genre} to={`/genres/${encodeURIComponent(genre)}`}>
                  <GenreChip label={genre} />
                </Link>
              ))}
            </div>
          )}
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
