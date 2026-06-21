// Global event bridge — single subscriber that pipes Tauri playback
// events into the Zustand stores. Mounted once at the app root so
// every component sees the same live state.
//
// Bootstrap on mount: seed the stores from `get_playback_state` and
// `get_queue` so a hot-reload during dev or a tab restore after the
// backend already started a sink reflects the real state instead
// of the empty defaults.

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { getPlaybackState, getQueue } from "../lib/tauri";
import { usePlaybackStore } from "../stores/playbackStore";
import { useQueueStore } from "../stores/queueStore";
import type {
  PlaybackStatePayload,
  QueueSnapshotPayload,
  TrackChangedPayload,
} from "../types/domain";

export function usePlaybackEvents(): void {
  useEffect(() => {
    let cancelled = false;
    let unsubs: UnlistenFn[] = [];

    void Promise.all([getPlaybackState(), getQueue()])
      .then(([pb, queue]) => {
        if (cancelled) return;
        usePlaybackStore.getState().setState(pb);
        useQueueStore.setState({
          serverId: queue.serverId,
          entries: queue.entries,
          currentIndex: queue.currentIndex,
          repeat: queue.repeat,
          shuffle: queue.shuffle,
          shuffleSeed: queue.shuffleSeed,
        });
      })
      .catch(() => {
        // Backend may not be ready yet (e.g., during dev HMR). The
        // next event from the backend will catch the stores up.
      });

    void Promise.all([
      listen<PlaybackStatePayload>("playback-state-changed", (e) => {
        usePlaybackStore.getState().setState(e.payload);
      }),
      listen<TrackChangedPayload>("track-changed", (e) => {
        usePlaybackStore.getState().setTrack(e.payload);
      }),
      listen<QueueSnapshotPayload>("queue-changed", (e) => {
        useQueueStore.setState({
          entries: e.payload.entries,
          currentIndex: e.payload.currentIndex,
          repeat: e.payload.repeat,
          shuffle: e.payload.shuffle,
        });
      }),
    ]).then((arr) => {
      if (cancelled) {
        arr.forEach((u) => u());
      } else {
        unsubs = arr;
      }
    });

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
    };
  }, []);
}
