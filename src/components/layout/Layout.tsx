// Layout — TitleBar + Sidebar + main outlet + PlayerBar + QueuePanel.
//
// The right-side QueuePanel reads its mode (queue / lyrics) from
// the queue store so the PlayerBar's panel toggles and the panel
// itself stay in sync without prop drilling. Layout only owns the
// `panelOpen` boolean (derived from the store) for the mr-56 margin
// and the conditional render.
//
// Every major container is tagged with `data-layout-el` so the
// resize probe (mounted further down in this file) can read each
// one's pixel dimensions and log them to the console. That's what
// tells us the minimum window size: resize until an element reports
// overflow > 0, then back off ~20 px.

import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useLibraryAutoLoad } from "@/hooks/useLibraryAutoLoad";
import { cn } from "@/lib/cn";
import { useQueueStore } from "@/stores/queueStore";
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
  const panelMode = useQueueStore((s) => s.panelMode);
  const setPanelMode = useQueueStore((s) => s.setPanelMode);

  const panelOpen = panelMode !== null;

  const toggleQueue = () => setPanelMode(panelMode === "queue" ? null : "queue");
  const toggleLyrics = () => setPanelMode(panelMode === "lyrics" ? null : "lyrics");

  // Top-level layout probe — runs on mount and on every window
  // resize. Walks the DOM for anything tagged with data-layout-el
  // and logs the actual pixel dimensions + overflow so we can pick
  // a sensible minimum window size. Gated behind import.meta.env.DEV
  // so production consoles stay clean.
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const probe = () => {
      const root = document.body;
      const nodes = Array.from(root.querySelectorAll<HTMLElement>("[data-layout-el]"));
      const entries = nodes.map(probeElement);
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
            panelOpen && "mr-56",
          )}
          data-layout-el="main"
          data-panels-open={panelOpen || undefined}
        >
          <div className="flex-1 overflow-auto [overscroll-behavior:contain]">
            <Outlet />
          </div>
          <PlayerBar
            queueOpen={panelMode === "queue"}
            onToggleQueue={toggleQueue}
            lyricsOpen={panelMode === "lyrics"}
            onToggleLyrics={toggleLyrics}
          />
        </main>
        {panelOpen && (
          <div data-layout-el="queuepanel">
            <QueuePanel />
          </div>
        )}
      </div>
    </div>
  );
}
