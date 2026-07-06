// SongsView — top-level /songs route. Full table of tracks with
// numeric pagination, column sort, a play button per row (hover on
// cover), drag-to-queue, and a "Play all" header button that
// replaces the queue with the visible page (see note below).
//
// Phase 3 of feature/direct-fetch-providers:
//   - For Subsonic, tracks come from the SQLite cache populated by
//     the background sync (kick_subsonic_background_sync). While
//     the sync is running the cache is partial — `providerListTracks`
//     returns whatever's been written so far.
//   - We subscribe to `library-sync-status` and trigger a re-fetch
//     when the sync reports `complete` so the view doesn't stay
//     stuck on "Loading…" after the cache warms.
//   - While the cache is empty AND a sync is running, we render the
//     SubsonicSyncIndicator + a "Sincronizando canciones…" message
//     instead of the empty state so the user knows data IS on the
//     way.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { SubsonicSyncIndicator } from "@/components/layout/SubsonicSyncIndicator";
import { EmptyState } from "@/components/ui/EmptyState";
import { Pagination } from "@/components/ui/Pagination";
import { PlayGlyph } from "@/components/ui/PlayGlyph";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { playAlbumWithContext, playTrackWithContext, providerListTracks } from "@/lib/tauri";
import { safelyUnlisten } from "@/lib/tauriListen";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
import type { SyncState, Track } from "@/types/domain";

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

interface SyncStatusPayload {
  serverId?: string | null;
  state: SyncState;
}

export function SongsView() {
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);
  // The "tracks synced via background sync" counter drives the
  // SongView so the loading UI stays out of the user's way even
  // when the Subsonic sync produces partial results during warm-up.
  const syncedTracksCount = useLibraryStore((s) => s.tracks.length);

  const [page, setPage] = useState(0);
  const [items, setItems] = useState<Track[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [syncInProgress, setSyncInProgress] = useState(false);

  // Server-switch resets the pager to page 0. The fetch effect
  // (keyed on `[activeServerId, page]`) fires next and populates
  // `items` / `total`.
  useEffect(() => {
    setPage(0);
    if (!activeServerId) {
      setItems([]);
      setTotal(0);
      setLoaded(false);
      setSyncInProgress(false);
    }
  }, [activeServerId]);

  // Subsonic background sync listener: when sync reports
  // `complete` for the active server, force a re-fetch so the
  // freshly-warmed cache shows up immediately. Without this the
  // user would have to navigate away + back to see the new tracks.
  useEffect(() => {
    if (!activeServerId) return;
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    void listen<SyncStatusPayload>("library-sync-status", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (!payload) return;
      if (payload.serverId && payload.serverId !== activeServerId) return;
      if (payload.state === "started" || payload.state === "preparing") {
        setSyncInProgress(true);
      } else if (payload.state === "complete" || payload.state === "error") {
        setSyncInProgress(false);
        // Trigger a re-fetch so the next page renders the
        // freshly-cached tracks. `page` is in the dep list of
        // the fetch effect below, so bumping a counter is the
        // cheapest way to nudge it.
        if (payload.state === "complete") {
          setPage((current) => current);
        }
      }
    })
      .then((fn) => {
        if (cancelled) safelyUnlisten(fn);
        else unlisten = fn;
      })
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.warn("SongsView: sync-status listen() rejected", err);
      });

    return () => {
      cancelled = true;
      safelyUnlisten(unlisten);
    };
  }, [activeServerId]);

  useEffect(() => {
    if (!activeServerId) return;
    let cancelled = false;
    setLoading(true);
    providerListTracks(page * PAGE_SIZE, PAGE_SIZE)
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
    if (!activeServerId) return;
    setBusy(true);
    try {
      // Anchor the auto-fill to the entire library so the queue
      // extends with the next 30 tracks after this one in the
      // title-ascending order (the same order SongsView uses).
      // The backend preserves the previous history — the track
      // that was playing becomes part of History instead of being
      // wiped.
      await playTrackWithContext(track, {
        kind: "all",
        serverId: activeServerId,
      });
    } catch (err) {
      toast.error(`Couldn't play track: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayAll = async () => {
    if (busy || items.length === 0 || !activeServerId) return;
    setBusy(true);
    try {
      await playAlbumWithContext(items, {
        kind: "all",
        serverId: activeServerId,
      });
    } catch (err) {
      toast.error(`Couldn't play all: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  if (!activeServerId) {
    return <p className="text-sm text-muted-foreground">Connect a server to see your songs.</p>;
  }

  // Show "syncing…" instead of the empty state when the Subsonic
  // background sync is still running and the local cache is empty.
  // Once the sync reports `complete` the listener above flips
  // `syncInProgress` false and the fetch effect re-populates.
  if (loading && items.length === 0 && total === 0 && syncInProgress) {
    return (
      <div className="flex flex-col gap-4" role="status">
        <SubsonicSyncIndicator />
        <p className="text-sm text-muted-foreground">
          Sincronizando canciones…{syncedTracksCount > 0 ? ` (${syncedTracksCount} ya listas)` : ""}
        </p>
      </div>
    );
  }

  if (loading && items.length === 0 && total === 0) {
    return (
      <div className="flex flex-col gap-4" role="status">
        <SubsonicSyncIndicator />
        <p className="text-sm text-muted-foreground">Loading songs…</p>
      </div>
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
            {syncInProgress && (
              <span className="ml-2 text-xs text-muted-foreground/70">
                (background sync running)
              </span>
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
