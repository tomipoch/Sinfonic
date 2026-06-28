// useServerSettings — Saved servers, sync, logout, and Last.fm state.
//
// Connection form state (Jellyfin / Subsonic / Local inputs) now lives
// inside `ServerConnectionForm`; this hook covers the surrounding
// concerns that the Settings window still owns:
//   * the list of saved servers and the currently active one,
//   * the manual sync button state,
//   * the Last.fm credentials form + connect/disconnect handlers.
//
// Refresh on mount is the only side-effect; everything else is a
// selector or a thin callback around the Zustand store.

import { type FormEvent, useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { extractError } from "@/lib/errors";
import {
  bootstrapState,
  type LastFmStatus,
  lastfmConnect,
  lastfmDisconnect,
  lastfmStatus,
} from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";

export interface ServerSettings {
  servers: ReturnType<typeof useServerStore.getState>["servers"];
  activeServer: ReturnType<typeof useServerStore.getState>["servers"][number] | undefined;
  isLocalActive: boolean;
  activeServerId: string | null;
  lastSync: ReturnType<typeof useServerStore.getState>["lastSync"];
  error: string | null;
  discovered: ReturnType<typeof useServerStore.getState>["discovered"];

  refreshServers: () => Promise<void>;
  refreshActive: () => Promise<void>;

  lastfm: LastFmStatus;
  lastfmApiKey: string;
  setLastfmApiKey: (v: string) => void;
  lastfmApiSecret: string;
  setLastfmApiSecret: (v: string) => void;
  lastfmUsername: string;
  setLastfmUsername: (v: string) => void;
  lastfmPassword: string;
  setLastfmPassword: (v: string) => void;
  lastfmBusy: boolean;
  onLastfmConnect: (event: FormEvent) => Promise<void>;
  onLastfmDisconnect: () => Promise<void>;
}

export function useServerSettings(): ServerSettings {
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const error = useServerStore((s) => s.error);
  const discovered = useServerStore((s) => s.discovered);
  // Post-login/disconnect refreshers returned below so ServerManager can
  // repaint the saved-servers list after a mutation without waiting on
  // the next bootstrap poll. Subscribed here (not just read from the
  // store) so callers always get the latest fn reference.
  const refreshServers = useServerStore((s) => s.refreshServers);
  const refreshActive = useServerStore((s) => s.refreshActive);

  const [lastfm, setLastfm] = useState<LastFmStatus>({
    configured: false,
    authenticated: false,
    username: null,
  });
  const [lastfmApiKey, setLastfmApiKey] = useState("");
  const [lastfmApiSecret, setLastfmApiSecret] = useState("");
  const [lastfmUsername, setLastfmUsername] = useState("");
  const [lastfmPassword, setLastfmPassword] = useState("");
  const [lastfmBusy, setLastfmBusy] = useState(false);

  const refreshLastfm = useCallback(async () => {
    try {
      const status = await lastfmStatus();
      setLastfm(status);
    } catch (err) {
      console.warn("lastfm status failed", err);
    }
  }, []);

  // Single round-trip: bootstrapState returns { ready, activeServerId,
  // savedServers }. Lets the Settings window paint in one shot instead
  // of waiting on three independent invokes that arrive out of order.
  const refreshBootstrap = useCallback(async () => {
    try {
      const state = await bootstrapState();
      useServerStore.getState().setServers(state.savedServers);
      useServerStore.getState().setActiveServerId(state.activeServerId);
    } catch (err) {
      console.warn("bootstrapState failed", err);
    }
  }, []);

  useEffect(() => {
    void refreshBootstrap();
    void refreshLastfm();
  }, [refreshBootstrap, refreshLastfm]);

  const onLastfmConnect = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      setLastfmBusy(true);
      try {
        const status = await lastfmConnect({
          apiKey: lastfmApiKey.trim(),
          apiSecret: lastfmApiSecret.trim(),
          username: lastfmUsername.trim(),
          password: lastfmPassword,
        });
        setLastfm(status);
        setLastfmPassword("");
        toast.success(
          status.username ? `Last.fm connected as ${status.username}` : "Last.fm connected",
        );
      } catch (err) {
        toast.error(`Last.fm: ${extractError(err, "unknown error")}`);
      } finally {
        setLastfmBusy(false);
      }
    },
    [lastfmApiKey, lastfmApiSecret, lastfmUsername, lastfmPassword],
  );

  const onLastfmDisconnect = useCallback(async () => {
    setLastfmBusy(true);
    try {
      const status = await lastfmDisconnect();
      setLastfm(status);
      toast.success("Last.fm disconnected");
    } catch (err) {
      toast.error(`Last.fm: ${extractError(err, "unknown error")}`);
    } finally {
      setLastfmBusy(false);
    }
  }, []);

  const activeServer = servers.find((s) => s.id === activeServerId);
  const isLocalActive = activeServer?.kind === "local";

  return {
    servers,
    activeServer,
    isLocalActive,
    activeServerId,
    lastSync,
    error,
    discovered,
    refreshServers,
    refreshActive,
    lastfm,
    lastfmApiKey,
    setLastfmApiKey,
    lastfmApiSecret,
    setLastfmApiSecret,
    lastfmUsername,
    setLastfmUsername,
    lastfmPassword,
    setLastfmPassword,
    lastfmBusy,
    onLastfmConnect,
    onLastfmDisconnect,
  };
}
