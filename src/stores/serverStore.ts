// Server store — list of configured providers and the active one.
//
// This is the single source of truth for "which music library am I
// looking at right now?". It coordinates with three sibling stores
// (`useLibraryStore`, `useQueueStore`, `usePlaybackStore`) via
// `resetSessionState` — see the lifecycle notes on each lifecycle
// transition (`logout`, `deleteServer`, `setActive`).

import { create } from "zustand";
import { extractError } from "@/lib/errors";
import {
  jellyfinDiscover,
  providerActiveServer,
  providerDelete,
  providerLogout,
  providerServers,
  providerSetActive,
  providerSyncLibrary,
} from "@/lib/tauri";
import { resetSessionState } from "@/lifecycle/resetSession";
import type {
  ConnectedServer,
  DiscoveredServer,
  JellyfinLoginRequest,
  LocalLoginRequest,
  ServerKind,
  SubsonicLoginRequest,
} from "@/types/domain";
import { makeLogger } from "@/utils/log";

import { dispatchLogin } from "./loginProviders";

const log = makeLogger("serverStore");

/** A trimmed view of `ConnectedServer` for the UI. */
export interface Server {
  id: string;
  kind: ServerKind;
  name: string;
  baseUrl?: string;
  username?: string;
}

/** Lifecycle status of the most recent sync. */
export type SyncStatus = "idle" | "syncing" | "success" | "error";

/**
 * Discriminated union of every login request shape. The `kind`
 * discriminator is what `dispatchLogin` keys the registry on.
 */
export type LoginRequest =
  | ({ kind: "jellyfin" } & JellyfinLoginRequest)
  | ({ kind: "subsonic" } & SubsonicLoginRequest)
  | ({ kind: "local" } & LocalLoginRequest);

/**
 * Pending connection — set by SetupView / LoginDialog when the user
 * submits the connection form, consumed by LoadingView on mount. This
 * keeps all the heavy lifting (auth + scan + sync) on a single route
 * that already owns the sync progress UI.
 */
export type PendingConnection =
  | { kind: "jellyfin"; baseUrl: string; username: string; password: string }
  | { kind: "subsonic"; baseUrl: string; username: string; password: string }
  | { kind: "local"; path: string };

export interface ServerStore {
  /** All servers the backend has persisted (regardless of which is active). */
  servers: Server[];
  /** `serverId` of the currently active provider, or `null` if none. */
  activeServerId: string | null;
  /** Cached Jellyfin discovery result (cleared on logout / setActive). */
  discovered: DiscoveredServer[];
  /** Last sync lifecycle state — drives the SyncBanner chip. */
  lastSync: SyncStatus;
  /** Last error message; cleared via `clearError`. */
  error: string | null;
  /** Set by SetupView / LoginDialog; consumed once by LoadingView on mount. */
  pendingConnection: PendingConnection | null;

  /**
   * Re-pull the saved-servers list from the backend. Resets `error` on
   * the success path; on failure the message lands in `error`.
   */
  refreshServers: () => Promise<void>;
  /** Re-pull the active-server id from the backend. */
  refreshActive: () => Promise<void>;
  /** Bulk-set the saved-servers list (used by LoginDialog after success). */
  setServers: (servers: ConnectedServer[]) => void;
  /** Imperatively override the active id without going through the backend. */
  setActiveServerId: (id: string | null) => void;
  /** Set / clear the pending-connection handoff for LoadingView. */
  setPendingConnection: (pending: PendingConnection | null) => void;
  /**
   * Run a Jellyfin SSDP / mDNS discovery pass and return the
   * discovered servers. On failure returns `[]` and stashes the
   * message in `error`.
   */
  discover: () => Promise<DiscoveredServer[]>;
  /**
   * Authenticate against the requested provider, mark it active,
   * and refresh the saved-servers list. Throws on failure (the
   * LoginDialog / SetupView callers toast the message).
   */
  login: (req: LoginRequest) => Promise<ConnectedServer>;
  /**
   * Tear down the active provider session on the backend AND
   * eagerly reset the in-memory library / queue / playback caches
   * so the UI doesn't flash the previous server's tracks.
   */
  logout: () => Promise<void>;
  /**
   * Delete a saved server by id. If it was the active one, also
   * reset the in-memory caches (same reasoning as `logout`).
   */
  deleteServer: (serverId: string) => Promise<void>;
  /**
   * Switch the active provider. Throws on failure so callers can
   * bounce back to the server picker. Eagerly resets in-memory
   * caches before the backend event lands.
   */
  setActive: (serverId: string) => Promise<void>;
  /**
   * Trigger a full library resync. Transitions
   * `lastSync: idle → syncing → success | error`. Throws on failure
   * so LoadingView can toast + navigate back to /setup.
   */
  syncLibrary: () => Promise<void>;
  /** Imperatively override the `lastSync` chip (e.g. from SyncOverlay). */
  setSyncStatus: (status: SyncStatus) => void;
  /** Clear the `error` field. */
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
      set({
        servers: servers.map((s) => ({
          id: s.serverId,
          kind: s.kind,
          name: s.name,
          baseUrl: s.baseUrl,
        })),
      });
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
        kind: s.kind,
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
      const connected: ConnectedServer = await dispatchLogin(req);
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
      resetSessionState();
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
        resetSessionState();
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
      resetSessionState();
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
