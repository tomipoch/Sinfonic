// ServerConnectionForm — shared connection form for Jellyfin / Subsonic /
// Local files. The form owns its own input state (source picker, base URL,
// username, password, local path) and emits a discriminated `ConnectionValues`
// payload on submit, leaving handoff / progress / error policy to the caller.
//
// Variants:
//   - `page`  — SetupView: full-width, helper text below the source picker,
//               "Continue" submit label.
//   - `modal` — LoginDialog: tighter density, helper text inside the form
//               card, discovery collapsed into a single scan button.
//   - `card`  — ServerManager (settings): rendered inside a SettingsCard
//               wrapper provided by the parent; this variant is denser and
//               shows the sync status block.
//
// Discovery and folder picker callbacks are optional so the modal variant
// can omit them when the parent hasn't configured Jellyfin discovery.
//
// React 19 idioms in play:
//   * `<form action={handleSubmit}>` — React 19 wraps the action in a
//     transition automatically and exposes pending state via
//     `useFormStatus()` so the submit button shows a spinner without
//     local busy state.
//   * `useTransition` is not used because there's no UI work to defer
//     during the submit — the IPC round-trip happens entirely inside
//     the parent's `onSubmit` callback.

import { HardDriveIcon, Link04Icon, Wifi01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { useFormStatus } from "react-dom";

import { ChoiceCard, SettingsCard, SettingsSection } from "@/components/primitives/primitives";
import { cleanError } from "@/lib/errors";
import type { DiscoveredServer } from "@/types/domain";

export type ConnectionSource = "jellyfin" | "subsonic" | "local";

export type ConnectionValues =
  | { kind: "jellyfin"; baseUrl: string; username: string; password: string }
  | { kind: "subsonic"; baseUrl: string; username: string; password: string }
  | { kind: "local"; path: string };

export interface ServerConnectionFormProps {
  variant: "page" | "modal" | "card";
  initialSource?: ConnectionSource;
  discovered?: DiscoveredServer[];
  busy?: boolean;
  error?: string | null | undefined;
  onSubmit: (values: ConnectionValues) => Promise<void> | void;
  onDiscover?: () => Promise<void> | void;
  onPickLocalPath?: () => Promise<string | undefined> | undefined;
}

const SOURCES: {
  id: ConnectionSource;
  label: string;
  icon: typeof Link04Icon;
  blurb: string;
}[] = [
  {
    id: "jellyfin",
    label: "Jellyfin",
    icon: Link04Icon,
    blurb: "Sign in to a Jellyfin server. Supports LAN discovery.",
  },
  {
    id: "subsonic",
    label: "Subsonic",
    icon: Wifi01Icon,
    blurb: "Subsonic, Navidrome, Funkwhale, Airsonic. Manual URL only.",
  },
  {
    id: "local",
    label: "Local files",
    icon: HardDriveIcon,
    blurb: "Point Sinfonic at a folder of audio files on this machine.",
  },
];

/// `Url::parse` on the Rust side rejects inputs without a scheme, so
/// we prepend `http://` to bare hosts client-side first. An existing
/// scheme is preserved.
function normaliseBaseUrl(raw: string): string {
  if (!raw) return raw;
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(raw)) return raw;
  return `http://${raw}`;
}

export function ServerConnectionForm({
  variant,
  initialSource = "jellyfin",
  discovered,
  busy = false,
  error,
  onSubmit,
  onDiscover,
  onPickLocalPath,
}: ServerConnectionFormProps) {
  const [source, setSource] = useState<ConnectionSource>(initialSource);
  const [baseUrl, setBaseUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [discovering, setDiscovering] = useState(false);

  const isLocal = source === "local";
  const submitLabel = isLocal ? "Continue" : "Connect";

  // React 19 form action: receives FormData directly. The latest
  // controlled values are also reflected in state, so we read from
  // FormData first (which captures the DOM value at submit time,
  // including any pending IME composition that hasn't fired onChange).
  const handleSubmit = async (formData: FormData) => {
    const path = (formData.get("localPath") as string | null)?.trim() ?? localPath.trim();
    const url = (formData.get("baseUrl") as string | null)?.trim() ?? baseUrl.trim();
    const user = (formData.get("username") as string | null)?.trim() ?? username.trim();
    const pass = (formData.get("password") as string | null) ?? password;

    if (isLocal) {
      if (!path) return;
      await onSubmit({ kind: "local", path });
      return;
    }

    const normalised = normaliseBaseUrl(url);
    if (!normalised || !user || !pass) return;
    await onSubmit({
      kind: source,
      baseUrl: normalised,
      username: user,
      password: pass,
    });
  };

  const handleDiscover = async () => {
    if (!onDiscover) return;
    setDiscovering(true);
    try {
      await onDiscover();
    } finally {
      setDiscovering(false);
    }
  };

  const renderSourcePicker = (cols: string) => (
    <SettingsSection label={variant === "modal" ? "Music Source" : "Music source"}>
      <div className={`grid gap-2 ${cols}`}>
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
      {variant === "page" ? (
        <p className="mt-2 text-xs text-muted-foreground">
          {SOURCES.find((s) => s.id === source)?.blurb}
        </p>
      ) : null}
    </SettingsSection>
  );

  const renderError = () => {
    const msg = cleanError(error);
    if (!msg) return null;
    return (
      <div
        role="alert"
        className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
      >
        {msg}
      </div>
    );
  };

  const renderLocalForm = () => (
    <SettingsSection label={variant === "modal" ? "Local folder" : "Music folder"}>
      <SettingsCard>
        <form action={handleSubmit} className="flex flex-col gap-3 px-4 py-4">
          {variant === "modal" ? (
            <div className="text-xs text-muted-foreground">
              Point Sinfonic at a directory of audio files.
            </div>
          ) : variant === "page" ? (
            <div className="text-xs text-muted-foreground">
              Sinfonic will walk the directory recursively and read metadata from the file tags
              (ID3, Vorbis, MP4, …).
            </div>
          ) : (
            <div className="text-xs text-muted-foreground">
              Point Sinfonic at a directory of audio files (MP3, FLAC, OGG, Opus, MP4/M4A, WAV). The
              directory is walked recursively; metadata comes from the file tags.
            </div>
          )}
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-muted-foreground">
              {variant === "modal" ? "Music folder" : "Folder path"}
            </span>
            <div className="flex gap-2">
              <input
                type="text"
                name="localPath"
                value={localPath}
                onChange={(e) => setLocalPath(e.currentTarget.value)}
                placeholder="/Users/you/Music"
                required
                spellCheck={false}
                autoCorrect="off"
                autoCapitalize="off"
                className="flex-1 rounded-md border border-input bg-background px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
              />
              {onPickLocalPath ? (
                <button
                  type="button"
                  onClick={async () => {
                    const picked = await onPickLocalPath();
                    if (typeof picked === "string") setLocalPath(picked);
                  }}
                  className="shrink-0 rounded-md border border-border px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted"
                >
                  Browse…
                </button>
              ) : null}
            </div>
          </label>
          {renderError()}
          <SubmitButton busy={busy} variant="primary">
            {submitLabel}
          </SubmitButton>
        </form>
      </SettingsCard>
    </SettingsSection>
  );

  const renderRemoteForm = () => (
    <SettingsSection label="Server">
      <SettingsCard>
        <form action={handleSubmit} className="flex flex-col gap-3 px-4 py-4">
          {variant === "modal" ? (
            <div className="text-xs text-muted-foreground">
              {source === "jellyfin"
                ? "Jellyfin supports auto-discovery on the local network."
                : "Subsonic / Navidrome / Funkwhale — manual entry."}
            </div>
          ) : variant === "page" ? null : (
            <div className="text-xs text-muted-foreground">
              {source === "jellyfin"
                ? "Jellyfin supports both auto-discovery on the local network and manual entry."
                : "Subsonic / Navidrome / Funkwhale — manual entry only. Salt and token are computed per request, so your password never leaves the device."}
            </div>
          )}
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-muted-foreground">Server URL</span>
            <input
              type="url"
              name="baseUrl"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              placeholder={
                source === "jellyfin" ? "http://192.168.1.10:8096" : "http://192.168.1.10:4533"
              }
              required
              className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
            />
          </label>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <label className="flex flex-col gap-1 text-sm">
              <span className="text-muted-foreground">Username</span>
              <input
                type="text"
                name="username"
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
                name="password"
                value={password}
                onChange={(e) => setPassword(e.currentTarget.value)}
                required
                autoComplete="current-password"
                className="rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none"
              />
            </label>
          </div>
          {renderError()}
          <div className="flex flex-wrap items-center gap-3">
            <SubmitButton busy={busy} variant="primary">
              {submitLabel}
            </SubmitButton>
            {source === "jellyfin" && onDiscover ? (
              <button
                type="button"
                onClick={() => void handleDiscover()}
                disabled={discovering}
                className="rounded-md border border-border px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
              >
                {discovering ? "Scanning…" : variant === "page" ? "Scan local network" : "Scan"}
              </button>
            ) : null}
          </div>
        </form>
      </SettingsCard>

      {source === "jellyfin" && discovered && discovered.length > 0 ? (
        <SettingsCard>
          <ul className="divide-y divide-border">
            {discovered.map((d) => (
              <li key={d.serverId} className="flex items-center justify-between gap-3 px-4 py-2.5">
                <div className="flex min-w-0 flex-col">
                  <div className="truncate text-sm font-medium text-foreground">{d.name}</div>
                  <div className="truncate text-xs text-muted-foreground">{d.baseUrl}</div>
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
  );

  const gridCols = variant === "modal" ? "grid-cols-3" : "grid-cols-1 sm:grid-cols-3";

  return (
    <div className="flex flex-col gap-6">
      {renderSourcePicker(gridCols)}
      {isLocal ? renderLocalForm() : renderRemoteForm()}
    </div>
  );
}

function SubmitButton({
  busy,
  variant,
  children,
}: {
  busy: boolean;
  variant: "primary";
  children: React.ReactNode;
}) {
  // `useFormStatus` only works inside a `<form>` — this component
  // must be a child of the form whose status it reports.
  const { pending } = useFormStatus();
  const disabled = busy || pending;
  const label = pending ? "Connecting…" : children;
  return (
    <button
      type="submit"
      disabled={disabled}
      className={
        variant === "primary"
          ? "rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
          : ""
      }
    >
      {label}
    </button>
  );
}
