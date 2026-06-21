// Library shell — tabs + outlet. Each tab owns its loading / empty
// states. The library cache is fetched once when the active server
// changes (see `useLibraryAutoLoad`).

import { NavLink, Outlet } from "react-router-dom";

import { cn } from "../../lib/cn";
import { useLibraryAutoLoad } from "../../hooks/useLibraryAutoLoad";
import { useServerStore } from "../../stores/serverStore";

const tabs: ReadonlyArray<{ to: string; label: string; end?: boolean }> = [
  { to: "/library", label: "Albums", end: true },
  { to: "/library/artists", label: "Artists" },
  { to: "/library/tracks", label: "Tracks" },
];

export function LibraryView() {
  useLibraryAutoLoad();
  const activeServerId = useServerStore((s) => s.activeServerId);

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
      {activeServerId ? (
        <Outlet />
      ) : (
        <p className="text-fg-subtle text-sm">
          Connect a server in Settings to populate the library.
        </p>
      )}
    </section>
  );
}
