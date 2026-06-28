// ServerManager — server connection + sync UI for the Settings window.
//
// Sections (top to bottom):
//   1. Music Source — connected-state chrome (sync + disconnect) or the
//      shared `ServerConnectionForm` (handles both source picker and
//      the per-source form body).
//   2. Saved servers — saved servers list with active marker + delete.
//   3. Local files   — rescan action when local is active.
//   4. Last.fm       — scrobbling status + credentials form.

import { Add01Icon, Delete03Icon, Tick02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { LoginDialog } from "@/components/dialogs/LoginDialog";
import {
  type ConnectionValues,
  ServerConnectionForm,
} from "@/components/dialogs/ServerConnectionForm";
import { SettingsCard, SettingsSection } from "@/components/primitives/primitives";
import { useServerSettings } from "@/hooks/useServerSettings";
import { pickLocalFolder } from "@/lib/dialogs";
import { cleanError, extractError } from "@/lib/errors";
import { useServerStore } from "@/stores/serverStore";

export function ServerManager() {
  const {
    servers,
    discovered,
    activeServer,
    isLocalActive,
    activeServerId,
    lastSync,
    error,
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
  } = useServerSettings();

  useEffect(() => {
    useServerStore.getState().clearError();
  }, []);

  const login = useServerStore((s) => s.login);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [adding, setAdding] = useState(false);

  // From the Settings window we can't `navigate("/loading")` — that
  // would target the Settings router (which has no /loading route)
  // and silently no-op. So we bypass the LoadingView handoff and
  // call login() directly. The new server becomes active in-place,
  // the saved-servers list refreshes, and the user can click Sync
  // to populate the cache.
  const handleAddSubmit = async (values: ConnectionValues) => {
    setAdding(true);
    try {
      const connected = await login(values);
      toast.success(`Connected to ${connected.name}`);
      setAddDialogOpen(false);
    } catch (err) {
      const clean = cleanError(extractError(err, "login failed")) ?? "login failed";
      toast.error(`${values.kind === "local" ? "Local scan" : "Login"}: ${clean}`);
    } finally {
      setAdding(false);
    }
  };

  const handleSubmit = async (values: ConnectionValues) => {
    try {
      const connected = await login(values);
      toast.success(`Connected to ${connected.name}`);
    } catch (err) {
      const clean = cleanError(extractError(err, "login failed")) ?? "login failed";
      toast.error(`${values.kind === "local" ? "Local scan" : "Login"}: ${clean}`);
      throw err;
    }
  };

  const handlePickLocalPath = async () => {
    try {
      return await pickLocalFolder();
    } catch (err) {
      toast.error(`Couldn't open folder picker: ${extractError(err, "unknown error")}`);
      return undefined;
    }
  };

  return (
    <div className="flex flex-col gap-8">
      {/* ─── Music Source ────────────────────────────────────── */}
      <SettingsSection label="Music Source">
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
                  onClick={() => void syncLibrary()}
                  disabled={lastSync === "syncing"}
                  className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                >
                  {lastSync === "syncing" ? "Syncing…" : "Sync"}
                </button>
                <button
                  type="button"
                  onClick={() => setAddDialogOpen(true)}
                  disabled={adding}
                  className="flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
                >
                  <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={2} />
                  Add new
                </button>
                <button
                  type="button"
                  onClick={() => void useServerStore.getState().logout()}
                  className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted"
                >
                  Disconnect
                </button>
              </div>
            </div>
            {isLocalActive ? (
              <div className="border-t border-border px-4 py-3">
                <LocalRescanButton />
              </div>
            ) : null}
          </SettingsCard>
        ) : (
          <ServerConnectionForm
            variant="card"
            discovered={discovered}
            error={error}
            onSubmit={handleSubmit}
            onDiscover={() => void useServerStore.getState().discover()}
            onPickLocalPath={handlePickLocalPath}
          />
        )}
      </SettingsSection>

      {/* ─── Saved servers ───────────────────────────────────── */}
      <SettingsSection label="Saved Servers">
        {servers.length === 0 ? (
          <SettingsCard>
            <div className="px-4 py-4 text-sm text-muted-foreground">No saved servers yet.</div>
          </SettingsCard>
        ) : (
          <SettingsCard>
            <ul className="divide-y divide-border">
              {servers.map((s) => (
                <li key={s.id} className="flex items-center justify-between px-4 py-2.5">
                  <div className="flex min-w-0 flex-col">
                    <div className="truncate text-sm font-medium text-foreground">{s.name}</div>
                    <div className="truncate text-xs text-muted-foreground">
                      {s.kind}
                      {s.baseUrl ? ` · ${s.baseUrl}` : ""}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {s.id === activeServerId ? (
                      <span className="inline-flex items-center gap-1 rounded-full bg-primary/15 px-2 py-0.5 text-[11px] font-medium text-primary">
                        <HugeiconsIcon icon={Tick02Icon} size={11} strokeWidth={2.5} />
                        active
                      </span>
                    ) : (
                      <button
                        type="button"
                        onClick={() => void useServerStore.getState().deleteServer(s.id)}
                        className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                        title="Delete server"
                      >
                        <HugeiconsIcon icon={Delete03Icon} size={16} strokeWidth={1.75} />
                      </button>
                    )}
                  </div>
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
              <div className="text-sm font-medium text-foreground">Scrobbling</div>
              <div className="text-xs text-muted-foreground">
                Send a scrobble to Last.fm when a track crosses 50% of its duration.
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
                onClick={() => void onLastfmDisconnect()}
                disabled={lastfmBusy}
                className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
              >
                {lastfmBusy ? "Disconnecting…" : "Disconnect"}
              </button>
            </div>
          ) : (
            <form
              onSubmit={(e) => void onLastfmConnect(e)}
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

      <LoginDialog
        open={addDialogOpen}
        onClose={() => setAddDialogOpen(false)}
        onSubmit={handleAddSubmit}
      />
    </div>
  );
}

function LocalRescanButton() {
  const [busy, setBusy] = useState(false);
  const handleRescan = async () => {
    setBusy(true);
    try {
      const { localRescan } = await import("@/lib/tauri");
      const stats = await localRescan();
      toast.success(
        `Rescanned ${stats.tracks} tracks / ${stats.albums} albums` +
          (stats.errors > 0 ? ` (${stats.errors} file(s) skipped)` : ""),
      );
    } catch (err) {
      toast.error(`Local rescan: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };
  return (
    <button
      type="button"
      onClick={() => void handleRescan()}
      disabled={busy}
      className="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
    >
      {busy ? "Rescanning…" : "Rescan local library"}
    </button>
  );
}
