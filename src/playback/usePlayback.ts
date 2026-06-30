// `usePlayback` — the single source of playback state for the UI.
//
// Two data paths merge into one snapshot:
//   1. `get_playback_state` IPC polled every 250 ms — is the rodio
//      sink still playing? where's the playhead? volume?
//   2. `track-changed` event — what track is current? (rare:
//      changes only on play / next / previous / track-end).
//
// We poll instead of subscribing to a state event for one reason:
// the macOS WebView's `WKScriptMessageHandler` was wedging after
// one or two `app.emit` calls, which froze the position bar. A
// polled IPC round-trip is unaffected by that bug.
//
// Commands (togglePlay, seekTo, setVolume, ...) update the snapshot
// optimistically before the IPC resolves, then roll back on error.
// The polled state converges to the truth within one tick.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { getPlaybackState } from "@/lib/tauri";
import type { PlaybackStatePayload, TrackChangedPayload } from "@/types/domain";
import { makeLogger } from "@/utils/log";
import {
  next,
  pause,
  previous,
  resume,
  seek,
  setMuted,
  setRepeat,
  setShuffle,
  setVolume,
} from "./commands";
import { nextRepeat } from "./repeat";
import type { PlaybackSnapshot } from "./types";
import { DEFAULT_SNAPSHOT } from "./types";

const POLL_INTERVAL_MS = 250;

const log = makeLogger("usePlayback");

function snapshotEqual(a: PlaybackSnapshot, b: PlaybackSnapshot): boolean {
  return (
    a.isPlaying === b.isPlaying &&
    a.positionSeconds === b.positionSeconds &&
    a.durationSeconds === b.durationSeconds &&
    a.volume === b.volume &&
    a.muted === b.muted &&
    a.repeat === b.repeat &&
    a.shuffle === b.shuffle &&
    a.currentTrack?.trackId === b.currentTrack?.trackId
  );
}

export interface PlaybackControls {
  snapshot: PlaybackSnapshot;
  togglePlay: () => Promise<void>;
  next: () => Promise<void>;
  previous: () => Promise<void>;
  seekTo: (positionSeconds: number) => Promise<void>;
  setVolume: (volume: number) => Promise<void>;
  setMuted: (muted: boolean) => Promise<void>;
  setRepeat: (mode: PlaybackSnapshot["repeat"]) => Promise<void>;
  cycleRepeat: () => Promise<void>;
  setShuffle: (enabled: boolean) => Promise<void>;
  reset: () => void;
}

export function usePlayback(): PlaybackControls {
  const [snapshot, setSnapshot] = useState<PlaybackSnapshot>(DEFAULT_SNAPSHOT);
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;

  // 1) Bootstrap: pull the current state once on mount so the UI
  //    doesn't start from all-zeros if the backend already has a
  //    sink playing (HMR, restore from session, etc.).
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const initial = await getPlaybackState();
        if (cancelled) return;
        setSnapshot((prev) => applyState(prev, initial));
      } catch (err) {
        log.warn("bootstrap: get_playback_state failed", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 2) Polling: refresh the runtime fields every 250 ms. We skip
  //    re-renders when nothing changed so the seek bar updates
  //    don't thrash unrelated consumers.
  useEffect(() => {
    let cancelled = false;
    const id = window.setInterval(async () => {
      if (cancelled) return;
      try {
        const next = await getPlaybackState();
        if (cancelled) return;
        setSnapshot((prev) => {
          const merged = applyState(prev, next);
          return snapshotEqual(prev, merged) ? prev : merged;
        });
      } catch {
        // Polling is best-effort; ignore transient IPC errors.
      }
    }, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  // 3) Track identity: subscribe to `track-changed` so the now-playing
  //    card updates instantly when the user hits play / next /
  //    previous, without waiting for the next poll.
  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    void listen<TrackChangedPayload>("track-changed", (event) => {
      if (cancelled) return;
      setSnapshot((prev) => {
        if (prev.currentTrack?.trackId === event.payload.trackId) return prev;
        return { ...prev, currentTrack: event.payload };
      });
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const updateOptimistic = useCallback((patch: Partial<PlaybackSnapshot>) => {
    setSnapshot((prev) => ({ ...prev, ...patch }));
  }, []);

  const rollback = useCallback((previous: Partial<PlaybackSnapshot>) => {
    setSnapshot((prev) => ({ ...prev, ...previous }));
  }, []);

  const togglePlay = useCallback(async () => {
    const target = !snapshotRef.current.isPlaying;
    updateOptimistic({ isPlaying: target });
    try {
      await (target ? resume() : pause());
    } catch (err) {
      rollback({ isPlaying: !target });
      throw err;
    }
  }, [rollback, updateOptimistic]);

  const seekTo = useCallback(
    async (positionSeconds: number) => {
      const prev = snapshotRef.current.positionSeconds;
      updateOptimistic({ positionSeconds });
      try {
        await seek(positionSeconds);
      } catch (err) {
        rollback({ positionSeconds: prev });
        throw err;
      }
    },
    [rollback, updateOptimistic],
  );

  const setVolumeCtl = useCallback(
    async (volume: number) => {
      const prev = snapshotRef.current.volume;
      updateOptimistic({ volume });
      try {
        await setVolume(volume);
      } catch (err) {
        rollback({ volume: prev });
        throw err;
      }
    },
    [rollback, updateOptimistic],
  );

  const setMutedCtl = useCallback(
    async (muted: boolean) => {
      const prev = snapshotRef.current.muted;
      updateOptimistic({ muted });
      try {
        await setMuted(muted);
      } catch (err) {
        rollback({ muted: prev });
        throw err;
      }
    },
    [rollback, updateOptimistic],
  );

  const setRepeatCtl = useCallback(
    async (mode: PlaybackSnapshot["repeat"]) => {
      const prev = snapshotRef.current.repeat;
      updateOptimistic({ repeat: mode });
      try {
        await setRepeat(mode);
      } catch (err) {
        rollback({ repeat: prev });
        throw err;
      }
    },
    [rollback, updateOptimistic],
  );

  const cycleRepeatCtl = useCallback(async () => {
    await setRepeatCtl(nextRepeat(snapshotRef.current.repeat));
  }, [setRepeatCtl]);

  const setShuffleCtl = useCallback(
    async (enabled: boolean) => {
      const prev = snapshotRef.current.shuffle;
      updateOptimistic({ shuffle: enabled });
      try {
        await setShuffle(enabled);
      } catch (err) {
        rollback({ shuffle: prev });
        throw err;
      }
    },
    [rollback, updateOptimistic],
  );

  const resetCtl = useCallback(() => {
    setSnapshot(DEFAULT_SNAPSHOT);
  }, []);

  return {
    snapshot,
    togglePlay,
    next: () => next(),
    previous: () => previous(),
    seekTo,
    setVolume: setVolumeCtl,
    setMuted: setMutedCtl,
    setRepeat: setRepeatCtl,
    cycleRepeat: cycleRepeatCtl,
    setShuffle: setShuffleCtl,
    reset: resetCtl,
  };
}

function applyState(prev: PlaybackSnapshot, state: PlaybackStatePayload): PlaybackSnapshot {
  return {
    isPlaying: state.isPlaying,
    positionSeconds: state.positionSeconds,
    durationSeconds: state.durationSeconds,
    volume: state.volume,
    muted: state.muted,
    repeat: state.repeat,
    shuffle: state.shuffle,
    currentTrack: prev.currentTrack,
  };
}
