// SourceSelector — dropdown at the bottom of the sidebar for quick
// server switching. Opens LoginDialog when no server is connected or
// when "Add new server..." is selected.

import { useRef, useState } from "react";
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

const KIND_ICONS = {
  jellyfin: Link04Icon,
  subsonic: Wifi01Icon,
  local: HardDriveIcon,
} as const;

export function SourceSelector() {
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const activeServer = servers.find((s) => s.id === activeServerId);

  const [open, setOpen] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const handleSelectServer = (_serverId: string) => {
    setOpen(false);
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
                  return (
                    <li key={server.id}>
                      <button
                        type="button"
                        onClick={() => handleSelectServer(server.id)}
                        className="flex w-full items-center gap-2 px-3 py-2 text-sm transition-colors hover:bg-muted"
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
                        {isActive && (
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
            </div>
          </>
        )}
      </div>

      <LoginDialog open={dialogOpen} onClose={handleCloseDialog} />
    </>
  );
}
