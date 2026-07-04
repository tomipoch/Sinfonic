// TrackTable — shared track list rendering.
//
// Configurable via a `columns` array. Each entry is a discriminated
// union that describes one column kind (cover, song, time, etc.) —
// the source of truth for what's rendered. No more boolean flags
// adding up to surprise duplicate columns.
//
// Visual contract per row:
//   [ # | Cover (play on hover) | Song (title+artist stacked) | Album | Time | Fav | Menu ]
//
// "song" stacks title + artist; "title" and "artist" render in
// separate columns (used by SongsView for a denser spreadsheet feel).
//
// P5: row memoisation. Each row is a `React.memo`d sub-component
// keyed on the `Track` object plus its own (track.id, hasSelection,
// isSelected) derivations. Sort changes rebuild the sorted array
// but only re-render the rows whose `Track` actually changed — the
// rest of the table keeps the same row reference. The thead /
// checkbox / sort buttons re-render on every sort as before.

import { memo, useEffect, useMemo, useState } from "react";

import { AlbumCover } from "@/components/ui/AlbumCover";
import type { DropdownMenuItem } from "@/components/ui/DropdownMenu";
import { FavoriteButton } from "@/components/ui/FavoriteButton";
import { PlayGlyph } from "@/components/ui/PlayGlyph";
import { TrackRowMenu } from "@/components/ui/TrackRowMenu";
import { useAlbumLookup } from "@/hooks/useAlbumLookup";
import { cn } from "@/lib/cn";
import { formatDuration } from "@/lib/format";
import { encodeDragData, type TrackDragData } from "@/lib/queueDnD";
import { compareNumber, compareString } from "@/lib/sort";
import type { ImageRef, Track } from "@/types/domain";

// ─── Column descriptors ────────────────────────────────────────────

export type TrackColumn =
  | { kind: "index"; mode: "position" | "track-number" }
  | { kind: "cover" }
  | { kind: "song" } // title + artist stacked
  | { kind: "title" }
  | { kind: "artist" }
  | { kind: "album" }
  | { kind: "time" }
  | { kind: "favorite" }
  | {
      kind: "menu";
      /** Optional per-track extra items appended after a separator. */
      extraItems?: (track: Track, index: number) => DropdownMenuItem[];
    };

export type TrackSortKey = "title" | "artist" | "album" | "durationSeconds";

const SORT_LABELS: Record<TrackSortKey, string> = {
  title: "Title",
  artist: "Artist",
  album: "Album",
  durationSeconds: "Time",
};

function compareTracks(a: Track, b: Track, key: TrackSortKey): number {
  if (key === "durationSeconds") return compareNumber(a.durationSeconds, b.durationSeconds);
  return compareString(a[key], b[key]);
}

// ─── Props ────────────────────────────────────────────────────────

export interface TrackTableSelection {
  selectedIds: ReadonlySet<string>;
  onToggle: (id: string) => void;
  onRangeToggle: (id: string) => void;
  lastSelectedId: string | null;
}

export interface TrackTableProps {
  tracks: Track[];

  /** Source of truth for which columns render and in what order. */
  columns: TrackColumn[];

  /** Click handler for the cover-overlay play button. */
  onPlayTrack?: (track: Track, index: number) => void;

  /** Subset of columns that get a sortable header. */
  sortableColumns?: TrackSortKey[];
  defaultSort?: TrackSortKey;

  /** Whether rows are draggable to the queue. */
  draggable?: boolean;
  dragSource?: TrackDragData["source"];

  /** Multi-select state. */
  selection?: TrackTableSelection;

  className?: string;
}

// ─── Row component (memoised) ────────────────────────────────────

interface TrackRowProps {
  track: Track;
  index: number;
  columns: readonly TrackColumn[];
  coverImageRef: ImageRef | null;
  selection: TrackTableSelection | undefined;
  isSelected: boolean;
  isDragging: boolean;
  draggable: boolean;
  dragSource: TrackDragData["source"];
  onPlayTrack: ((track: Track, index: number) => void) | undefined;
  onRowClick: (e: React.MouseEvent, trackId: string) => void;
  onDragStart: (trackId: string) => void;
  onDragEnd: () => void;
}

const TrackRow = memo(function TrackRow({
  track,
  index,
  columns,
  coverImageRef,
  selection,
  isSelected,
  isDragging,
  draggable,
  dragSource,
  onPlayTrack,
  onRowClick,
  onDragStart,
  onDragEnd,
}: TrackRowProps) {
  const hasSelection = selection !== undefined;
  return (
    <tr
      draggable={draggable}
      onDragStart={(e) => {
        if (!draggable) return;
        onDragStart(track.id);
        e.dataTransfer.setData(
          "application/json",
          encodeDragData({ tracks: [track], source: dragSource }),
        );
        e.dataTransfer.effectAllowed = "copy";
      }}
      onDragEnd={onDragEnd}
      onClick={hasSelection ? (e) => onRowClick(e, track.id) : undefined}
      className={cn(
        "group transition-colors",
        hasSelection ? "cursor-pointer hover:bg-muted" : "hover:bg-muted",
        isSelected && "bg-primary/15 hover:bg-primary/20",
        isDragging && "opacity-30",
      )}
    >
      {hasSelection && (
        <td className="px-2 py-1.5">
          <input
            type="checkbox"
            aria-label={`Select ${track.title}`}
            checked={isSelected}
            onChange={(e) => {
              // Click on the checkbox itself → toggle (with optional
              // shift-range support). Don't go through `onRowClick`
              // because that handler also has the `closest("button,
              // a, input")` guard which would swallow this click.
              const shift = e.nativeEvent instanceof MouseEvent && e.nativeEvent.shiftKey;
              if (shift && selection?.lastSelectedId) {
                selection?.onRangeToggle(track.id);
              } else {
                selection?.onToggle(track.id);
              }
            }}
            onClick={(e) => e.stopPropagation()}
            className="h-4 w-4 cursor-pointer accent-primary"
          />
        </td>
      )}
      {columns.map((col, idx) => renderCell(col, idx, track, index, coverImageRef, onPlayTrack))}
    </tr>
  );
});

// ─── Component ────────────────────────────────────────────────────

export function TrackTable({
  tracks,
  columns,
  onPlayTrack,
  sortableColumns = [],
  defaultSort = "title",
  draggable = true,
  dragSource = "songs-view",
  selection,
  className,
}: TrackTableProps) {
  const { albumById, ensureLoaded } = useAlbumLookup();

  const [sortKey, setSortKey] = useState<TrackSortKey>(defaultSort);
  const [draggingId, setDraggingId] = useState<string | null>(null);

  const isSortable = sortableColumns.length > 0;

  const sorted = useMemo(() => {
    if (!isSortable) return tracks;
    return [...tracks].sort((a, b) => compareTracks(a, b, sortKey));
  }, [tracks, isSortable, sortKey]);

  // Prewarm the cover for every visible row so the hover-play overlay
  // doesn't pop in after a second fetch.
  useEffect(() => {
    if (!onPlayTrack) return;
    for (const track of sorted) ensureLoaded(track.albumId);
  }, [sorted, ensureLoaded, onPlayTrack]);

  const hasSelection = selection !== undefined;
  const allSelected =
    hasSelection && sorted.length > 0 && sorted.every((t) => selection?.selectedIds.has(t.id));
  const someSelected =
    hasSelection && sorted.some((t) => selection?.selectedIds.has(t.id)) && !allSelected;

  const toggleAll = () => {
    if (!selection) return;
    if (allSelected) {
      for (const t of sorted) selection.onToggle(t.id);
    } else {
      for (const t of sorted) {
        if (!selection.selectedIds.has(t.id)) selection.onToggle(t.id);
      }
    }
  };

  const onRowClick = (e: React.MouseEvent, trackId: string) => {
    if (!hasSelection) return;
    const target = e.target as HTMLElement;
    if (target.closest("button, a, input")) return;
    if (e.shiftKey && selection?.lastSelectedId) {
      selection.onRangeToggle(trackId);
    } else {
      selection.onToggle(trackId);
    }
  };

  return (
    <div className={cn("overflow-x-auto rounded-md border border-border", className)}>
      <table className="w-full table-fixed text-sm">
        <thead className="bg-muted text-xs tracking-wide text-muted-foreground">
          <tr>
            {hasSelection && (
              <th scope="col" className="w-10 px-2 py-2">
                <input
                  type="checkbox"
                  aria-label="Select all"
                  checked={allSelected}
                  ref={(el) => {
                    if (el) el.indeterminate = someSelected;
                  }}
                  onChange={toggleAll}
                  className="h-4 w-4 cursor-pointer accent-primary"
                />
              </th>
            )}
            {columns.map((col, idx) =>
              renderHeader(col, idx, sortKey, setSortKey, sortableColumns),
            )}
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {sorted.map((track, index) => {
            const album = albumById.get(track.albumId);
            const coverImageRef = track.imageRef ?? album?.imageRef ?? null;
            const isSelected = hasSelection && selection?.selectedIds.has(track.id) === true;
            return (
              <TrackRow
                key={track.id}
                track={track}
                index={index}
                columns={columns}
                coverImageRef={coverImageRef}
                selection={selection}
                isSelected={isSelected}
                isDragging={draggingId === track.id}
                draggable={draggable}
                dragSource={dragSource}
                onPlayTrack={onPlayTrack}
                onRowClick={onRowClick}
                onDragStart={setDraggingId}
                onDragEnd={() => setDraggingId(null)}
              />
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

// ─── Header / cell renderers ──────────────────────────────────────

function renderHeader(
  col: TrackColumn,
  idx: number,
  sortKey: TrackSortKey,
  setSortKey: (k: TrackSortKey) => void,
  sortableColumns: TrackSortKey[],
): React.ReactNode {
  switch (col.kind) {
    case "index":
      return (
        <th key={idx} scope="col" className="w-10 px-2 py-2 text-right">
          #
        </th>
      );
    case "cover":
      return <th key={idx} scope="col" className="w-14 px-2 py-2" aria-label="Cover" />;
    case "song":
      return (
        <th key={idx} scope="col" className="px-3 py-2 text-left">
          <SortHeader
            label="Song"
            active={sortKey === "title"}
            sortable={sortableColumns.includes("title")}
            onClick={() => setSortKey("title")}
          />
        </th>
      );
    case "title":
      return (
        <th key={idx} scope="col" className="px-3 py-2 text-left">
          <SortHeader
            label={SORT_LABELS.title}
            active={sortKey === "title"}
            sortable={sortableColumns.includes("title")}
            onClick={() => setSortKey("title")}
          />
        </th>
      );
    case "artist":
      return (
        <th key={idx} scope="col" className="px-3 py-2 text-left">
          <SortHeader
            label={SORT_LABELS.artist}
            active={sortKey === "artist"}
            sortable={sortableColumns.includes("artist")}
            onClick={() => setSortKey("artist")}
          />
        </th>
      );
    case "album":
      return (
        <th key={idx} scope="col" className="px-3 py-2 text-left">
          <SortHeader
            label={SORT_LABELS.album}
            active={sortKey === "album"}
            sortable={sortableColumns.includes("album")}
            onClick={() => setSortKey("album")}
          />
        </th>
      );
    case "time":
      return (
        <th key={idx} scope="col" className="px-3 py-2 text-right">
          <SortHeader
            label={SORT_LABELS.durationSeconds}
            active={sortKey === "durationSeconds"}
            sortable={sortableColumns.includes("durationSeconds")}
            onClick={() => setSortKey("durationSeconds")}
            align="right"
          />
        </th>
      );
    case "favorite":
      return <th key={idx} scope="col" className="w-10 px-2 py-2" />;
    case "menu":
      return <th key={idx} scope="col" className="w-10 px-2 py-2" />;
  }
}

function SortHeader({
  label,
  active,
  sortable,
  onClick,
  align = "left",
}: {
  label: string;
  active: boolean;
  sortable: boolean;
  onClick: () => void;
  align?: "left" | "right";
}) {
  const baseClass = cn("font-medium", align === "right" && "block text-right");
  if (!sortable) {
    return <span className={baseClass}>{label}</span>;
  }
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "font-medium hover:text-foreground focus:outline-none",
        align === "right" && "ml-auto block",
        active ? "text-foreground" : "text-muted-foreground",
      )}
    >
      {label}
      {active && <span className="ml-1">▾</span>}
    </button>
  );
}

function renderCell(
  col: TrackColumn,
  idx: number,
  track: Track,
  index: number,
  coverImageRef: Track["imageRef"],
  onPlayTrack: ((track: Track, index: number) => void) | undefined,
): React.ReactNode {
  switch (col.kind) {
    case "index":
      return (
        <td key={idx} className="px-2 py-2 text-right font-mono text-xs text-muted-foreground">
          {col.mode === "position" ? index + 1 : track.trackNumber || "—"}
        </td>
      );
    case "cover":
      return (
        <td key={idx} className="px-2 py-1.5">
          <div className="group/cover relative h-9 w-9 overflow-hidden rounded shadow-sm ring-1 ring-inset ring-border/40">
            {coverImageRef ? (
              <AlbumCover
                source={{
                  id: track.id,
                  title: track.album || track.title,
                  imageRef: coverImageRef,
                }}
                ariaLabel={`Cover art for ${track.album}`}
                className="h-9 w-9"
              />
            ) : (
              <div className="h-9 w-9 bg-gradient-to-br from-secondary to-muted" />
            )}
            {onPlayTrack && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onPlayTrack(track, index);
                }}
                aria-label={`Play ${track.title}`}
                className="absolute inset-0 flex items-center justify-center bg-black/40 text-primary-foreground opacity-0 transition-opacity group-hover/cover:opacity-100 focus:opacity-100 focus:outline-none"
              >
                <PlayGlyph size={14} />
              </button>
            )}
          </div>
        </td>
      );
    case "song":
      return (
        <td key={idx} className="px-3 py-1.5">
          <div className="truncate font-medium text-foreground">{track.title}</div>
          <div className="truncate text-xs text-muted-foreground">{track.artist}</div>
        </td>
      );
    case "title":
      return (
        <td key={idx} className="px-3 py-2 text-foreground">
          {track.title}
        </td>
      );
    case "artist":
      return (
        <td key={idx} className="px-3 py-2 text-muted-foreground">
          {track.artist}
        </td>
      );
    case "album":
      return (
        <td key={idx} className="px-3 py-2 text-muted-foreground">
          {track.album}
        </td>
      );
    case "time":
      return (
        <td key={idx} className="px-3 py-2 text-right text-muted-foreground">
          {formatDuration(track.durationSeconds)}
        </td>
      );
    case "favorite":
      return (
        <td key={idx} className="px-2 py-2">
          <FavoriteButton kind="track" itemId={track.id} initialFavorite={track.favorite} />
        </td>
      );
    case "menu":
      return (
        <td key={idx} className="px-2 py-2">
          <TrackRowMenu track={track} extraItems={col.extraItems?.(track, index)} />
        </td>
      );
  }
}
