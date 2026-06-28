// Layout — TitleBar + Sidebar + main outlet + PlayerBar + QueuePanel.

import { useState } from "react";
import { Outlet } from "react-router-dom";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useLibraryAutoLoad } from "@/hooks/useLibraryAutoLoad";
import { cn } from "@/lib/cn";
import { PlayerBar } from "./PlayerBar";
import { QueuePanel } from "./QueuePanel";
import { Sidebar } from "./Sidebar";
import { SyncBanner } from "./SyncBanner";
import { TitleBar } from "./TitleBar";

export function Layout() {
  useKeyboardShortcuts();
  useLibraryAutoLoad();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [panelMode, setPanelMode] = useState<"closed" | "queue" | "lyrics">("closed");

  const panelOpen = panelMode !== "closed";

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
            panelOpen && "mr-80",
          )}
        >
          <div className="flex-1 overflow-auto [overscroll-behavior:contain]">
            <Outlet />
          </div>
          <PlayerBar
            queueOpen={panelMode === "queue"}
            onToggleQueue={() => setPanelMode((m) => (m === "queue" ? "closed" : "queue"))}
            lyricsOpen={panelMode === "lyrics"}
            onToggleLyrics={() => setPanelMode((m) => (m === "lyrics" ? "closed" : "lyrics"))}
          />
        </main>
        {panelOpen && (
          <QueuePanel
            initialMode={panelMode === "lyrics" ? "lyrics" : "queue"}
            onClose={() => setPanelMode("closed")}
          />
        )}
      </div>
    </div>
  );
}
