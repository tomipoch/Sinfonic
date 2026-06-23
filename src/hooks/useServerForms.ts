// useServerForms — extracted form state + handlers for Jellyfin, Subsonic,
// and Local server connection. Shared between ServerManager (Settings window)
// and LoginDialog (inline connection from empty states / SourceSelector).

import { useCallback, useEffect, useState, type FormEvent } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import {
  lastfmConnect,
  lastfmDisconnect,
  lastfmStatus,
  localLogin,
  localRescan,
  type LastFmStatus,
  type LocalScanResult,
} from "@/lib/tauri";
import { useServerStore, type ServerKind } from "@/stores/serverStore";

export type Source = Exclude<ServerKind, "local"> | "local";

function labelFor(source: Source): string {
  switch (source) {
    case "jellyfin":
      return "Jellyfin";
    case "subsonic":
      return "Subsonic";
    case "local":
      return "Local files";
  }
}

/// Prepend `http://` if the user typed a bare host (e.g. `192.168.1.10:8096`).
/// `Url::parse` on the Rust side rejects inputs without a scheme, so we
/// normalise the value client-side first. Existing scheme is preserved.
function normaliseBaseUrl(raw: string): string {
  if (!raw) return raw;
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(raw)) return raw;
  return `http://${raw}`;
}

export function useServerForms() {
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const error = useServerStore((s) => s.error);
  const refreshServers = useServerStore((s) => s.refreshServers);
  const refreshActive = useServerStore((s) => s.refreshActive);
  const discover = useServerStore((s) => s.discover);
  const login = useServerStore((s) => s.login);
  const logout = useServerStore((s) => s.logout);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  const [source, setSource] = useState<Source>("jellyfin");
  const discovered = useServerStore((s) => s.discovered);
  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [localPath, setLocalPath] = useState("");
  const [localBusy, setLocalBusy] = useState(false);
  const [localStats, setLocalStats] = useState<LocalScanResult | null>(null);

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

  useEffect(() => {
    void refreshServers();
    void refreshActive();
    void refreshLastfm();
  }, [refreshServers, refreshActive]);

  const refreshLastfm = useCallback(async () => {
    try {
      const status = await lastfmStatus();
      setLastfm(status);
    } catch (err) {
      console.warn("lastfm status failed", err);
    }
  }, []);

  const onDiscover = useCallback(async () => {
    setDiscovering(true);
    try {
      await discover();
    } finally {
      setDiscovering(false);
    }
  }, [discover]);

  const onRemoteLogin = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      setBusy(true);
      try {
        const normalisedUrl = normaliseBaseUrl(baseUrl.trim());
        const connected = await login({
          kind: source,
          baseUrl: normalisedUrl,
          username,
          password,
        } as Parameters<typeof login>[0]);
        setPassword("");
        toast.success(
          `Connected to ${connected.name}`,
        );
      } catch (err) {
        const message = (err as Error).message || "login failed";
        // Trim the redundant "login failed: " prefix that the Tauri
        // command wraps on. The base error is more useful to show.
        const clean = message.replace(/^login failed:\s*/i, "");
        toast.error(`${labelFor(source)} login failed: ${clean}`);
      } finally {
        setBusy(false);
      }
    },
    [source, baseUrl, username, password, login],
  );

  const onPickLocalPath = useCallback(async () => {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Select your music folder",
      });
      if (typeof picked === "string" && picked.length > 0) {
        setLocalPath(picked);
      }
    } catch (err) {
      toast.error(
        `Couldn't open folder picker: ${(err as Error).message ?? String(err)}`,
      );
    }
  }, []);

  const onLocalScan = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      setLocalBusy(true);
      try {
        const stats = await localLogin(localPath.trim());
        setLocalStats(stats);
        toast.success(
          `Scanned ${stats.tracks} tracks / ${stats.albums} albums` +
            (stats.errors > 0 ? ` (${stats.errors} file(s) skipped)` : ""),
        );
        await login({ kind: "local", path: localPath.trim() });
      } catch (err) {
        toast.error(`Local scan: ${(err as Error).message ?? String(err)}`);
      } finally {
        setLocalBusy(false);
      }
    },
    [localPath, login],
  );

  const onLocalRescan = useCallback(async () => {
    setLocalBusy(true);
    try {
      const stats = await localRescan();
      setLocalStats(stats);
      toast.success(
        `Rescanned ${stats.tracks} tracks / ${stats.albums} albums` +
          (stats.errors > 0 ? ` (${stats.errors} file(s) skipped)` : ""),
      );
    } catch (err) {
      toast.error(`Local rescan: ${(err as Error).message ?? String(err)}`);
    } finally {
      setLocalBusy(false);
    }
  }, []);

  const onLogout = useCallback(async () => {
    setBusy(true);
    try {
      await logout();
    } finally {
      setBusy(false);
    }
  }, [logout]);

  const onSync = useCallback(async () => {
    setBusy(true);
    try {
      await syncLibrary();
    } finally {
      setBusy(false);
    }
  }, [syncLibrary]);

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
  const isLocalSource = source === "local";

  return {
    servers,
    source,
    setSource,
    baseUrl,
    setBaseUrl,
    username,
    setUsername,
    password,
    setPassword,
    busy,
    discovering,
    localPath,
    setLocalPath,
    localBusy,
    localStats,
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
    discovered,
    activeServer,
    isLocalActive,
    isLocalSource,
    activeServerId,
    lastSync,
    error,
    onDiscover,
    onRemoteLogin,
    onLocalScan,
    onLocalRescan,
    onPickLocalPath,
    onLogout,
    onSync,
    onLastfmConnect,
    onLastfmDisconnect,
  };
}
