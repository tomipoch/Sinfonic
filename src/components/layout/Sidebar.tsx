// Sidebar — left navigation rail.
//
// Structure:
//   Home
//   Library (collapsible, expanded by default)
//     Albums, Artists, Genres, Songs
//   Smart Playlists
//   Playlist (Favorites)
//   SourceSelector (fixed bottom)

import { useState, type ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { ArrowDown01Icon, ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { cn } from "@/lib/cn";

import { SourceSelector } from "./SourceSelector";

type NavItem = {
  to: string;
  label: string;
  end?: boolean;
  sub?: readonly NavItem[];
};

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Home", end: true },
  {
    to: "/library",
    label: "Library",
    sub: [
      { to: "/library/albums", label: "Albums" },
      { to: "/library/artists", label: "Artists" },
      { to: "/library/genres", label: "Genres" },
      { to: "/library/songs", label: "Songs" },
    ],
  },
  { to: "/smart-playlists", label: "Smart Playlists" },
  { to: "/favorites", label: "Playlist" },
];

type CollapsibleSectionProps = {
  title: string;
  children: ReactNode;
  defaultExpanded?: boolean;
};

function CollapsibleSection({
  title,
  children,
  defaultExpanded = true,
}: CollapsibleSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-1.5 px-2 py-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground transition-colors hover:text-foreground"
        aria-expanded={expanded}
      >
        <HugeiconsIcon
          icon={expanded ? ArrowDown01Icon : ArrowRight01Icon}
          size={11}
          strokeWidth={2.5}
        />
        {title}
      </button>
      {expanded && children}
    </div>
  );
}

type Props = {
  collapsed?: boolean;
};

function NavItemLink({ item, collapsed }: { item: NavItem; collapsed?: boolean }) {
  if (item.sub) {
    return (
      <CollapsibleSection title={item.label} defaultExpanded={true}>
        <nav className="flex flex-col gap-0.5 pl-3">
          {item.sub.map((sub) => (
            <NavLink
              key={sub.to}
              to={sub.to}
              end={sub.end}
              className={({ isActive }) =>
                cn(
                  "rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )
              }
            >
              {sub.label}
            </NavLink>
          ))}
        </nav>
      </CollapsibleSection>
    );
  }

  return (
    <NavLink
      to={item.to}
      end={item.end}
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
      {collapsed ? item.label.charAt(0) : item.label}
    </NavLink>
  );
}

export function Sidebar({ collapsed = false }: Props) {
  return (
    <aside
      className={cn(
        "flex h-full flex-col gap-1 border-r border-border bg-card p-3 transition-all duration-200",
        collapsed ? "w-12 items-center" : "w-56",
      )}
    >
      <nav className="flex flex-col gap-0.5">
        {NAV_ITEMS.map((item) => (
          <NavItemLink key={item.to} item={item} collapsed={collapsed} />
        ))}
      </nav>

      <div className="flex-1" />

      {!collapsed && (
        <div className="border-t border-border pt-2">
          <SourceSelector />
        </div>
      )}
    </aside>
  );
}
