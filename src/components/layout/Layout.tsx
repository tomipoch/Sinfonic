// Layout — Sidebar + main outlet + PlayerBar.

import { Outlet } from "react-router-dom";
import { PlayerBar } from "./PlayerBar";
import { Sidebar } from "./Sidebar";
import { useKeyboardShortcuts } from "../../hooks/useKeyboardShortcuts";

export function Layout() {
  useKeyboardShortcuts();

  return (
    <div className="flex h-full w-full">
      <Sidebar />
      <main className="flex min-w-0 flex-1 flex-col">
        <div className="flex-1 overflow-auto">
          <Outlet />
        </div>
        <PlayerBar />
      </main>
    </div>
  );
}
