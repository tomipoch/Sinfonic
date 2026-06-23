// Server store — list of configured providers and the active one.

import { create } from "zustand";

import {
  jellyfinDiscover,
  jellyfinLogin,
  localLogin,
  providerActiveServer,
  providerLogout,
  providerServers,
  providerSyncLibrary,
  subsonicLogin,
} from "@/lib/tauri";
import type {
  ConnectedServer,
  DiscoveredServer,
  JellyfinLoginRequest,
  LocalLoginRequest,
  SubsonicLoginRequest,
} from "@/types/domain";

export type ServerKind = "jellyfin" | "subsonic" | "local";

export interface Server {
  id: string;
  kind: ServerKind;
  name: string;
  baseUrl?: string;
  username?: string;
}

export type SyncStatus = "idle" | "syncing" | "success" | "error";

export type LoginRequest =
  | ({ kind: "jellyfin" } & JellyfinLoginRequest)
  | ({ kind: "subsonic" } & SubsonicLoginRequest)
  | ({ kind: "local" } & LocalLoginRequest);

export interface ServerStore {
  servers: Server[];
  activeServerId: string | null;
  discovered: DiscoveredServer[];
  lastSync: SyncStatus;
  error: string | null;

  refreshServers: () => Promise<void>;
  refreshActive: () => Promise<void>;
  discover: () => Promise<DiscoveredServer[]>;
  login: (req: LoginRequest) => Promise<ConnectedServer>;
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
      const servers = await providerServers();
      set({ servers: servers as unknown as Server[] });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  refreshActive: async () => {
    try {
      const id = await providerActiveServer();
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
      let connected: ConnectedServer;
      if (req.kind === "jellyfin") {
        connected = await jellyfinLogin({
          baseUrl: req.baseUrl,
          username: req.username,
          password: req.password,
        });
      } else if (req.kind === "subsonic") {
        connected = await subsonicLogin({
          baseUrl: req.baseUrl,
          username: req.username,
          password: req.password,
        });
      } else {
        // Local: no `ConnectedServer` is returned (the scan result
        // carries stats, not server metadata), so synthesise one
        // from the canonical local server id.
        await localLogin(req.path);
        connected = {
          serverId: "server-local",
          kind: "local",
          name: "Local files",
          baseUrl: req.path,
        };
      }
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
      await providerLogout();
      set({ activeServerId: null });
      await get().refreshServers();
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  syncLibrary: async () => {
    set({ lastSync: "syncing", error: null });
    try {
      await providerSyncLibrary();
      set({ lastSync: "success" });
    } catch (e) {
      set({ lastSync: "error", error: (e as Error).message });
    }
  },

  setSyncStatus: (status) => set({ lastSync: status }),
  clearError: () => set({ error: null }),
}));
