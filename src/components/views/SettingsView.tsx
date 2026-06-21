// Settings — manage server connections (Jellyfin and Subsonic) and
// trigger library sync. Phase 3 added the Jellyfin path; Phase 5
// adds Subsonic with the same form layout but no discovery (the
// Subsonic protocol has no UDP broadcast equivalent). Phase 7 adds
// an optional Last.fm scrobble section below the server block.

import { useEffect, useState } from "react";
import { toast } from "sonner";

import {
  lastfmConnect,
  lastfmDisconnect,
  lastfmStatus,
  type LastFmStatus,
} from "../../lib/tauri";
import { useServerStore, type ServerKind } from "../../stores/serverStore";

export function SettingsView() {
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const discovered = useServerStore((s) => s.discovered);
  const lastSync = useServerStore((s) => s.lastSync);
  const error = useServerStore((s) => s.error);
  const refreshServers = useServerStore((s) => s.refreshServers);
  const refreshActive = useServerStore((s) => s.refreshActive);
  const discover = useServerStore((s) => s.discover);
  const login = useServerStore((s) => s.login);
  const logout = useServerStore((s) => s.logout);
  const syncLibrary = useServerStore((s) => s.syncLibrary);
  const clearError = useServerStore((s) => s.clearError);

  const [kind, setKind] = useState<Exclude<ServerKind, "local">>("jellyfin");
  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);

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

  const refreshLastfm = async () => {
    try {
      const status = await lastfmStatus();
      setLastfm(status);
    } catch (err) {
      console.warn("lastfm status failed", err);
    }
  };

  const onDiscover = async () => {
    setDiscovering(true);
    try {
      await discover();
    } finally {
      setDiscovering(false);
    }
  };

  const onLogin = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    clearError();
    try {
      await login({
        kind,
        baseUrl: baseUrl.trim(),
        username,
        password,
      });
      setPassword("");
    } catch {
      // Error already on the store.
    } finally {
      setBusy(false);
    }
  };

  const onLogout = async () => {
    setBusy(true);
    try {
      await logout();
    } finally {
      setBusy(false);
    }
  };

  const onSync = async () => {
    setBusy(true);
    try {
      await syncLibrary();
    } finally {
      setBusy(false);
    }
  };

  const onLastfmConnect = async (event: React.FormEvent) => {
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
  };

  const onLastfmDisconnect = async () => {
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
  };

  const activeServer = servers.find((s) => s.id === activeServerId);

  return (
    <section className="mx-auto flex max-w-3xl flex-col gap-6 p-6">
      <header>
        <h1 className="text-2xl font-semibold">Settings</h1>
        <p className="text-sm text-fg-subtle">
          Connect a music provider to start playing.
        </p>
      </header>

      {activeServerId ? (
        <div className="rounded-md border border-bg-raised bg-bg-subtle p-4">
          <div className="text-sm text-fg-subtle">Connected</div>
          <div className="mt-1 text-base font-medium text-fg">
            {activeServer?.name ?? activeServerId}
          </div>
          {activeServer?.baseUrl && (
            <div className="mt-1 text-xs text-fg-subtle">
              {activeServer.baseUrl}
            </div>
          )}
          <div className="mt-3 flex gap-2">
            <button
              type="button"
              onClick={onSync}
              disabled={busy || lastSync === "syncing"}
              className="btn-primary"
            >
              {lastSync === "syncing" ? "Syncing…" : "Sync library"}
            </button>
            <button
              type="button"
              onClick={onLogout}
              disabled={busy}
              className="btn-secondary"
            >
              Disconnect
            </button>
          </div>
          {lastSync !== "idle" && (
            <div className="mt-2 text-xs text-fg-subtle">
              Last sync: {lastSync}
            </div>
          )}
        </div>
      ) : (
        <form
          onSubmit={onLogin}
          className="flex flex-col gap-3 rounded-md border border-bg-raised bg-bg-subtle p-4"
        >
          <div className="flex items-center gap-2">
            <h2 className="text-lg font-medium">Connect to</h2>
            <div className="flex gap-1 rounded-md border border-bg-raised bg-bg p-0.5">
              <button
                type="button"
                onClick={() => setKind("jellyfin")}
                className={`rounded px-3 py-1 text-sm ${kind === "jellyfin" ? "bg-accent text-fg" : "text-fg-subtle"}`}
              >
                Jellyfin
              </button>
              <button
                type="button"
                onClick={() => setKind("subsonic")}
                className={`rounded px-3 py-1 text-sm ${kind === "subsonic" ? "bg-accent text-fg" : "text-fg-subtle"}`}
              >
                Subsonic
              </button>
            </div>
          </div>

          <p className="text-xs text-fg-subtle">
            {kind === "jellyfin"
              ? "Jellyfin supports both auto-discovery on the local network and manual entry."
              : "Subsonic / Navidrome / Funkwhale — manual entry only. Salt and token are computed per request, so your password never leaves the device."}
          </p>

          <label className="flex flex-col gap-1 text-sm">
            <span className="text-fg-subtle">Server URL</span>
            <input
              type="url"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              placeholder={
                kind === "jellyfin"
                  ? "http://192.168.1.10:8096"
                  : "http://192.168.1.10:4533"
              }
              required
              className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
            />
          </label>

          <label className="flex flex-col gap-1 text-sm">
            <span className="text-fg-subtle">Username</span>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.currentTarget.value)}
              required
              autoComplete="username"
              className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
            />
          </label>

          <label className="flex flex-col gap-1 text-sm">
            <span className="text-fg-subtle">Password</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              required
              autoComplete="current-password"
              className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
            />
          </label>

          {error && (
            <div className="rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">
              {error}
            </div>
          )}

          <div className="flex gap-2">
            <button type="submit" disabled={busy} className="btn-primary">
              {busy ? "Connecting…" : "Connect"}
            </button>
          </div>
        </form>
      )}

      {kind === "jellyfin" && (
        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-medium">Discovery</h2>
            <button
              type="button"
              onClick={onDiscover}
              disabled={discovering}
              className="btn-secondary"
            >
              {discovering ? "Scanning…" : "Scan local network"}
            </button>
          </div>
          {discovered.length === 0 ? (
            <p className="text-sm text-fg-subtle">
              No servers found. Make sure your Jellyfin server is on the same network.
            </p>
          ) : (
            <ul className="divide-y divide-bg-raised rounded-md border border-bg-raised">
              {discovered.map((d) => (
                <li
                  key={d.serverId}
                  className="flex items-center justify-between gap-3 px-3 py-2"
                >
                  <div>
                    <div className="text-sm font-medium text-fg">{d.name}</div>
                    <div className="text-xs text-fg-subtle">{d.baseUrl}</div>
                  </div>
                  <button
                    type="button"
                    className="btn-secondary"
                    onClick={() => setBaseUrl(d.baseUrl)}
                  >
                    Use this URL
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <section className="flex flex-col gap-2">
        <h2 className="text-lg font-medium">Saved servers</h2>
        {servers.length === 0 ? (
          <p className="text-sm text-fg-subtle">No servers saved yet.</p>
        ) : (
          <ul className="divide-y divide-bg-raised rounded-md border border-bg-raised">
            {servers.map((s) => (
              <li key={s.id} className="flex items-center justify-between px-3 py-2">
                <div>
                  <div className="text-sm font-medium text-fg">{s.name}</div>
                  <div className="text-xs text-fg-subtle">
                    {s.kind}
                    {s.baseUrl ? ` · ${s.baseUrl}` : ""}
                  </div>
                </div>
                {s.id === activeServerId && (
                  <span className="text-xs text-accent">active</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-medium">Last.fm scrobbling</h2>
            <p className="text-xs text-fg-subtle">
              Optional. Scrobble tracks you play to your Last.fm profile.
            </p>
          </div>
          <span
            className={
              "rounded-full px-2 py-0.5 text-xs " +
              (lastfm.authenticated
                ? "bg-accent/20 text-accent"
                : lastfm.configured
                  ? "bg-yellow-500/20 text-yellow-300"
                  : "bg-bg-raised text-fg-subtle")
            }
          >
            {lastfm.authenticated
              ? `Connected${lastfm.username ? ` as ${lastfm.username}` : ""}`
              : lastfm.configured
                ? "Session expired"
                : "Not configured"}
          </span>
        </div>

        {lastfm.authenticated ? (
          <div className="rounded-md border border-bg-raised bg-bg-subtle p-4">
            <p className="text-sm text-fg">
              Scrobbling is on. A scrobble is sent when a track crosses
              50 % of its duration (or when it ends).
            </p>
            <button
              type="button"
              onClick={onLastfmDisconnect}
              disabled={lastfmBusy}
              className="btn-secondary mt-3"
            >
              {lastfmBusy ? "Disconnecting…" : "Disconnect"}
            </button>
          </div>
        ) : (
          <form
            onSubmit={onLastfmConnect}
            className="flex flex-col gap-3 rounded-md border border-bg-raised bg-bg-subtle p-4"
          >
            <p className="text-xs text-fg-subtle">
              Create an API account at{" "}
              <a
                className="text-accent underline"
                href="https://www.last.fm/api/account/create"
                target="_blank"
                rel="noreferrer"
              >
                last.fm/api
              </a>{" "}
              and paste the credentials here. Your password is hashed
              locally and never persisted.
            </p>
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-fg-subtle">API key</span>
              <input
                type="text"
                value={lastfmApiKey}
                onChange={(e) => setLastfmApiKey(e.currentTarget.value)}
                required
                className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
              />
            </label>
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-fg-subtle">API secret</span>
              <input
                type="password"
                value={lastfmApiSecret}
                onChange={(e) => setLastfmApiSecret(e.currentTarget.value)}
                required
                className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
              />
            </label>
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-fg-subtle">Last.fm username</span>
              <input
                type="text"
                value={lastfmUsername}
                onChange={(e) => setLastfmUsername(e.currentTarget.value)}
                required
                autoComplete="username"
                className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
              />
            </label>
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-fg-subtle">Last.fm password</span>
              <input
                type="password"
                value={lastfmPassword}
                onChange={(e) => setLastfmPassword(e.currentTarget.value)}
                required
                autoComplete="current-password"
                className="rounded-md border border-bg-raised bg-bg px-3 py-2 text-sm text-fg placeholder:text-fg-muted focus:border-accent focus:outline-none"
              />
            </label>
            <div className="flex gap-2">
              <button
                type="submit"
                disabled={lastfmBusy}
                className="btn-primary"
              >
                {lastfmBusy ? "Connecting…" : "Connect"}
              </button>
            </div>
          </form>
        )}
      </section>
    </section>
  );
}
