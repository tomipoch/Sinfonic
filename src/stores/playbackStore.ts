// Playback store — current track, transport state, volume, repeat, shuffle.
//
// KISS: one Zustand store per concern. The store never reaches into
// Tauri directly; components call the IPC wrappers in `lib/tauri.ts`
// and update the store on the result.

import { create } from "zustand";

import type {
  PlaybackStatePayload,
  TrackChangedPayload,
} from "@/types/domain";

export interface PlaybackStore {
  isPlaying: boolean;
  currentTrack: TrackChangedPayload | null;
  positionSeconds: number;
  durationSeconds: number;
  volume: number;
  muted: boolean;
  repeat: "off" | "one" | "all";
  shuffle: boolean;

  setState: (state: PlaybackStatePayload) => void;
  setIsPlaying: (isPlaying: boolean) => void;
  setTrack: (track: TrackChangedPayload) => void;
  setPosition: (position: number) => void;
  setVolume: (volume: number) => void;
  setMuted: (muted: boolean) => void;
  setRepeat: (repeat: "off" | "one" | "all") => void;
  setShuffle: (shuffle: boolean) => void;
  reset: () => void;
}

export const usePlaybackStore = create<PlaybackStore>((set) => ({
  isPlaying: false,
  currentTrack: null,
  positionSeconds: 0,
  durationSeconds: 0,
  volume: 0.8,
  muted: false,
  repeat: "off",
  shuffle: false,

  setState: (state) =>
    set({
      isPlaying: state.isPlaying,
      positionSeconds: state.positionSeconds,
      durationSeconds: state.durationSeconds,
      volume: state.volume,
      muted: state.muted,
      repeat: state.repeat,
      shuffle: state.shuffle,
    }),

  setIsPlaying: (isPlaying: boolean) => set({ isPlaying }),

  setTrack: (track) =>
    set({
      currentTrack: track,
      positionSeconds: 0,
      durationSeconds: 0,
    }),

  setPosition: (position) => set({ positionSeconds: position }),
  setVolume: (volume) => set({ volume: Math.max(0, Math.min(1, volume)) }),
  setMuted: (muted) => set({ muted }),
  setRepeat: (repeat) => set({ repeat }),
  setShuffle: (shuffle) => set({ shuffle }),

  reset: () =>
    set({
      isPlaying: false,
      currentTrack: null,
      positionSeconds: 0,
      durationSeconds: 0,
    }),
}));
