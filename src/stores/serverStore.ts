// Server store — list of configured providers and the active one.

import { create } from "zustand";

import {
  jellyfinActiveServer,
  jellyfinDiscover,
  jellyfinLogin,
  jellyfinLogout,
  jellyfinServers,
  jellyfinSyncLibrary,
} from "../lib/tauri";
import type {
  ConnectedServer,
  DiscoveredServer,
  JellyfinLoginRequest,
} from "../types/domain";

export type ServerKind = "jellyfin" | "subsonic" | "local";

export interface Server {
  id: string;
  kind: ServerKind;
  name: string;
  baseUrl?: string;
  username?: string;
}

export type SyncStatus = "idle" | "syncing" | "success" | "error";

export interface ServerStore {
  servers: Server[];
  activeServerId: string | null;
  discovered: DiscoveredServer[];
  lastSync: SyncStatus;
  error: string | null;

  refreshServers: () => Promise<void>;
  refreshActive: () => Promise<void>;
  discover: () => Promise<DiscoveredServer[]>;
  login: (req: JellyfinLoginRequest) => Promise<ConnectedServer>;
  logout: () => Promise<void>;
  syncLibrary: () => Promise<void>;
  setSyncStatus: (status: SyncStatus) => void;
  clearError: () => void;
}

export const useServerStore = create<ServerStore>((set, get) => ({
  servers: [],
  activeServerId: null,
  discovered: [],
  lastSync: "idle",
  error: null,

  refreshServers: async () => {
    try {
      const servers = await jellyfinServers();
      set({ servers: servers as unknown as Server[] });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  refreshActive: async () => {
    try {
      const id = await jellyfinActiveServer();
      set({ activeServerId: id });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  discover: async () => {
    try {
      const found = await jellyfinDiscover();
      set({ discovered: found });
      return found;
    } catch (e) {
      set({ error: (e as Error).message });
      return [];
    }
  },

  login: async (req) => {
    set({ error: null });
    try {
      const connected = await jellyfinLogin(req);
      set({ activeServerId: connected.serverId });
      // Refresh the full list so the Settings view can show it.
      await get().refreshServers();
      return connected;
    } catch (e) {
      const msg = (e as Error).message || "login failed";
      set({ error: msg });
      throw e;
    }
  },

  logout: async () => {
    try {
      await jellyfinLogout();
      set({ activeServerId: null });
      await get().refreshServers();
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  syncLibrary: async () => {
    set({ lastSync: "syncing", error: null });
    try {
      await jellyfinSyncLibrary();
      set({ lastSync: "success" });
    } catch (e) {
      set({ lastSync: "error", error: (e as Error).message });
    }
  },

  setSyncStatus: (status) => set({ lastSync: status }),
  clearError: () => set({ error: null }),
}));