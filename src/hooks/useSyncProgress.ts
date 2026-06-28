// useSyncProgress — subscribe to the `library-sync-status` Tauri event
// and surface its state as React-friendly values.
//
// The backend emits this event during login (jellyfin_login, subsonic_login,
// local_login) and during explicit `provider_sync_library` calls. Each
// payload carries the active `server_id`, a coarse `state` string
// (`preparing` / `started` / `scanning` / `indexing` / `complete`),
// and a `progress` float in `[0.0, 1.0]`.
//
// One subscription per mount; the listener is torn down with the hook.
// Multiple components can subscribe independently and share no state —
// the Rust side fans out events to every active subscriber.
//
// The returned `ready` flag flips to `true` once Tauri's `listen()`
// promise has resolved, so callers that want to fire a sync from a
// route handler can wait for it before kicking off any IPC work —
// otherwise the very first event could race the listener registration
// and be lost.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { safelyUnlisten } from "@/lib/tauriListen";
import type { SyncState } from "@/types/domain";
import { makeLogger } from "@/utils/log";

export type { SyncState };

const log = makeLogger("useSyncProgress");

/**
 * Re-exported from `@/types/domain` so existing callers that import
 * `SyncState` from this hook keep working — `useSyncProgress` was
 * the original source until `LibrarySyncStatus` adopted the same
 * closed union.
 */

export interface SyncPayload {
  serverId?: string | null;
  state: SyncState;
  progress: number;
}

export interface SyncProgress {
  /** Coarse state string from the backend. */
  state: SyncState;
  /** Normalised progress in `[0.0, 1.0]`. */
  progress: number;
  /** `true` once we've seen at least one event from this run. */
  active: boolean;
  /** `true` after the backend reports `complete`. */
  done: boolean;
  /** `server_id` of the server being synced, when known. */
  serverId: string | null;
  /** Last error message encountered while subscribing or listening. */
  error: string | null;
  /**
   * `true` once the `listen()` registration has resolved on the
   * Tauri side. Until this flips, events fired by the backend could
   * race the listener and be lost; callers that kick off a sync
   * should wait for it.
   */
  ready: boolean;
}

const INITIAL: SyncProgress = {
  state: "preparing",
  progress: 0,
  active: false,
  done: false,
  serverId: null,
  error: null,
  ready: false,
};

interface Options {
  /**
   * When provided, the hook resets to `INITIAL` on every change so a
   * new run can be tracked from scratch. Useful when the consumer
   * knows a sync has just been kicked off.
   */
  resetKey?: string | number;
}

export function useSyncProgress({ resetKey }: Options = {}): SyncProgress {
  const [progress, setProgress] = useState<SyncProgress>(INITIAL);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    void listen<SyncPayload>("library-sync-status", (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (!payload) return;
      const clamped = Math.max(0, Math.min(1, payload.progress));
      const done = payload.state === "complete";
      log.log("event", {
        state: payload.state,
        progress: payload.progress,
        done,
        serverId: payload.serverId ?? null,
      });
      setProgress({
        state: payload.state,
        progress: done ? 1 : clamped,
        active: !done,
        done,
        serverId: payload.serverId ?? null,
        error: null,
        ready: true,
      });
    })
      .then((fn) => {
        if (cancelled) {
          // The component unmounted before the listen promise
          // resolved — clean up immediately. safelyUnlisten handles
          // both sync throws and the async rejection Tauri throws
          // when the JS context is being torn down.
          safelyUnlisten(fn);
        } else {
          unlisten = fn;
          log.log("ready: listen() resolved");
          // Flip ready even if no event has fired yet, so callers
          // can kick off work without waiting for the first tick.
          setProgress((current) => (current.ready ? current : { ...current, ready: true }));
        }
      })
      .catch((err) => {
        log.error("listen() rejected", err);
        if (cancelled) return;
        setProgress((current) => ({
          ...current,
          ready: true,
          error: `Couldn't listen for sync progress: ${String(err)}`,
        }));
      });

    return () => {
      cancelled = true;
      safelyUnlisten(unlisten);
    };
  }, []);

  // Optional reset when the consumer knows a new sync is starting.
  useEffect(() => {
    if (resetKey === undefined) return;
    setProgress(INITIAL);
  }, [resetKey]);

  return progress;
}
