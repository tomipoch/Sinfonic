// Settings — manage Jellyfin server connections and trigger library
// sync. Phase 3 ships the Jellyfin path; Subsonic and local will
// follow in later phases.

import { useEffect, useState } from "react";

import { useServerStore } from "../../stores/serverStore";

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

  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);

  useEffect(() => {
    void refreshServers();
    void refreshActive();
  }, [refreshServers, refreshActive]);

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
      await login({ baseUrl: baseUrl.trim(), username, password });
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
            {servers.find((s) => s.id === activeServerId)?.name ?? activeServerId}
          </div>
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
          <h2 className="text-lg font-medium">Connect to Jellyfin</h2>

          <label className="flex flex-col gap-1 text-sm">
            <span className="text-fg-subtle">Server URL</span>
            <input
              type="url"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              placeholder="http://192.168.1.10:8096"
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
    </section>
  );
}