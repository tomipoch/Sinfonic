// ServerManager — server connection + sync UI.
//
// Shared between the standalone Settings window (via `GeneralSection`)
// and any future in-app Settings view. All state + IPC lives in
// `useServerForms` so the wrapping layout only decides *where* to
// render it, not *what*.
//
// Sections (top to bottom):
//   1. Music Source — provider picker + connection form / status
//   2. Discovery     — Jellyfin LAN scan + result list
//   3. Saved         — saved servers list
//   4. Local files   — path picker + scan / rescan
//   5. Last.fm       — scrobbling status + credentials form

import {
  HardDriveIcon,
  Link04Icon,
  Tick02Icon,
  Wifi01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect } from "react";

import { useServerForms } from "@/hooks/useServerForms";
import { useServerStore } from "@/stores/serverStore";
import {
  ChoiceCard,
  SettingsCard,
  SettingsSection,
} from "@/components/settings/primitives";

const SOURCES: { id: "jellyfin" | "subsonic" | "local"; label: string; icon: typeof Link04Icon }[] = [
  { id: "jellyfin", label: "Jellyfin", icon: Link04Icon },
  { id: "subsonic", label: "Subsonic", icon: Wifi01Icon },
  { id: "local", label: "Local files", icon: HardDriveIcon },
];

export function ServerManager() {
  const {
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
  } = useServerForms();

  useEffect(() => {
    useServerStore.getState().clearError();
  }, [source]);

  return (
    <div className="flex flex-col gap-8">
      {/* ─── Music Source ────────────────────────────────────── */}
      <SettingsSection label="Music Source">
        <div className="grid grid-cols-3 gap-2">
          {SOURCES.map((s) => (
            <ChoiceCard
              key={s.id}
              selected={source === s.id}
              onClick={() => setSource(s.id)}
              icon={<HugeiconsIcon icon={s.icon} size={20} strokeWidth={1.5} />}
              label={s.label}
            />
          ))}
        </div>

        {activeServerId ? (
          <SettingsCard>
            <div className="flex items-start justify-between gap-4 px-4 py-4">
              <div className="flex min-w-0 flex-col gap-0.5">
                <div className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                  Connected
                </div>
                <div className="truncate text-base font-medium text-foreground">
                  {activeServer?.name ?? activeServerId}
                </div>
                {activeServer?.baseUrl ? (
                  <div className="truncate text-xs text-muted-foreground">
                    {activeServer.baseUrl}
                  </div>
                ) : null}
                {lastSync !== "idle" ? (
                  <div className="mt-1 text-[11px] text-muted-foreground">
                    Last sync: {lastSync}
                  </div>
                ) : null}
              </div>
              <div className="flex shrink-0 gap-2">
                <button
                  type="button"
                  onClick={onSync}
                  disabled={busy || lastSync === "syncing"}
                  className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                >
                  {lastSync === "syncing" ? "Syncing…" : "Sync"}
                </button>
                <button
                  type="button"
                  onClick={onLogout}
                  disabled={busy}
                  className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
                >
                  Disconnect
                </button>
              </div>
            </div>
          </SettingsCard>
        ) : isLocalSource ? (
          <SettingsCard>
            <form onSubmit={onLocalScan} className="flex flex-col gap-3 px-4 py-4">
              <div className="text-xs text-muted-foreground">
                Point Sinfonic at a directory of audio files (MP3, FLAC, OGG,
                Opus, MP4/M4A, WAV). The directory is walked recursively;
                metadata comes from the file tags.
              </div>
              <label className="flex flex-col gap-1 text-sm">
                <span className="text-muted-foreground">Music folder</span>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={localPath}
                    onChange={(e) => setLocalPath(e.currentTarget.value)}
                    placeholder="/Users/you/Music"
                    required
                    spellCheck={false}
                    autoCorrect="off"
                    autoCapitalize="off"
                    className="flex-1 rounded-md border border-input bg-background px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={() => void onPickLocalPath()}
                    className="shrink-0 rounded-md border border-border px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted"
                  >
                    Browse…
                  </button>
                </div>
              </label>
              {error ? (
                <div
                  role="alert"
                  className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                >
                  {error.replace(/^local scan:\s*/i, "")}
                </div>
              ) : null}
              <button
                type="submit"
                disabled={localBusy}
                className="w-fit rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
              >
                {localBusy ? "Scanning…" : "Scan library"}
              </button>
            </form>
          </SettingsCard>
        ) : (
          <SettingsCard>
            <form onSubmit={onRemoteLogin} className="flex flex-col gap-3 px-4 py-4">
              <div className="text-xs text-muted-foreground">
                {source === "jellyfin"
                  ? "Jellyfin supports both auto-discovery on the local network and manual entry."
                  : "Subsonic / Navidrome / Funkwhale — manual entry only. Salt and token are computed per request, so your password never leaves the device."}
              </div>
              <label className="flex flex-col gap-1 text-sm">
                <span className="text-muted-foreground">Server URL</span>
                <input
                  type="url"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.currentTarget.value)}
                  placeholder={
                    source === "jellyfin"
                      ? "http://192.168.1.10:8096"
                      : "http://192.168.1.10:4533"
                  }
                  required
                  className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                />
              </label>
              <div className="grid grid-cols-2 gap-3">
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-muted-foreground">Username</span>
                  <input
                    type="text"
                    value={username}
                    onChange={(e) => setUsername(e.currentTarget.value)}
                    required
                    autoComplete="username"
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-muted-foreground">Password</span>
                  <input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.currentTarget.value)}
                    required
                    autoComplete="current-password"
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                </label>
              </div>
              {error ? (
                <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                  {error}
                </div>
              ) : null}
              <button
                type="submit"
                disabled={busy}
                className="w-fit rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
              >
                {busy ? "Connecting…" : "Connect"}
              </button>
            </form>
          </SettingsCard>
        )}

        {isLocalActive && localStats ? (
          <div className="text-xs text-muted-foreground">
            {localStats.tracks} tracks · {localStats.albums} albums ·{" "}
            {localStats.artists} artists
            {localStats.errors > 0 ? (
              <span className="ml-1 text-yellow-400">
                · {localStats.errors} file(s) skipped
              </span>
            ) : null}
          </div>
        ) : null}

        {isLocalActive ? (
          <button
            type="button"
            onClick={onLocalRescan}
            disabled={localBusy}
            className="w-fit rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
          >
            {localBusy ? "Rescanning…" : "Rescan local library"}
          </button>
        ) : null}
      </SettingsSection>

      {/* ─── Discovery (Jellyfin only) ────────────────────────── */}
      {source === "jellyfin" && !activeServerId ? (
        <SettingsSection label="Discovery">
          <SettingsCard>
            <div className="flex items-center justify-between gap-4 px-4 py-4">
              <div className="flex min-w-0 flex-col gap-0.5">
                <div className="text-sm font-medium text-foreground">
                  Local network
                </div>
                <div className="text-xs text-muted-foreground">
                  {discovered.length === 0
                    ? "No Jellyfin servers detected yet."
                    : `${discovered.length} server${discovered.length === 1 ? "" : "s"} found.`}
                </div>
              </div>
              <button
                type="button"
                onClick={onDiscover}
                disabled={discovering}
                className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
              >
                {discovering ? "Scanning…" : "Scan"}
              </button>
            </div>
          </SettingsCard>
          {discovered.length > 0 ? (
            <SettingsCard>
              <ul className="divide-y divide-border">
                {discovered.map((d) => (
                  <li
                    key={d.serverId}
                    className="flex items-center justify-between gap-3 px-4 py-2.5"
                  >
                    <div className="flex min-w-0 flex-col">
                      <div className="truncate text-sm font-medium text-foreground">
                        {d.name}
                      </div>
                      <div className="truncate text-xs text-muted-foreground">
                        {d.baseUrl}
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => setBaseUrl(d.baseUrl)}
                      className="rounded-md border border-border px-2 py-1 text-xs text-foreground transition-colors hover:bg-muted"
                    >
                      Use URL
                    </button>
                  </li>
                ))}
              </ul>
            </SettingsCard>
          ) : null}
        </SettingsSection>
      ) : null}

      {/* ─── Saved servers ───────────────────────────────────── */}
      <SettingsSection label="Saved Servers">
        {servers.length === 0 ? (
          <SettingsCard>
            <div className="px-4 py-4 text-sm text-muted-foreground">
              No saved servers yet.
            </div>
          </SettingsCard>
        ) : (
          <SettingsCard>
            <ul className="divide-y divide-border">
              {servers.map((s) => (
                <li
                  key={s.id}
                  className="flex items-center justify-between px-4 py-2.5"
                >
                  <div className="flex min-w-0 flex-col">
                    <div className="truncate text-sm font-medium text-foreground">
                      {s.name}
                    </div>
                    <div className="truncate text-xs text-muted-foreground">
                      {s.kind}
                      {s.baseUrl ? ` · ${s.baseUrl}` : ""}
                    </div>
                  </div>
                  {s.id === activeServerId ? (
                    <span className="inline-flex items-center gap-1 rounded-full bg-primary/15 px-2 py-0.5 text-[11px] font-medium text-primary">
                      <HugeiconsIcon icon={Tick02Icon} size={11} strokeWidth={2.5} />
                      active
                    </span>
                  ) : null}
                </li>
              ))}
            </ul>
          </SettingsCard>
        )}
      </SettingsSection>

      {/* ─── Last.fm ─────────────────────────────────────────── */}
      <SettingsSection label="Last.fm">
        <SettingsCard>
          <div className="flex items-start justify-between gap-4 px-4 py-4">
            <div className="flex min-w-0 flex-col gap-0.5">
              <div className="text-sm font-medium text-foreground">
                Scrobbling
              </div>
              <div className="text-xs text-muted-foreground">
                Send a scrobble to Last.fm when a track crosses 50% of its
                duration.
              </div>
            </div>
            <span
              className={
                "inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[11px] font-medium " +
                (lastfm.authenticated
                  ? "bg-primary/15 text-primary"
                  : lastfm.configured
                    ? "bg-yellow-500/20 text-yellow-300"
                    : "bg-muted text-muted-foreground")
              }
            >
              {lastfm.authenticated
                ? `Connected${lastfm.username ? ` · ${lastfm.username}` : ""}`
                : lastfm.configured
                  ? "Session expired"
                  : "Not configured"}
            </span>
          </div>
          {lastfm.authenticated ? (
            <div className="border-t border-border px-4 py-3">
              <button
                type="button"
                onClick={onLastfmDisconnect}
                disabled={lastfmBusy}
                className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
              >
                {lastfmBusy ? "Disconnecting…" : "Disconnect"}
              </button>
            </div>
          ) : (
            <form
              onSubmit={onLastfmConnect}
              className="flex flex-col gap-3 border-t border-border px-4 py-4"
            >
              <div className="text-xs text-muted-foreground">
                Create an API account at{" "}
                <a
                  className="text-primary underline-offset-2 hover:underline"
                  href="https://www.last.fm/api/account/create"
                  target="_blank"
                  rel="noreferrer"
                >
                  last.fm/api
                </a>
                . Your password is hashed locally and never persisted.
              </div>
              <div className="grid grid-cols-2 gap-3">
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-muted-foreground">API key</span>
                  <input
                    type="text"
                    value={lastfmApiKey}
                    onChange={(e) => setLastfmApiKey(e.currentTarget.value)}
                    required
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-muted-foreground">API secret</span>
                  <input
                    type="password"
                    value={lastfmApiSecret}
                    onChange={(e) => setLastfmApiSecret(e.currentTarget.value)}
                    required
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-muted-foreground">Username</span>
                  <input
                    type="text"
                    value={lastfmUsername}
                    onChange={(e) => setLastfmUsername(e.currentTarget.value)}
                    required
                    autoComplete="username"
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-muted-foreground">Password</span>
                  <input
                    type="password"
                    value={lastfmPassword}
                    onChange={(e) => setLastfmPassword(e.currentTarget.value)}
                    required
                    autoComplete="current-password"
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                  />
                </label>
              </div>
              <button
                type="submit"
                disabled={lastfmBusy}
                className="w-fit rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
              >
                {lastfmBusy ? "Connecting…" : "Connect"}
              </button>
            </form>
          )}
        </SettingsCard>
      </SettingsSection>
    </div>
  );
}
