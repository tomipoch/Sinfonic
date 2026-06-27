// Layout — TitleBar + Sidebar + main outlet + PlayerBar + QueuePanel.

import { useState } from "react";
import { Outlet } from "react-router-dom";
import { PlayerBar } from "./PlayerBar";
import { QueuePanel } from "./QueuePanel";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import { SyncBanner } from "./SyncBanner";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useLibraryAutoLoad } from "@/hooks/useLibraryAutoLoad";
import { cn } from "@/lib/cn";

export function Layout() {
  useKeyboardShortcuts();
  useLibraryAutoLoad();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [queueOpen, setQueueOpen] = useState(false);

  return (
    <div className="flex h-full w-full flex-col">
      <TitleBar
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed((c) => !c)}
      />
      <SyncBanner />
      <div className="relative flex flex-1 overflow-hidden">
        <Sidebar collapsed={sidebarCollapsed} />
        <main
          className={cn(
            "flex min-w-0 flex-1 flex-col transition-all duration-200",
            queueOpen && "mr-80",
          )}
        >
          <div className="flex-1 overflow-auto [overscroll-behavior:contain]">
            <Outlet />
          </div>
          <PlayerBar
            queueOpen={queueOpen}
            onToggleQueue={() => setQueueOpen((v) => !v)}
          />
        </main>
        {queueOpen && (
          <QueuePanel onClose={() => setQueueOpen(false)} />
        )}
      </div>
    </div>
  );
}
