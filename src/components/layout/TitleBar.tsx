// TitleBar — top bar with navigation arrows, view title, and action buttons.
//
// Layout: [←][→] [title (drag region)] [Search] [Settings]

import { IS_MAC } from "@/lib/platform";
import { cn } from "@/lib/cn";
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
  Search01Icon,
  Settings01Icon,
  SidebarLeftIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { useLocation, useNavigate } from "react-router-dom";
import { useEffect, useState, type ReactNode } from "react";

type Props = {
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
};

const ROUTE_TITLES: Record<string, string> = {
  "/": "Home",
  "/library": "Library",
  "/playlists": "Playlists",
  "/favorites": "Favorites",
  "/smart-playlists": "Smart Playlists",
  "/queue": "Queue",
  "/search": "Search",
};

export function TitleBar({ sidebarCollapsed, onToggleSidebar }: Props) {
  const location = useLocation();
  const navigate = useNavigate();
  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoForward, setCanGoForward] = useState(false);

  useEffect(() => {
    const handlePopState = () => {
      setCanGoBack(window.history.state?.idx > 0);
      setCanGoForward(
        window.history.state?.idx < window.history.length - 1,
      );
    };
    handlePopState();
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, []);

  const title = ROUTE_TITLES[location.pathname] ?? "Sinfonic";

  return (
    <div
      className={cn(
        "relative flex h-10 shrink-0 items-center border-b border-border bg-card select-none",
        IS_MAC ? "pl-[100px]" : "pl-2",
      )}
    >
      {/* Navigation arrows */}
      <div className="flex shrink-0 items-center gap-0.5">
        <TitleBarButton
          ariaLabel="Go back"
          onClick={() => navigate(-1)}
          disabled={!canGoBack}
        >
          <HugeiconsIcon icon={ArrowLeft01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton
          ariaLabel="Go forward"
          onClick={() => navigate(1)}
          disabled={!canGoForward}
        >
          <HugeiconsIcon icon={ArrowRight01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>
      </div>

      {/* View title (drag region) */}
      <div
        className="ml-3 mr-2 min-w-0 truncate text-sm font-medium text-foreground data-tauri-drag-region"
        data-tauri-drag-region
      >
        {title}
      </div>

      {/* Spacer */}
      <div className="flex-1" data-tauri-drag-region />

      {/* Action buttons */}
      <div className="flex shrink-0 items-center gap-0.5 pr-2">
        <TitleBarButton
          ariaLabel="Toggle sidebar"
          onClick={onToggleSidebar}
          className={cn(sidebarCollapsed && "text-muted-foreground/50")}
        >
          <HugeiconsIcon icon={SidebarLeftIcon} size={18} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton ariaLabel="Search" onClick={() => {}}>
          <HugeiconsIcon icon={Search01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton
          ariaLabel="Settings"
          onClick={() => void invoke("open_settings_window")}
        >
          <HugeiconsIcon icon={Settings01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>
      </div>
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
        "size-7 shrink-0 rounded-md text-muted-foreground transition-colors hover:bg-primary hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-muted-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}
