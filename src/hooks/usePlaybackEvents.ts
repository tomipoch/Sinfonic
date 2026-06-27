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

import { getPlaybackState, getQueue } from "@/lib/tauri";
import { makeLogger } from "@/utils/log";
import { usePlaybackStore } from "@/stores/playbackStore";
import { useQueueStore } from "@/stores/queueStore";
import type {
  PlaybackStatePayload,
  QueueSnapshotPayload,
  TrackChangedPayload,
} from "@/types/domain";

const log = makeLogger("usePlaybackEvents");

export function usePlaybackEvents(): void {
  useEffect(() => {
    let cancelled = false;
    let unsubs: UnlistenFn[] = [];

    void Promise.all([getPlaybackState(), getQueue()])
      .then(([pb, queue]) => {
        if (cancelled) return;
        log.log("bootstrap: playback state loaded", {
          isPlaying: pb.isPlaying,
          volume: pb.volume,
          repeat: pb.repeat,
        });
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
      .catch((err) => {
        log.warn("bootstrap: backend not ready", err);
        // Backend may not be ready yet (e.g., during dev HMR). The
        // next event from the backend will catch the stores up.
      });

    void Promise.all([
      listen<PlaybackStatePayload>("playback-state-changed", (e) => {
        log.log("playback-state-changed", {
          isPlaying: e.payload.isPlaying,
          position: e.payload.positionSeconds,
          duration: e.payload.durationSeconds,
        });
        usePlaybackStore.getState().setState(e.payload);
      }),
      listen<TrackChangedPayload>("track-changed", (e) => {
        log.log("track-changed", {
          trackId: e.payload.trackId,
          title: e.payload.title,
        });
        usePlaybackStore.getState().setTrack(e.payload);
      }),
      listen<QueueSnapshotPayload>("queue-changed", (e) => {
        log.log("queue-changed", {
          entries: e.payload.entries.length,
          currentIndex: e.payload.currentIndex,
          repeat: e.payload.repeat,
          shuffle: e.payload.shuffle,
        });
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
        log.log("subscribed: 3 listeners");
      }
    });

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
    };
  }, []);
}
