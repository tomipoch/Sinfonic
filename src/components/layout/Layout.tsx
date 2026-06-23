// Layout — TitleBar + Sidebar + main outlet + PlayerBar.

import { useState } from "react";
import { Outlet } from "react-router-dom";
import { PlayerBar } from "./PlayerBar";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";

export function Layout() {
  useKeyboardShortcuts();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  return (
    <div className="flex h-full w-full flex-col">
      <TitleBar
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed((c) => !c)}
      />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar collapsed={sidebarCollapsed} />
        <main className="flex min-w-0 flex-1 flex-col">
          <div className="flex-1 overflow-auto">
            <Outlet />
          </div>
          <PlayerBar />
        </main>
      </div>
    </div>
  );
}
