// EQ bands — the 10-band graphic equalizer on the AudioPlayer.
//
// Hydrated from the backend (`get_eq_bands`) on mount and kept in
// sync with `eq-changed` / `eq-reset` events. The store is a thin
// Zustand cache; mutations always go through `setEqBand` / `resetEq`
// so the backend is the source of truth (the UI never assumes its
// own optimistic write succeeded until the event comes back).

import { create } from "zustand";

import type { EqBandPayload } from "../lib/tauri";

interface EqState {
  bands: EqBandPayload[];
  setBands: (bands: EqBandPayload[]) => void;
  reset: () => void;
}

const EMPTY: EqBandPayload[] = [];

export const useEqStore = create<EqState>((set) => ({
  bands: EMPTY,
  setBands: (bands) => set({ bands }),
  reset: () => set({ bands: EMPTY }),
}));
