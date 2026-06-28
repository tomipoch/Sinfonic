// SetupView — fullscreen first-run experience shown when no server is
// connected. Renders the same `ServerConnectionForm` the `LoginDialog`
// uses, but on a dedicated page so the user can read the explanation,
// pick a previously configured source via the "Quick connect" list,
// or fill in credentials / pick a folder before they ever see the
// main UI.
//
// If the user has configured sources from a previous session they are
// surfaced as a "Quick connect" list at the top — clicking a card
// re-attaches the existing source without running the full wizard.
// For local folders this is a near-instant restore (library data and
// album art are already cached on disk).
//
// Once a server is connected the route guard in `App.tsx` swaps the
// user out of `/setup` and into `/`.

import {
  ArrowRight01Icon,
  Delete03Icon,
  HardDriveIcon,
  Link04Icon,
  Tick02Icon,
  Wifi01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  type ConnectionValues,
  ServerConnectionForm,
} from "@/components/dialogs/ServerConnectionForm";
import { SettingsCard, SettingsSection } from "@/components/primitives/primitives";
import { pickLocalFolder } from "@/lib/dialogs";
import { extractError } from "@/lib/errors";
import { useServerStore } from "@/stores/serverStore";
import { makeLogger } from "@/utils/log";

const log = makeLogger("SetupView");

function SourceIcon({ kind }: { kind: string }) {
  if (kind === "local") {
    return <HugeiconsIcon icon={HardDriveIcon} size={18} strokeWidth={1.5} />;
  }
  if (kind === "subsonic") {
    return <HugeiconsIcon icon={Wifi01Icon} size={18} strokeWidth={1.5} />;
  }
  return <HugeiconsIcon icon={Link04Icon} size={18} strokeWidth={1.5} />;
}

export function SetupView() {
  const navigate = useNavigate();
  const [connectingId, setConnectingId] = useState<string | null>(null);
  const [connectError, setConnectError] = useState<string | null>(null);

  const servers = useServerStore((s) => s.servers);
  const discovered = useServerStore((s) => s.discovered);
  const discoveredError = useServerStore((s) => s.error);
  const setPendingConnection = useServerStore((s) => s.setPendingConnection);
  const discover = useServerStore((s) => s.discover);

  // The form does no async work itself: it stores the desired
  // connection in the store and hands off to /loading, which owns
  // the login/scan/sync lifecycle and its progress UI.
  const handleSubmit = (values: ConnectionValues) => {
    log.log("form submit", values);
    setPendingConnection(values);
    void navigate("/loading", { replace: true });
  };

  // Attach to a previously configured source without going through
  // the wizard. For local files this is fast: the library rows are
  // already in SQLite and the album art is in the filesystem cache.
  const handleQuickConnect = async (serverId: string) => {
    log.log("quick connect", serverId);
    setConnectError(null);
    setConnectingId(serverId);
    try {
      await useServerStore.getState().setActive(serverId);
      void navigate("/loading", { replace: true });
    } catch (e) {
      const msg = extractError(e, "could not connect");
      log.error("quick connect failed", serverId, msg);
      setConnectError(msg);
    } finally {
      setConnectingId(null);
    }
  };

  const handlePickLocalPath = async () => {
    try {
      return await pickLocalFolder();
    } catch (err) {
      setConnectError(`Couldn't open folder picker: ${extractError(err, "unknown error")}`);
      return undefined;
    }
  };

  return (
    <div className="flex h-full w-full items-stretch justify-center overflow-auto [overscroll-behavior:contain]">
      <div className="flex w-full max-w-3xl flex-col gap-10 p-8 md:p-12">
        <header className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-lg bg-primary text-primary-foreground">
              <HugeiconsIcon icon={Tick02Icon} size={20} strokeWidth={2.5} />
            </div>
            <h1 className="text-3xl font-semibold tracking-tight text-foreground">
              Welcome to Sinfonic
            </h1>
          </div>
          <p className="max-w-prose text-sm text-muted-foreground">
            Pick a music source to get started. You can add more servers later from the source
            selector at the bottom of the sidebar.
          </p>
        </header>

        {servers.length > 0 ? (
          <SettingsSection label="Quick connect">
            <p className="text-xs text-muted-foreground">
              Re-attach an existing source. Local folders load straight from the on-disk cache.
            </p>
            <SettingsCard>
              <ul className="divide-y divide-border">
                {servers.map((s) => {
                  const busy = connectingId === s.id;
                  return (
                    <li key={s.id} className="flex items-center justify-between gap-3 px-4 py-3">
                      <div className="flex min-w-0 items-center gap-3">
                        <span
                          className="flex size-9 shrink-0 items-center justify-center rounded-md border border-border bg-background text-foreground/80"
                          aria-hidden
                        >
                          <SourceIcon kind={s.kind} />
                        </span>
                        <div className="flex min-w-0 flex-col">
                          <div className="truncate text-sm font-medium text-foreground">
                            {s.name}
                          </div>
                          <div className="truncate font-mono text-xs text-muted-foreground">
                            {s.baseUrl ?? s.id}
                          </div>
                        </div>
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        <button
                          type="button"
                          onClick={() => void handleQuickConnect(s.id)}
                          disabled={connectingId !== null}
                          className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                        >
                          {busy ? (
                            "Connecting…"
                          ) : (
                            <>
                              Connect
                              <HugeiconsIcon icon={ArrowRight01Icon} size={14} strokeWidth={2} />
                            </>
                          )}
                        </button>
                        <button
                          type="button"
                          onClick={() => void useServerStore.getState().deleteServer(s.id)}
                          disabled={connectingId !== null}
                          className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
                          title="Delete server"
                        >
                          <HugeiconsIcon icon={Delete03Icon} size={16} strokeWidth={1.75} />
                        </button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </SettingsCard>
            {connectError ? (
              <div
                role="alert"
                className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
              >
                {connectError.replace(/^provider_set_active:\s*/i, "")}
              </div>
            ) : null}
          </SettingsSection>
        ) : null}

        <ServerConnectionForm
          variant="page"
          discovered={discovered}
          error={discoveredError}
          onSubmit={handleSubmit}
          onDiscover={() => void discover()}
          onPickLocalPath={handlePickLocalPath}
        />
      </div>
    </div>
  );
}
