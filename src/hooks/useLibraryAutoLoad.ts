// Auto-load library + playlists when the active server changes, and
// refresh both when a sync completes while the app is mounted.
//
// The library cache is server-scoped (`active_server_id`), so a
// disconnect must clear the in-memory copy and a connect must
// repopulate it. Centralising the effect in one hook keeps every
// tab consistent — they all subscribe to the same store snapshot.
//
// `library-sync-status` events are fired by the Rust side during
// login flows and explicit `provider_sync_library` calls; we watch
// for `state === "complete"` and re-fetch the cached lists so any
// view that was already mounted (Songs page, sidebar playlists)
// updates without needing a route change or server switch.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { safelyUnlisten } from "@/lib/tauriListen";
import { useLibraryStore } from "@/stores/libraryStore";
import { usePlaylistsStore } from "@/stores/playlistsStore";
import { useServerStore } from "@/stores/serverStore";
import type { SyncState } from "@/types/domain";
import { makeLogger } from "@/utils/log";

const log = makeLogger("useLibraryAutoLoad");

interface SyncPayload {
  serverId?: string | null;
  state: SyncState;
  progress: number;
}

export function useLibraryAutoLoad(): void {
  const activeServerId = useServerStore((s) => s.activeServerId);
  const loadAll = useLibraryStore((s) => s.loadAll);
  const reset = useLibraryStore((s) => s.reset);

  // Track the previous sync state so we only refetch on the
  // transition INTO "complete" (not on every event during the run).
  const previousSyncState = useRef<SyncState | null>(null);

  useEffect(() => {
    if (activeServerId) {
      log.log("activeServerId changed → loadAll()", activeServerId);
      void loadAll().catch((err) => log.error("loadAll failed", err));
    } else {
      log.log("activeServerId cleared → reset()");
      reset();
      // Also drop the cached playlists so a logout doesn't leak the
      // previous server's list into the sidebar on next login.
      usePlaylistsStore.setState({ playlists: [] });
    }
    // Reset the sync-state memory so the post-sync refetch doesn't
    // double-fire on the next activeServerId change.
    previousSyncState.current = null;
  }, [activeServerId, loadAll, reset]);

  // Refetch on sync completion: the backend emits a series of
  // `library-sync-status` events during a sync; the last one has
  // `state === "complete"`. We compare with the previous value so a
  // re-mount during the sync doesn't refetch twice.
  useEffect(() => {
    if (!activeServerId) return;
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    void listen<SyncPayload>("library-sync-status", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (!payload) return;
      const prev = previousSyncState.current;
      previousSyncState.current = payload.state;
      // Only refetch on the started → complete edge. Any other
      // intermediate states are ignored.
      if (payload.state === "complete" && prev !== "complete") {
        log.log("sync complete → refetch library + playlists");
        void loadAll().catch((err) => log.error("post-sync loadAll failed", err));
        void usePlaylistsStore
          .getState()
          .loadPlaylists()
          .catch((err) => log.error("post-sync loadPlaylists failed", err));
      }
    })
      .then((fn) => {
        if (cancelled) {
          safelyUnlisten(fn);
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => log.error("sync-status listen() rejected", err));

    return () => {
      cancelled = true;
      safelyUnlisten(unlisten);
    };
  }, [activeServerId, loadAll]);
}
