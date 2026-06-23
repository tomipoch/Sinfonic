// Sidebar — left navigation rail inspired by Apple Music.
//
// Structure (expanded):
//   Home (direct link with icon)
//   BIBLIOTECA  (collapsible header)
//     Canciones, Álbumes, Artistas, Géneros
//   PLAYLISTS   (collapsible header)
//     Todas las playlists, Canciones favoritas, Smart Playlists
//
// Structure (collapsed):
//   Icon-only rail. Items without icons are hidden.
//   Headers become invisible but the row spacing is preserved so the
//   icon column stays aligned with the expanded layout.

import { useState } from "react";
import { NavLink } from "react-router-dom";
import {
  Album01Icon,
  AlbumIcon,
  ArrowDown01Icon,
  ArrowRight01Icon,
  Home01Icon,
  SparklesIcon,
  StarIcon,
  TagIcon,
  UserIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { cn } from "@/lib/cn";

import { SourceSelector } from "./SourceSelector";

type NavItem = {
  to: string;
  label: string;
  end?: boolean;
  icon?: typeof Home01Icon;
};

type Section = {
  title: string;
  defaultExpanded?: boolean;
  items: NavItem[];
};

const HOME_ITEM: NavItem = { to: "/", label: "Home", end: true, icon: Home01Icon };

const SECTIONS: Section[] = [
  {
    title: "Library",
    defaultExpanded: true,
    items: [
      { to: "/library/songs", label: "Songs", icon: AlbumIcon },
      { to: "/library/albums", label: "Albums", icon: Album01Icon },
      { to: "/library/artists", label: "Artists", icon: UserIcon },
      { to: "/library/genres", label: "Genres", icon: TagIcon },
    ],
  },
  {
    title: "Playlists",
    defaultExpanded: true,
    items: [
      { to: "/playlists", label: "All Playlists", icon: Album01Icon },
      { to: "/favorites", label: "Favorite Songs", icon: StarIcon },
      { to: "/smart-playlists", label: "Smart Playlists", icon: SparklesIcon },
    ],
  },
];

type ItemLinkProps = {
  item: NavItem;
  collapsed: boolean;
};

function ItemLink({ item, collapsed }: ItemLinkProps) {
  // In collapsed mode, items without an icon are hidden (Apple Music-like).
  if (collapsed && !item.icon) return null;

  const iconSize = collapsed ? 18 : 14;

  return (
    <NavLink
      to={item.to}
      end={item.end}
      title={collapsed ? item.label : undefined}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
          collapsed && "justify-center px-0 py-2",
          isActive
            ? "bg-primary text-primary-foreground"
            : "text-muted-foreground hover:bg-muted hover:text-foreground",
        )
      }
    >
      {item.icon && (
        <HugeiconsIcon
          icon={item.icon}
          size={iconSize}
          strokeWidth={1.75}
          className="shrink-0"
        />
      )}
      {!collapsed && item.label}
    </NavLink>
  );
}

type SectionBlockProps = {
  section: Section;
  collapsed: boolean;
};

function SectionBlock({ section, collapsed }: SectionBlockProps) {
  const [expanded, setExpanded] = useState(section.defaultExpanded ?? true);

  // In collapsed mode, hide the header but keep the row spacing.
  if (collapsed) {
    const visibleItems = section.items.filter((it) => it.icon);
    if (visibleItems.length === 0) return null;
    return (
      <div className="flex flex-col gap-0.5">
        {visibleItems.map((item) => (
          <ItemLink key={item.to} item={item} collapsed={collapsed} />
        ))}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
        className="flex w-full items-center gap-1.5 px-3 pt-3 pb-1 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground transition-colors hover:text-foreground"
      >
        <HugeiconsIcon
          icon={expanded ? ArrowDown01Icon : ArrowRight01Icon}
          size={10}
          strokeWidth={2.5}
        />
        {section.title}
      </button>
      {expanded && (
        <div className="flex flex-col gap-0.5">
          {section.items.map((item) => (
            <ItemLink key={item.to} item={item} collapsed={collapsed} />
          ))}
        </div>
      )}
    </div>
  );
}

type Props = {
  collapsed?: boolean;
};

export function Sidebar({ collapsed = false }: Props) {
  return (
    <aside
      className={cn(
        "flex h-full flex-col gap-1 overflow-y-auto border-r border-border bg-card p-3 transition-all duration-200 [overscroll-behavior:contain]",
        collapsed ? "w-14 items-center" : "w-56",
      )}
    >
      <nav className="flex flex-col gap-0.5">
        <ItemLink item={HOME_ITEM} collapsed={collapsed} />
      </nav>

      {SECTIONS.map((section) => (
        <SectionBlock key={section.title} section={section} collapsed={collapsed} />
      ))}

      <div className="flex-1" />

      {!collapsed && (
        <div className="border-t border-border pt-2">
          <SourceSelector />
        </div>
      )}
    </aside>
  );
}
