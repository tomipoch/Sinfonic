// Queue store — the list of upcoming entries and the current index,
// plus the side panel's current mode (queue / lyrics).
//
// The panelMode lives here so the right-side panel can render
// without having to receive a prop from <Layout>; the PlayerBar
// toggles it and the panel reads it. Keeping it next to the queue
// shape avoids a second Zustand store for one boolean.
//
// Phase 0 just keeps the shape; the queue is filled by `play_track`
// commands in later phases.

import { create } from "zustand";

import type { QueueSnapshot } from "@/types/domain";

export type QueuePanelMode = "queue" | "lyrics";

export interface QueueStore extends QueueSnapshot {
  panelMode: QueuePanelMode | null;
  setSnapshot: (snapshot: QueueSnapshot) => void;
  clear: () => void;
  setPanelMode: (mode: QueuePanelMode | null) => void;
}

export const useQueueStore = create<QueueStore>((set) => ({
  serverId: null,
  entries: [],
  currentIndex: null,
  repeat: "off",
  shuffle: false,
  shuffleSeed: 0,
  panelMode: null,

  setSnapshot: (snapshot) => set(snapshot),
  clear: () => set({ entries: [], currentIndex: null, serverId: null, panelMode: null }),
  setPanelMode: (mode) => set({ panelMode: mode }),
}));
