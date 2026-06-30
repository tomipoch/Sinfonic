// Public types for the playback module.
//
// `PlaybackSnapshot` is the data the frontend cares about; it merges
// runtime playback state (from `get_playback_state`) with the current
// track identity (delivered via `track-changed` events).

import type { PlaybackStatePayload, RepeatMode, TrackChangedPayload } from "@/types/domain";

export type { PlaybackStatePayload, RepeatMode, TrackChangedPayload };

export interface PlaybackSnapshot extends PlaybackStatePayload {
  currentTrack: TrackChangedPayload | null;
}

export const DEFAULT_SNAPSHOT: PlaybackSnapshot = {
  isPlaying: false,
  positionSeconds: 0,
  durationSeconds: 0,
  volume: 0.8,
  muted: false,
  repeat: "off",
  shuffle: false,
  currentTrack: null,
};
