// Sidebar — left navigation rail inspired by Apple Music.
//
// Structure (expanded):
//   Home (direct link with icon)
//   LIBRARY    (collapsible header)
//     Songs, Albums, Artists, Genres
//   PLAYLISTS  (collapsible header)
//     All Playlists, Favorite Songs, Smart Playlists,
//     <one entry per user playlist from the active server>
//
// Structure (collapsed):
//   Icon-only rail. Items without icons are hidden.
//   Headers become invisible but the row spacing is preserved so the
//   icon column stays aligned with the expanded layout.
//
// Dynamic playlist entries come from `usePlaylistsStore`, which is
// fed by the same `playlists_get` IPC that the full-page PlaylistsView
// uses. The store is fetched on mount (and on every server switch);
// a refresh-on-sync listener in `Layout` keeps it current.

import { useEffect, useMemo, useState } from "react";
import { NavLink } from "react-router-dom";
import { useShallow } from "zustand/react/shallow";
import {
  Album01Icon,
  AlbumIcon,
  ArrowDown01Icon,
  ArrowRight01Icon,
  Home01Icon,
  MusicNote01Icon,
  SparklesIcon,
  StarIcon,
  TagIcon,
  UserIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { cn } from "@/lib/cn";

import { usePlaylistsStore } from "@/stores/playlistsStore";
import { useServerStore } from "@/stores/serverStore";

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

const LIBRARY_ITEMS: NavItem[] = [
  { to: "/songs", label: "Songs", icon: AlbumIcon },
  { to: "/albums", label: "Albums", icon: Album01Icon },
  { to: "/artists", label: "Artists", icon: UserIcon },
  { to: "/genres", label: "Genres", icon: TagIcon },
];

const PLAYLISTS_STATIC_ITEMS: NavItem[] = [
  { to: "/playlists", label: "All Playlists", icon: Album01Icon },
  { to: "/favorites", label: "Favorite Songs", icon: StarIcon },
  { to: "/smart-playlists", label: "Smart Playlists", icon: SparklesIcon },
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
      {!collapsed && <span className="truncate">{item.label}</span>}
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
  const activeServerId = useServerStore((s) => s.activeServerId);
  // `useShallow` so the Sidebar doesn't re-render on every store
  // mutation (e.g. detail loading/error flips in PlaylistsView).
  const { playlists, loadPlaylists } = usePlaylistsStore(
    useShallow((s) => ({
      playlists: s.playlists,
      loadPlaylists: s.loadPlaylists,
    })),
  );

  // Fetch the playlist list when an active server is present, and
  // reset to empty when the user logs out. The post-sync refresh
  // lives in `Layout` so it survives view changes.
  useEffect(() => {
    if (activeServerId) {
      void loadPlaylists();
    } else {
      // Drop the cache so a new login doesn't briefly show the
      // previous server's playlists.
      usePlaylistsStore.setState({ playlists: [] });
    }
  }, [activeServerId, loadPlaylists]);

  const sections = useMemo<Section[]>(() => {
    const userPlaylistItems: NavItem[] = [...playlists]
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }))
      .map((pl) => ({
        to: `/playlists/${encodeURIComponent(pl.id)}`,
        label: pl.name,
        icon: MusicNote01Icon,
      }));
    return [
      { title: "Library", defaultExpanded: true, items: LIBRARY_ITEMS },
      {
        title: "Playlists",
        defaultExpanded: true,
        items: [...PLAYLISTS_STATIC_ITEMS, ...userPlaylistItems],
      },
    ];
  }, [playlists]);

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

      {sections.map((section) => (
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