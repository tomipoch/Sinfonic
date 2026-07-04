// Playback configuration — crossfade on/off + duration slider.
//
// Hydrated from the backend (`get_crossfade_config`) on mount and
// kept in sync with `playback-config-changed` events. The store is
// a thin Zustand cache; mutations always go through `setCrossfade`
// so the backend is the source of truth (the UI never assumes its
// own optimistic write succeeded until the event comes back).

import { create } from "zustand";

export const CROSSFADE_SECONDS_MIN = 0;
export const CROSSFADE_SECONDS_MAX = 12;
export const CROSSFADE_SECONDS_DEFAULT = 6;

interface PlaybackConfigState {
  crossfadeEnabled: boolean;
  crossfadeSeconds: number;
  hydrate: (config: { crossfadeEnabled: boolean; crossfadeSeconds: number }) => void;
  reset: () => void;
}

const DEFAULTS = {
  crossfadeEnabled: false,
  crossfadeSeconds: CROSSFADE_SECONDS_DEFAULT,
};

export const usePlaybackConfigStore = create<PlaybackConfigState>((set) => ({
  crossfadeEnabled: DEFAULTS.crossfadeEnabled,
  crossfadeSeconds: DEFAULTS.crossfadeSeconds,
  hydrate: (config) =>
    set({
      crossfadeEnabled: config.crossfadeEnabled,
      crossfadeSeconds: config.crossfadeSeconds,
    }),
  reset: () => set(DEFAULTS),
}));
