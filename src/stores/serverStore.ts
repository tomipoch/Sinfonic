// Server store — list of configured providers and the active one.

import { create } from "zustand";

export type ServerKind = "jellyfin" | "subsonic" | "local";

export interface Server {
  id: string;
  kind: ServerKind;
  name: string;
  baseUrl?: string;
  username?: string;
}

export interface ServerStore {
  servers: Server[];
  activeServerId: string | null;

  setServers: (servers: Server[]) => void;
  setActiveServer: (id: string | null) => void;
}

export const useServerStore = create<ServerStore>((set) => ({
  servers: [],
  activeServerId: null,

  setServers: (servers) => set({ servers }),
  setActiveServer: (id) => set({ activeServerId: id }),
}));
