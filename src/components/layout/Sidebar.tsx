// Sidebar — left navigation rail. Uses React Router's NavLink for
// active-state styling. Supports collapse via `collapsed` prop.

import { NavLink } from "react-router-dom";
import { cn } from "@/lib/cn";

const links: ReadonlyArray<{ to: string; label: string; end?: boolean }> = [
  { to: "/", label: "Home", end: true },
  { to: "/library", label: "Library" },
  { to: "/playlists", label: "Playlists" },
  { to: "/favorites", label: "Favorites" },
  { to: "/smart-playlists", label: "Smart Playlists" },
];

type Props = {
  collapsed?: boolean;
};

export function Sidebar({ collapsed = false }: Props) {
  return (
    <aside
      className={cn(
        "flex h-full flex-col gap-1 border-r border-border bg-card p-3 transition-all duration-200",
        collapsed ? "w-12 items-center" : "w-56",
      )}
    >
      <nav className="flex flex-col gap-1">
        {links.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            end={link.end}
            className={({ isActive }) =>
              cn(
                "rounded-md px-3 py-2 text-sm font-medium transition-colors",
                collapsed && "px-2",
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )
            }
          >
            {collapsed ? link.label.charAt(0) : link.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
