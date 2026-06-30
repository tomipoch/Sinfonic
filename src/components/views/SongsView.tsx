// SongsView — top-level /songs route. Full table of tracks with
// numeric pagination, column sort, a play button per row (hover on
// cover), drag-to-queue, and a "Play all" header button that
// replaces the queue with the visible page (see note below).
//
// Pagination contract:
//   - `PAGE_SIZE` tracks per page.
//   - `total` is whatever `get_tracks(offset, limit)` returns in its
//     PagedResponse — the SQLite cache is the source of truth.
//   - The "Play all" button plays only the visible page, matching
//     the previous (broken-but-known) behaviour. Playing every page
//     in order is left as a follow-up; it needs a separate "play all
//     library" decision because it crosses the queue ownership
//     boundary.

import { useEffect, useState } from "react";
import { toast } from "sonner";

import { EmptyState } from "@/components/ui/EmptyState";
import { Pagination } from "@/components/ui/Pagination";
import { PlayGlyph } from "@/components/ui/PlayGlyph";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { getTracks, playAlbum, playTrack } from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";
import type { Track } from "@/types/domain";

const COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "title" },
  { kind: "artist" },
  { kind: "album" },
  { kind: "time" },
  { kind: "favorite" },
  { kind: "menu" },
];

const SORTABLE: ("title" | "artist" | "album" | "durationSeconds")[] = [
  "title",
  "artist",
  "album",
  "durationSeconds",
];

const PAGE_SIZE = 200;

export function SongsView() {
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  const [page, setPage] = useState(0);
  const [items, setItems] = useState<Track[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);

  // Server-switch resets the pager to page 0. The fetch effect
  // (keyed on `[activeServerId, page]`) fires next and populates
  // `items` / `total`.
  useEffect(() => {
    setPage(0);
    if (!activeServerId) {
      setItems([]);
      setTotal(0);
      setLoaded(false);
    }
  }, [activeServerId]);

  useEffect(() => {
    if (!activeServerId) return;
    let cancelled = false;
    setLoading(true);
    getTracks(page * PAGE_SIZE, PAGE_SIZE)
      .then((resp) => {
        if (cancelled) return;
        setItems(resp.items);
        setTotal(resp.total);
        setLoaded(true);
      })
      .catch((err) => {
        if (cancelled) return;
        toast.error(`Couldn't load songs: ${extractError(err, "unknown error")}`);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeServerId, page]);

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  const onPlay = async (track: Track) => {
    setBusy(true);
    try {
      await playTrack(track);
    } catch (err) {
      toast.error(`Couldn't play track: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayAll = async () => {
    if (busy || items.length === 0) return;
    setBusy(true);
    try {
      await playAlbum(items);
    } catch (err) {
      toast.error(`Couldn't play all: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  if (!activeServerId) {
    return <p className="text-sm text-muted-foreground">Connect a server to see your songs.</p>;
  }

  if (loading && items.length === 0 && total === 0) {
    return (
      <p className="text-sm text-muted-foreground" role="status">
        Loading songs…
      </p>
    );
  }

  if (loaded && total === 0) {
    return (
      <EmptyState
        title="No songs yet"
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
          <h1 className="text-2xl font-semibold text-foreground">Songs</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {total} {total === 1 ? "track" : "tracks"}
            {totalPages > 1 && (
              <>
                {" · "}
                page {page + 1} of {totalPages}
              </>
            )}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void onPlayAll()}
          disabled={busy || items.length === 0}
          className="inline-flex items-center gap-2 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm transition-all hover:scale-105 hover:shadow-md hover:shadow-primary/20 disabled:hover:scale-100"
        >
          <PlayGlyph />
          Play all
        </button>
      </header>

      <TrackTable
        tracks={loading ? [] : items}
        columns={COLUMNS}
        onPlayTrack={onPlay}
        sortableColumns={SORTABLE}
        dragSource="songs-view"
      />

      <Pagination page={page} totalPages={totalPages} onChange={setPage} />
    </div>
  );
}
