// useQueueBridge — keeps `useQueueStore` in sync with the backend.
//
// Two responsibilities:
//   1. Bootstrap on mount: fetch `get_queue` once so the store reflects
//      a queue that was persisted / restored before the UI mounted
//      (server-switch restore, prior-session hydration, etc.).
//   2. Subscribe to `queue-changed`: every backend mutation that
//      touches the queue (play_track, play_album, next, previous,
//      queue_add_many, jump_to, clear, set_shuffle, …) emits
//      `queue-changed` with the new snapshot. The store replaces the
//      queue fields in place and keeps `panelMode` untouched.
//
// This hook owns no UI; it just runs effects. It's mounted once by
// `<PlaybackProvider>` so the subscription survives navigation but
// never duplicates per consumer.
//
// On dev / test where the Tauri IPC isn't injected, the bootstrap
// call rejects silently — the store stays at its empty default.

import { useEffect } from "react";

import { useTauriEvent } from "@/hooks/useTauriEvent";
import { getQueue } from "@/lib/tauri";
import { useQueueStore } from "@/stores/queueStore";
import type { QueueSnapshotPayload } from "@/types/domain";

export function useQueueBridge(): void {
  // 1) Bootstrap from the backend.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const snap = await getQueue();
        if (cancelled) return;
        useQueueStore.setState((prev) => ({
          serverId: snap.serverId,
          entries: snap.entries,
          currentIndex: snap.currentIndex,
          repeat: snap.repeat,
          shuffle: snap.shuffle,
          shuffleSeed: snap.shuffleSeed,
          contextRemaining: null,
          panelMode: prev.panelMode,
        }));
      } catch {
        // Dev / test mode without Tauri runtime — leave defaults.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 2) Live updates from the backend.
  useTauriEvent<QueueSnapshotPayload>("queue-changed", (payload) => {
    useQueueStore.setState((prev) => ({
      serverId: prev.serverId,
      entries: payload.entries,
      currentIndex: payload.currentIndex,
      repeat: payload.repeat,
      shuffle: payload.shuffle,
      shuffleSeed: prev.shuffleSeed,
      contextRemaining:
        payload.contextRemaining === null || payload.contextRemaining === undefined
          ? null
          : payload.contextRemaining,
      panelMode: prev.panelMode,
    }));
  });
}
