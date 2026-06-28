// Queue store — the list of upcoming entries and the current index.
//
// Phase 0 just keeps the shape; the queue is filled by `play_track`
// commands in later phases.

import { create } from "zustand";

import type { QueueSnapshot } from "@/types/domain";

export interface QueueStore extends QueueSnapshot {
  setSnapshot: (snapshot: QueueSnapshot) => void;
  clear: () => void;
}

export const useQueueStore = create<QueueStore>((set) => ({
  serverId: null,
  entries: [],
  currentIndex: null,
  repeat: "off",
  shuffle: false,
  shuffleSeed: 0,

  setSnapshot: (snapshot) => set(snapshot),
  clear: () => set({ entries: [], currentIndex: null, serverId: null }),
}));
