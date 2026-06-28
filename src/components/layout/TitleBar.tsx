// TitleBar — top bar integrating with the OS window chrome.
//
// Layout:
//   [traffic-light overlay on macOS / custom window controls on Win+Linux]
//   [←][→] [drag region] [Search] [Settings]   ← when expanded on macOS
//   [][][] [drag region] [Search] [Settings]   ← when expanded on Win/Linux
//
// On macOS, the OS draws the traffic lights on top of the title bar as an
// overlay (configured via `titleBarStyle: "Overlay"` + `hiddenTitle: true`
// in tauri.conf.json). The first ~80px is reserved for them with `pl-20`.
// On Windows and Linux we disable the native decorations (see
// tauri.windows.conf.json / tauri.linux.conf.json) and paint our own
// minimize/maximize/close buttons on the right.

import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
  Cancel01Icon,
  Search01Icon,
  Settings01Icon,
  SidebarLeftIcon,
  SquareIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ReactNode } from "react";
import { useHistoryNav } from "@/hooks/useHistoryNav";
import { cn } from "@/lib/cn";
import { IS_LINUX, IS_MAC, IS_WINDOWS } from "@/lib/platform";

type Props = {
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
};

export function TitleBar({ sidebarCollapsed, onToggleSidebar }: Props) {
  const { goBack, goForward, canGoBack, canGoForward } = useHistoryNav();

  return (
    <div
      className={cn(
        "relative flex h-10 shrink-0 items-center border-b border-border bg-card select-none",
        IS_MAC && "pl-20",
      )}
    >
      {/* Sidebar toggle + Navigation arrows (not draggable) */}
      <div className="flex shrink-0 items-center gap-0.5">
        <TitleBarButton
          ariaLabel="Toggle sidebar"
          onClick={onToggleSidebar}
          className={cn(sidebarCollapsed && "text-muted-foreground/50")}
        >
          <HugeiconsIcon icon={SidebarLeftIcon} size={18} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton ariaLabel="Go back" onClick={goBack} disabled={!canGoBack}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton ariaLabel="Go forward" onClick={goForward} disabled={!canGoForward}>
          <HugeiconsIcon icon={ArrowRight01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>
      </div>

      {/* Spacer (draggable). On macOS with the native traffic-light overlay,
          any non-control area is automatically draggable by the OS. On
          Windows/Linux with `decorations: false`, the OS relies on the
          explicit `data-tauri-drag-region` attribute to know where to drag. */}
      <div className="flex-1" data-tauri-drag-region />

      {/* Action buttons + native window controls (Win/Linux) */}
      <div className="flex shrink-0 items-center gap-0.5 pr-2">
        <TitleBarButton ariaLabel="Search" onClick={() => {}}>
          <HugeiconsIcon icon={Search01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton ariaLabel="Settings" onClick={() => void invoke("open_settings_window")}>
          <HugeiconsIcon icon={Settings01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>

        {(IS_WINDOWS || IS_LINUX) && <WindowsControls />}
      </div>
    </div>
  );
}

function WindowsControls() {
  const appWindow = getCurrentWindow();

  return (
    <div className="ml-2 flex shrink-0 items-center">
      <TitleBarButton ariaLabel="Minimize" onClick={() => void appWindow.minimize()}>
        <svg viewBox="0 0 12 12" className="h-2.5 w-2.5" fill="currentColor" aria-hidden>
          <rect y="5" width="12" height="1.5" />
        </svg>
      </TitleBarButton>
      <TitleBarButton ariaLabel="Maximize" onClick={() => void appWindow.toggleMaximize()}>
        <HugeiconsIcon icon={SquareIcon} size={12} strokeWidth={1.75} />
      </TitleBarButton>
      <TitleBarButton
        ariaLabel="Close"
        onClick={() => void appWindow.close()}
        className="hover:bg-red-500 hover:text-white"
      >
        <HugeiconsIcon icon={Cancel01Icon} size={14} strokeWidth={1.75} />
      </TitleBarButton>
    </div>
  );
}

function TitleBarButton({
  ariaLabel,
  onClick,
  children,
  className,
  disabled,
}: {
  ariaLabel: string;
  onClick: () => void;
  children: ReactNode;
  className?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      title={ariaLabel}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-primary hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-muted-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}
