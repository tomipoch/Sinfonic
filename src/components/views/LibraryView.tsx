// Phase 0 placeholder. Real library view (Albums grid / Artists list /
// Tracks table tabs) lands in Phase 7.

import { NavLink, Outlet } from "react-router-dom";
import { cn } from "../../lib/cn";

const tabs: ReadonlyArray<{ to: string; label: string; end?: boolean }> = [
  { to: "/library", label: "Albums", end: true },
  { to: "/library/artists", label: "Artists" },
  { to: "/library/tracks", label: "Tracks" },
];

export function LibraryView() {
  return (
    <section className="p-6">
      <h1 className="mb-4 text-2xl font-semibold">Library</h1>
      <div className="mb-4 flex gap-2 border-b border-bg-raised">
        {tabs.map((tab) => (
          <NavLink
            key={tab.to}
            to={tab.to}
            end={tab.end}
            className={({ isActive }) =>
              cn(
                "border-b-2 px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "border-accent text-fg"
                  : "border-transparent text-fg-subtle hover:text-fg",
              )
            }
          >
            {tab.label}
          </NavLink>
        ))}
      </div>
      <Outlet />
      <p className="text-fg-muted text-sm">
        Library is empty — connect a server to populate it.
      </p>
    </section>
  );
}
