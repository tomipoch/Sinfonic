// Tracks table for /library/tracks. Read directly from the cache
// loaded by `useLibraryAutoLoad`. Each row has a Play button that
// fires the same single-track flow as the album detail view.

import { useState } from "react";
import { toast } from "sonner";

import { FavoriteButton } from "../ui/FavoriteButton";
import { useLibraryStore } from "../../stores/libraryStore";
import { playTrack } from "../../lib/tauri";
import { formatDuration } from "../../lib/format";
import { encodeDragData } from "../../lib/queueDnD";
import { cn } from "../../lib/cn";
import type { Track } from "../../types/domain";

type SortKey = "title" | "artist" | "album" | "durationSeconds";

function compareTracks(a: Track, b: Track, key: SortKey): number {
  if (key === "durationSeconds") return a.durationSeconds - b.durationSeconds;
  return a[key].localeCompare(b[key], undefined, { sensitivity: "base" });
}

export function TracksTab() {
  const tracks = useLibraryStore((s) => s.tracks);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);

  const [sortKey, setSortKey] = useState<SortKey>("title");
  const [busy, setBusy] = useState(false);
  const [draggingId, setDraggingId] = useState<string | null>(null);

  if (loading && tracks.length === 0) {
    return (
      <p className="text-fg-subtle text-sm" role="status">
        Loading tracks…
      </p>
    );
  }

  if (loaded && tracks.length === 0) {
    return (
      <p className="text-fg-subtle text-sm">
        No tracks in the library yet. Sync your library to populate it.
      </p>
    );
  }

  const sorted = [...tracks].sort((a, b) => compareTracks(a, b, sortKey));

  const onPlay = async (track: Track) => {
    setBusy(true);
    try {
      await playTrack(track);
    } catch (err) {
      toast.error(`Couldn't play track: ${(err as Error).message ?? String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const headers: ReadonlyArray<{ key: SortKey; label: string }> = [
    { key: "title", label: "Title" },
    { key: "artist", label: "Artist" },
    { key: "album", label: "Album" },
    { key: "durationSeconds", label: "Time" },
  ];

  return (
    <div className="overflow-hidden rounded-md border border-bg-raised">
      <table className="w-full text-sm">
        <thead className="bg-bg-subtle text-xs uppercase tracking-wide text-fg-subtle">
          <tr>
            <th className="w-10 px-2 py-2 text-right" aria-label="Play" />
            {headers.map((h) => (
              <th
                key={h.key}
                scope="col"
                className="px-3 py-2 text-left"
              >
                <button
                  type="button"
                  onClick={() => setSortKey(h.key)}
                  className={
                    "font-medium hover:text-fg focus:outline-none " +
                    (sortKey === h.key ? "text-fg" : "text-fg-subtle")
                  }
                >
                  {h.label}
                  {sortKey === h.key && <span className="ml-1">▾</span>}
                </button>
              </th>
            ))}
            <th scope="col" className="w-10 px-3 py-2" />
          </tr>
        </thead>
        <tbody className="divide-y divide-bg-raised">
          {sorted.map((track) => (
            <tr
              key={track.id}
              draggable
              onDragStart={(e) => {
                setDraggingId(track.id);
                e.dataTransfer.setData("application/json", encodeDragData({ tracks: [track], source: "tracks-tab" }));
                e.dataTransfer.effectAllowed = "copy";
              }}
              onDragEnd={() => setDraggingId(null)}
              className={cn("hover:bg-bg-subtle", draggingId === track.id && "opacity-30")}
            >
              <td className="px-2 py-2 text-right">
                <button
                  type="button"
                  onClick={() => void onPlay(track)}
                  disabled={busy}
                  aria-label={`Play ${track.title}`}
                  className="rounded p-1 text-fg-subtle hover:bg-bg-raised hover:text-fg focus:outline-none"
                >
                  ▶
                </button>
              </td>
              <td className="px-3 py-2 text-fg">{track.title}</td>
              <td className="px-3 py-2 text-fg-subtle">{track.artist}</td>
              <td className="px-3 py-2 text-fg-subtle">{track.album}</td>
              <td className="px-3 py-2 text-right text-fg-muted">
                {formatDuration(track.durationSeconds)}
              </td>
              <td className="px-3 py-2">
                <FavoriteButton kind="track" itemId={track.id} initialFavorite={track.favorite} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
