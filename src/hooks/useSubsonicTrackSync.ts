// useSubsonicTrackSync — subscribe to the per-batch `sync-progress`
// event and surface the album fan-out progress for Subsonic.
//
// Phase 3 of feature/direct-fetch-providers: Subsonic has no
// "list every track" endpoint, so the Subsonic provider runs a
// background fan-out of `getAlbum` for every album on the server
// and writes the resulting tracks into the SQLite cache. The
// backend emits a `sync-progress` event with `phase: "tracks"`
// after each album completes; this hook aggregates the latest
// batch into `{ active, done, total }` so the sidebar / Songs
// view can show progress.
//
// Also listens for `library-sync-status` to flip the `active`
// flag off when the run reports `complete` (or `error`) — the
// per-batch stream doesn't include a terminal event.
//
// Resets when `activeServerId` changes (server switch / logout)
// so a stale Subsonic sync from the previous server doesn't
// leak into the UI.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { safelyUnlisten } from "@/lib/tauriListen";
import { useServerStore } from "@/stores/serverStore";

export interface SubsonicTrackSync {
  /** True while a sync is running for the active server. */
  active: boolean;
  /** Album batches completed so far. */
  done: number;
  /** Total albums queued for this run. 0 before the first event. */
  total: number;
}

const INITIAL: SubsonicTrackSync = {
  active: false,
  done: 0,
  total: 0,
};

interface SyncProgressPayload {
  phase: string;
  done: number;
  total: number;
}

interface SyncStatusPayload {
  serverId?: string | null;
  state: string;
}

export function useSubsonicTrackSync(): SubsonicTrackSync {
  const [progress, setProgress] = useState<SubsonicTrackSync>(INITIAL);
  const activeServerId = useServerStore((s) => s.activeServerId);

  useEffect(() => {
    // Server switch / logout: reset so a stale batch from the
    // previous server doesn't show progress on the next one.
    setProgress(INITIAL);
  }, [activeServerId]);

  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    void listen<SyncProgressPayload>("sync-progress", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (!payload || payload.phase !== "tracks") return;
      setProgress({
        active: payload.done < payload.total,
        done: payload.done,
        total: payload.total,
      });
    })
      .then((fn) => {
        if (cancelled) safelyUnlisten(fn);
        else unlisteners.push(fn);
      })
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.warn("useSubsonicTrackSync: sync-progress listen() rejected", err);
      });

    void listen<SyncStatusPayload>("library-sync-status", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (!payload) return;
      // The Subsonic background sync emits "started" at the top
      // and "complete" / "error" at the end. We only care about
      // those terminal transitions here — the per-batch stream
      // covers the in-between.
      if (payload.state === "complete" || payload.state === "error") {
        setProgress((current) => (current.active ? { ...current, active: false } : current));
      }
    })
      .then((fn) => {
        if (cancelled) safelyUnlisten(fn);
        else unlisteners.push(fn);
      })
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.warn("useSubsonicTrackSync: library-sync-status listen() rejected", err);
      });

    return () => {
      cancelled = true;
      for (const fn of unlisteners) safelyUnlisten(fn);
    };
  }, []);

  return progress;
}
