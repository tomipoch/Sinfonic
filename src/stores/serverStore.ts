// Server store — list of configured providers and the active one.

import { create } from "zustand";

import {
  jellyfinDiscover,
  jellyfinLogin,
  localLogin,
  providerActiveServer,
  providerDelete,
  providerLogout,
  providerServers,
  providerSetActive,
  providerSyncLibrary,
  subsonicLogin,
} from "@/lib/tauri";
import { extractError } from "@/lib/errors";
import { makeLogger } from "@/utils/log";
import type {
  ConnectedServer,
  DiscoveredServer,
  JellyfinLoginRequest,
  LocalLoginRequest,
  SubsonicLoginRequest,
} from "@/types/domain";

import { useLibraryStore } from "./libraryStore";
import { useQueueStore } from "./queueStore";
import { usePlaybackStore } from "./playbackStore";

const log = makeLogger("serverStore");

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

/**
 * Pending connection — set by the SetupView/LoginDialog when the user
 * submits the connection form, consumed by LoadingView on mount. This
 * keeps all the heavy lifting (auth + scan + sync) on a single route
 * that already owns the sync progress UI.
 */
export type PendingConnection =
  | { kind: "jellyfin"; baseUrl: string; username: string; password: string }
  | { kind: "subsonic"; baseUrl: string; username: string; password: string }
  | { kind: "local"; path: string };

export interface ServerStore {
  servers: Server[];
  activeServerId: string | null;
  discovered: DiscoveredServer[];
  lastSync: SyncStatus;
  error: string | null;
  pendingConnection: PendingConnection | null;

  refreshServers: () => Promise<void>;
  refreshActive: () => Promise<void>;
  setServers: (servers: ConnectedServer[]) => void;
  setActiveServerId: (id: string | null) => void;
  setPendingConnection: (pending: PendingConnection | null) => void;
  discover: () => Promise<DiscoveredServer[]>;
  login: (req: LoginRequest) => Promise<ConnectedServer>;
  logout: () => Promise<void>;
  deleteServer: (serverId: string) => Promise<void>;
  setActive: (serverId: string) => Promise<void>;
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
  pendingConnection: null,

  refreshServers: async () => {
    try {
      const servers = await providerServers();
      set({ servers: servers.map((s) => ({ id: s.serverId, kind: s.kind as ServerKind, name: s.name, baseUrl: s.baseUrl })) });
    } catch (e) {
      set({ error: extractError(e, "couldn't load servers") });
    }
  },

  refreshActive: async () => {
    try {
      const id = await providerActiveServer();
      set({ activeServerId: id });
    } catch (e) {
      set({ error: extractError(e, "couldn't load active server") });
    }
  },

  setServers: (servers) => {
    set({
      servers: servers.map((s) => ({
        id: s.serverId,
        kind: s.kind as ServerKind,
        name: s.name,
        baseUrl: s.baseUrl,
      })),
    });
  },

  setActiveServerId: (id) => {
    set({ activeServerId: id });
  },

  setPendingConnection: (pending) => {
    set({ pendingConnection: pending });
  },

  discover: async () => {
    log.log("discover: start");
    try {
      const found = await jellyfinDiscover();
      log.log("discover: found", found.length, "server(s)");
      set({ discovered: found });
      return found;
    } catch (e) {
      log.error("discover: failed", e);
      set({ error: extractError(e, "discovery failed") });
      return [];
    }
  },

  login: async (req) => {
    set({ error: null });
    log.log("login: start", { kind: req.kind });
    try {
      let connected: ConnectedServer;
      if (req.kind === "jellyfin") {
        log.log("login: jellyfin", req.baseUrl, req.username);
        connected = await jellyfinLogin({
          baseUrl: req.baseUrl,
          username: req.username,
          password: req.password,
        });
      } else if (req.kind === "subsonic") {
        log.log("login: subsonic", req.baseUrl, req.username);
        connected = await subsonicLogin({
          baseUrl: req.baseUrl,
          username: req.username,
          password: req.password,
        });
      } else {
        log.log("login: local", req.path);
        // Local: `local_login` does the whole scan + provider install
        // + SQLite write as a single atomic step and emits the same
        // `library-sync-status` progress events the LoadingView
        // listens to.
        await localLogin(req.path);
        connected = {
          serverId: "server-local",
          kind: "local",
          name: "Local files",
          baseUrl: req.path,
        };
      }
      log.log("login: success", connected);
      set({ activeServerId: connected.serverId });
      // Refresh the full list so the Settings view can show it.
      await get().refreshServers();
      return connected;
    } catch (e) {
      log.error("login: failed", e);
      const msg = extractError(e, "login failed");
      set({ error: msg });
      throw e;
    }
  },

  logout: async () => {
    log.log("logout: start");
    try {
      await providerLogout();
      set({ activeServerId: null });
      await get().refreshServers();
      // Library cache is server-scoped; dropping all rows keeps the
      // UI honest about no longer having a source to read from. The
      // queue + current track are cleared eagerly so the QueuePanel
      // and PlayerBar don't flash stale entries from a session that's
      // just ended before the backend event lands.
      useLibraryStore.getState().reset();
      useQueueStore.getState().clear();
      usePlaybackStore.getState().reset();
      log.log("logout: done");
    } catch (e) {
      log.error("logout: failed", e);
      set({ error: extractError(e, "logout failed") });
    }
  },

  deleteServer: async (serverId: string) => {
    log.log("deleteServer: start", serverId);
    try {
      await providerDelete(serverId);
      // If the deleted server was active, clear the active id.
      if (get().activeServerId === serverId) {
        log.log("deleteServer: was active, clearing local state");
        set({ activeServerId: null });
        useLibraryStore.getState().reset();
        useQueueStore.getState().clear();
        usePlaybackStore.getState().reset();
      }
      await get().refreshServers();
      log.log("deleteServer: done", serverId);
    } catch (e) {
      log.error("deleteServer: failed", serverId, e);
      set({ error: extractError(e, "delete server failed") });
    }
  },

  setActive: async (serverId: string) => {
    set({ error: null });
    log.log("setActive: start", serverId);
    try {
      const connected = await providerSetActive(serverId);
      log.log("setActive: backend returned", connected);
      // Setting `activeServerId` here lets `useLibraryAutoLoad`
      // reset + reload the cached library in one place. We do an
      // eager reset here too so the UI doesn't briefly show the
      // previous server's rows while the effect runs. The queue
      // and current track are dropped for the same reason — the
      // new provider's library has different track ids, so the
      // old queue entries would never resolve.
      useLibraryStore.getState().reset();
      useQueueStore.getState().clear();
      usePlaybackStore.getState().reset();
      set({ activeServerId: connected.serverId });
      log.log("setActive: done", connected.serverId);
    } catch (e) {
      // Tauri rejects invoke() with the **string** the Rust command
      // returned in Err. Use extractError so the actual backend
      // message (e.g. "subsonic: password missing from keyring")
      // surfaces to the user instead of the generic fallback.
      const msg = extractError(e, "switch source failed");
      log.error("setActive: failed", serverId, msg);
      set({ error: msg });
      throw e;
    }
  },

  syncLibrary: async () => {
    log.log("syncLibrary: start");
    set({ lastSync: "syncing", error: null });
    try {
      await providerSyncLibrary();
      log.log("syncLibrary: success");
      set({ lastSync: "success" });
    } catch (e) {
      // Propagate the error so the caller (LoadingView's handoff)
      // can surface it as a toast and bounce back to /setup. The
      // store-level `lastSync`/`error` are also kept in sync for
      // any view that watches them.
      const msg = extractError(e, "sync failed");
      log.error("syncLibrary: failed", msg);
      set({ lastSync: "error", error: msg });
      throw e;
    }
  },

  setSyncStatus: (status) => set({ lastSync: status }),
  clearError: () => set({ error: null }),
}));
