// LoginDialog — modal dialog for quick server connection.
//
// Opens inline from empty states or SourceSelector when no server is
// connected. Shares form logic with ServerManager via `useServerForms`.

import { useRef, useState, type FormEvent } from "react";
import {
  HardDriveIcon,
  Link04Icon,
  Tick02Icon,
  Wifi01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

import { useServerForms } from "@/hooks/useServerForms";
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

interface Props {
  open: boolean;
  onClose: () => void;
}

export function LoginDialog({ open, onClose }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [dismissed, setDismissed] = useState(false);

  const {
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
    discovered,
    activeServer,
    isLocalSource,
    activeServerId,
    onDiscover,
    onRemoteLogin,
    onLocalScan,
    onLogout,
  } = useServerForms();

  if (!open || dismissed) return null;

  const isConnected = activeServerId !== null;

  const handleClose = () => {
    setDismissed(true);
    onClose();
  };

  const handleFormSubmit = async (e: FormEvent) => {
    if (isLocalSource) {
      await onLocalScan(e);
    } else {
      await onRemoteLogin(e);
    }
    if (activeServerId) {
      handleClose();
    }
  };

  return (
    <dialog
      ref={dialogRef}
      open={open}
      onClose={handleClose}
      className="fixed inset-0 z-50 m-0 min-h-screen w-screen bg-black/60 backdrop-blur-sm"
      aria-modal="true"
      aria-label="Connect to server"
    >
      <div className="absolute inset-0" onClick={handleClose} />
      <div className="relative mx-auto mt-[15vh] w-full max-w-lg rounded-xl border border-border bg-card p-0 shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-6 py-4">
          <h2 className="text-base font-semibold text-foreground">
            Connect to server
          </h2>
          <button
            type="button"
            onClick={handleClose}
            className="size-7 rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        {/* Content */}
        <div className="max-h-[60vh] overflow-y-auto p-6">
          {isConnected ? (
            <div className="flex flex-col gap-4">
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-col gap-0.5">
                  <div className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                    Currently connected
                  </div>
                  <div className="truncate text-base font-medium text-foreground">
                    {activeServer?.name ?? activeServerId}
                  </div>
                </div>
                <span className="inline-flex items-center gap-1 rounded-full bg-primary/15 px-2 py-0.5 text-[11px] font-medium text-primary">
                  <HugeiconsIcon icon={Tick02Icon} size={11} strokeWidth={2.5} />
                  active
                </span>
              </div>
              <button
                type="button"
                onClick={() => {
                  void onLogout();
                  handleClose();
                }}
                className="w-fit rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted"
              >
                Disconnect
              </button>
            </div>
          ) : (
            <div className="flex flex-col gap-6">
              {/* Source selector */}
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
              </SettingsSection>

              {/* Connection form */}
              {isLocalSource ? (
                <SettingsSection label="Local folder">
                  <SettingsCard>
                    <form onSubmit={handleFormSubmit} className="flex flex-col gap-3 px-4 py-4">
                      <div className="text-xs text-muted-foreground">
                        Point Sinfonic at a directory of audio files.
                      </div>
                      <label className="flex flex-col gap-1 text-sm">
                        <span className="text-muted-foreground">Music folder</span>
                        <input
                          type="text"
                          value={localPath}
                          onChange={(e) => setLocalPath(e.currentTarget.value)}
                          placeholder="/Users/you/Music"
                          required
                          spellCheck={false}
                          className="rounded-md border border-input bg-background px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
                        />
                      </label>
                      <button
                        type="submit"
                        disabled={localBusy}
                        className="w-fit rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                      >
                        {localBusy ? "Scanning…" : "Scan library"}
                      </button>
                    </form>
                  </SettingsCard>
                </SettingsSection>
              ) : (
                <SettingsSection label="Server">
                  <SettingsCard>
                    <form onSubmit={handleFormSubmit} className="flex flex-col gap-3 px-4 py-4">
                      <div className="text-xs text-muted-foreground">
                        {source === "jellyfin"
                          ? "Jellyfin supports auto-discovery on the local network."
                          : "Subsonic / Navidrome / Funkwhale — manual entry."}
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
                      <button
                        type="submit"
                        disabled={busy}
                        className="w-fit rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                      >
                        {busy ? "Connecting…" : "Connect"}
                      </button>
                    </form>
                  </SettingsCard>
                </SettingsSection>
              )}

              {/* Discovery */}
              {source === "jellyfin" && !isConnected && (
                <SettingsSection label="Discovery">
                  <SettingsCard>
                    <div className="flex items-center justify-between gap-4 px-4 py-4">
                      <div className="min-w-0 flex-col gap-0.5">
                        <div className="text-sm font-medium text-foreground">
                          Local network
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {discovered.length === 0
                            ? "No servers detected yet."
                            : `${discovered.length} server${discovered.length === 1 ? "" : "s"} found.`}
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => void onDiscover()}
                        disabled={discovering}
                        className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
                      >
                        {discovering ? "Scanning…" : "Scan"}
                      </button>
                    </div>
                  </SettingsCard>
                  {discovered.length > 0 && (
                    <SettingsCard>
                      <ul className="divide-y divide-border">
                        {discovered.map((d) => (
                          <li
                            key={d.serverId}
                            className="flex items-center justify-between gap-3 px-4 py-2.5"
                          >
                            <div className="min-w-0 flex-col">
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
                  )}
                </SettingsSection>
              )}
            </div>
          )}
        </div>
      </div>
    </dialog>
  );
}
