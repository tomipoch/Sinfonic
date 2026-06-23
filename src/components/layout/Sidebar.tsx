// Sidebar — left navigation rail with collapsible sections.
//
// Layout:
//   Navigation (collapsible)
//     Home, Library, Playlists, Favorites, Smart Playlists
//   Playlists (collapsible) [user playlists placeholder]
//   SourceSelector (fixed bottom)

import { useState, type ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { ArrowDown01Icon, ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { cn } from "@/lib/cn";

import { SourceSelector } from "./SourceSelector";

type CollapsibleSectionProps = {
  title: string;
  children: ReactNode;
  defaultExpanded?: boolean;
  collapsed?: boolean;
};

function CollapsibleSection({
  title,
  children,
  defaultExpanded = true,
  collapsed = false,
}: CollapsibleSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  if (collapsed) return null;

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

const navLinks: ReadonlyArray<{ to: string; label: string; end?: boolean }> = [
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
      {/* Navigation */}
      <CollapsibleSection title="Navigation" defaultExpanded={true} collapsed={collapsed}>
        <nav className="flex flex-col gap-1">
          {navLinks.map((link) => (
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
      </CollapsibleSection>

      {/* Playlists placeholder */}
      <CollapsibleSection title="Playlists" defaultExpanded={false} collapsed={collapsed}>
        <div className="pl-4 text-xs text-muted-foreground">
          No playlists yet
        </div>
      </CollapsibleSection>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Source selector */}
      {!collapsed && (
        <div className="border-t border-border pt-2">
          <SourceSelector />
        </div>
      )}
    </aside>
  );
}
