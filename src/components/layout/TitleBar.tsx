import { IS_MAC } from "@/lib/platform";
import { cn } from "@/lib/cn";
import { Search01Icon, Settings01Icon, SidebarLeftIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import type { ReactNode } from "react";

type Props = {
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
};

export function TitleBar({ sidebarCollapsed, onToggleSidebar }: Props) {
  return (
    <div
      className={cn(
        "relative flex h-10 shrink-0 items-center border-b border-border bg-card select-none",
        IS_MAC ? "pl-[100px]" : "pl-2",
      )}
    >
      <div className="flex shrink-0 items-center gap-0.5">
        <TitleBarButton
          ariaLabel="Toggle sidebar"
          onClick={onToggleSidebar}
          className={cn(sidebarCollapsed && "text-muted-foreground/50")}
        >
          <HugeiconsIcon icon={SidebarLeftIcon} size={18} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton
          ariaLabel="Search"
          onClick={() => {}}
        >
          <HugeiconsIcon icon={Search01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>

        <TitleBarButton
          ariaLabel="Settings"
          onClick={() => void invoke("open_settings_window")}
        >
          <HugeiconsIcon icon={Settings01Icon} size={15} strokeWidth={1.75} />
        </TitleBarButton>
      </div>

      <div className="flex-1" data-tauri-drag-region />
    </div>
  );
}

function TitleBarButton({
  ariaLabel,
  onClick,
  children,
  className,
}: {
  ariaLabel: string;
  onClick: () => void;
  children: ReactNode;
  className?: string;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      title={ariaLabel}
      onClick={onClick}
      className={cn(
        "size-7 shrink-0 rounded-md text-muted-foreground transition-colors hover:bg-primary hover:text-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}
