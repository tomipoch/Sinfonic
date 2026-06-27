// SourceSelector — dropdown at the bottom of the sidebar for quick
// server switching. Opens LoginDialog when no server is connected or
// when "Add new server..." is selected.

import { useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Add01Icon,
  HardDriveIcon,
  Link04Icon,
  Tick02Icon,
  Wifi01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

import { LoginDialog } from "@/components/dialogs/LoginDialog";
import { useServerStore } from "@/stores/serverStore";
import { extractError } from "@/lib/errors";
import { makeLogger } from "@/utils/log";

const log = makeLogger("SourceSelector");

const KIND_ICONS = {
  jellyfin: Link04Icon,
  subsonic: Wifi01Icon,
  local: HardDriveIcon,
} as const;

export function SourceSelector() {
  const navigate = useNavigate();
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const activeServer = servers.find((s) => s.id === activeServerId);
  const setActive = useServerStore((s) => s.setActive);

  const [open, setOpen] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const [switchError, setSwitchError] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const handleSelectServer = async (serverId: string) => {
    setOpen(false);
    if (serverId === activeServerId) return;
    log.log("switching source", serverId);
    setSwitching(serverId);
    setSwitchError(null);
    try {
      await setActive(serverId);
      // Hand off to the loading route so the user sees the cache /
      // sync progress for the newly-active source instead of a
      // blank intermediate state.
      void navigate("/loading", { replace: true });
    } catch (e) {
      const msg = extractError(e, "could not switch source");
      log.error("switch failed", serverId, msg);
      setSwitchError(msg);
    } finally {
      setSwitching(null);
    }
  };

  const handleAddNew = () => {
    setOpen(false);
    setDialogOpen(true);
  };

  const handleCloseDialog = () => {
    setDialogOpen(false);
  };

  return (
    <>
      <div ref={dropdownRef} className="relative">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors hover:bg-muted"
          aria-expanded={open}
          aria-haspopup="listbox"
        >
          {activeServer ? (
            <>
              <span className="size-2 rounded-full bg-primary" />
              <HugeiconsIcon
                icon={KIND_ICONS[activeServer.kind] ?? Link04Icon}
                size={14}
                strokeWidth={1.75}
              />
              <span className="min-w-0 flex-1 truncate text-left text-foreground">
                {activeServer.name}
              </span>
            </>
          ) : (
            <>
              <span className="size-2 rounded-full bg-muted-foreground" />
              <span className="min-w-0 flex-1 truncate text-left text-muted-foreground">
                Connect server
              </span>
            </>
          )}
        </button>

        {open && (
          <>
            <div
              className="fixed inset-0 z-40"
              onClick={() => setOpen(false)}
            />
            <div className="absolute bottom-full left-0 right-0 z-50 mb-1 overflow-hidden rounded-md border border-border bg-card shadow-lg">
              <ul role="listbox" className="py-1">
                {servers.map((server) => {
                  const isActive = server.id === activeServerId;
                  const isSwitching = switching === server.id;
                  return (
                    <li key={server.id}>
                      <button
                        type="button"
                        disabled={isSwitching}
                        onClick={() => handleSelectServer(server.id)}
                        className="flex w-full items-center gap-2 px-3 py-2 text-sm transition-colors hover:bg-muted disabled:opacity-60"
                      >
                        {isActive ? (
                          <span className="size-2 rounded-full bg-primary" />
                        ) : (
                          <span className="size-2" />
                        )}
                        <HugeiconsIcon
                          icon={KIND_ICONS[server.kind] ?? Link04Icon}
                          size={13}
                          strokeWidth={1.75}
                          className="text-muted-foreground"
                        />
                        <span className="min-w-0 flex-1 truncate text-foreground">
                          {server.name}
                        </span>
                        {isSwitching && (
                          <span className="text-[11px] text-muted-foreground">
                            switching…
                          </span>
                        )}
                        {isActive && !isSwitching && (
                          <HugeiconsIcon
                            icon={Tick02Icon}
                            size={12}
                            strokeWidth={2}
                            className="text-primary"
                          />
                        )}
                      </button>
                    </li>
                  );
                })}
                <li className="border-t border-border">
                  <button
                    type="button"
                    onClick={handleAddNew}
                    className="flex w-full items-center gap-2 px-3 py-2 text-sm transition-colors hover:bg-muted"
                  >
                    <span className="size-2" />
                    <HugeiconsIcon
                      icon={Add01Icon}
                      size={13}
                      strokeWidth={1.75}
                      className="text-muted-foreground"
                    />
                    <span className="text-muted-foreground">
                      Add new server…
                    </span>
                  </button>
                </li>
              </ul>
              {switchError && (
                <div className="border-t border-border px-3 py-2 text-[11px] text-destructive">
                  {switchError}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      <LoginDialog open={dialogOpen} onClose={handleCloseDialog} />
    </>
  );
}
