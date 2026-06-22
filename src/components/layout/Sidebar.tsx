// Sidebar — left navigation rail. Uses React Router's NavLink for
// active-state styling.

import { NavLink } from "react-router-dom";
import { cn } from "../../lib/cn";

const links: ReadonlyArray<{ to: string; label: string; end?: boolean }> = [
  { to: "/", label: "Home", end: true },
  { to: "/library", label: "Library" },
  { to: "/playlists", label: "Playlists" },
  { to: "/queue", label: "Queue" },
  { to: "/search", label: "Search" },
  { to: "/settings", label: "Settings" },
];

export function Sidebar() {
  return (
    <aside className="flex h-full w-56 flex-col gap-1 border-r border-bg-raised bg-bg-subtle p-3">
      <div className="px-2 pb-3 text-lg font-semibold text-fg">Sinfonic</div>
      <nav className="flex flex-col gap-1">
        {links.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            end={link.end}
            className={({ isActive }) =>
              cn(
                "rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-bg-raised text-fg"
                  : "text-fg-subtle hover:bg-bg-raised hover:text-fg",
              )
            }
          >
            {link.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
