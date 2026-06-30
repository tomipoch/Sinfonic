// Layout — TitleBar + Sidebar + main outlet + PlayerBar + QueuePanel.
//
// Every major container is tagged with `data-layout-el` so the
// resize probe (mounted further down in this file) can read each
// one's pixel dimensions from the DOM and log them to the console.
// That's what tells us the minimum window size: resize until an
// element reports overflow > 0, then back off ~20 px.

import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useLibraryAutoLoad } from "@/hooks/useLibraryAutoLoad";
import { cn } from "@/lib/cn";
import { PlayerBar } from "./PlayerBar";
import { QueuePanel } from "./QueuePanel";
import { Sidebar } from "./Sidebar";
import { SyncBanner } from "./SyncBanner";
import { TitleBar } from "./TitleBar";

interface ProbeEntry {
  el: string;
  width: number;
  height: number;
  content: number;
  overflow: number;
}

function probeElement(el: HTMLElement): ProbeEntry {
  const name = el.dataset.layoutEl ?? "?";
  const rect = el.getBoundingClientRect();
  return {
    el: name,
    width: Math.round(rect.width),
    height: Math.round(rect.height),
    content: el.scrollWidth,
    overflow: el.scrollWidth - el.clientWidth,
  };
}

export function Layout() {
  useKeyboardShortcuts();
  useLibraryAutoLoad();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [panelMode, setPanelMode] = useState<"closed" | "queue" | "lyrics">("closed");

  const panelOpen = panelMode !== "closed";

  // Top-level layout probe — runs on mount and on every window
  // resize. Walks the DOM for anything tagged with data-layout-el
  // and logs the actual pixel dimensions + overflow so we can pick
  // a sensible minimum window size.
  useEffect(() => {
    const probe = () => {
      const root = document.body;
      const nodes = Array.from(root.querySelectorAll<HTMLElement>("[data-layout-el]"));
      const entries = nodes.map(probeElement);
      // PlayerBar's column widths come from the same DOM but are
      // nested inside the main element — query them separately
      // so the log lists them at the same level as the top-level
      // elements.
      const playerCols = Array.from(root.querySelectorAll<HTMLElement>("[data-pb-col]")).map(
        (el) => ({
          col: el.dataset.pbCol ?? "?",
          width: Math.round(el.getBoundingClientRect().width),
          content: el.scrollWidth,
          overflow: el.scrollWidth - el.clientWidth,
        }),
      );
      console.log("[Layout probe]", {
        viewport: `${window.innerWidth}×${window.innerHeight}`,
        elements: entries,
        playerbar: { columns: playerCols },
      });
    };

    probe();
    window.addEventListener("resize", probe);
    return () => window.removeEventListener("resize", probe);
  }, []);

  return (
    <div className="flex h-full w-full flex-col" data-layout-el="root">
      <div data-layout-el="titlebar">
        <TitleBar
          sidebarCollapsed={sidebarCollapsed}
          onToggleSidebar={() => setSidebarCollapsed((c) => !c)}
        />
      </div>
      <div data-layout-el="syncbanner">
        <SyncBanner />
      </div>
      <div className="relative flex flex-1 overflow-hidden" data-layout-el="main-row">
        <div data-layout-el="sidebar">
          <Sidebar collapsed={sidebarCollapsed} />
        </div>
        <main
          className={cn(
            "flex min-w-0 flex-1 flex-col transition-all duration-200",
            panelOpen && "mr-80",
          )}
          data-layout-el="main"
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
          <div data-layout-el="queuepanel">
            <QueuePanel
              initialMode={panelMode === "lyrics" ? "lyrics" : "queue"}
              onClose={() => setPanelMode("closed")}
            />
          </div>
        )}
      </div>
    </div>
  );
}
