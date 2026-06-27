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

import { useCallback, useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";

import {
  lastfmConnect,
  lastfmDisconnect,
  lastfmStatus,
  type LastFmStatus,
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

  useEffect(() => {
    void refreshServers();
    void refreshActive();
    void refreshLastfm();
  }, [refreshServers, refreshActive, refreshLastfm]);

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
          status.username
            ? `Last.fm connected as ${status.username}`
            : "Last.fm connected",
        );
      } catch (err) {
        toast.error(`Last.fm: ${(err as Error).message ?? String(err)}`);
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
      toast.error(`Last.fm: ${(err as Error).message ?? String(err)}`);
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

/// Back-compat alias. `useServerForms` was the original god-hook that
/// exposed both connection form state and server settings; the
/// connection form now lives inside `ServerConnectionForm`. New code
/// should depend on `useServerSettings` directly.
export const useServerForms = useServerSettings;
