// PlaylistDetailView — header + track table for one playlist.
// Actions: play all, rename, delete, remove individual tracks.

import { Delete02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { playTrack } from "@/lib/tauri";
import { usePlaylistsStore } from "@/stores/playlistsStore";
import type { Track } from "@/types/domain";

const COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "song" },
  { kind: "time" },
  { kind: "favorite" },
  {
    kind: "menu",
    extraItems: (_, index) => [
      {
        label: "Remove from playlist",
        icon: <HugeiconsIcon icon={Delete02Icon} size={14} strokeWidth={1.75} />,
        onClick: () => {
          window.dispatchEvent(new CustomEvent("playlist:remove-track", { detail: { index } }));
        },
        destructive: true,
        separator: true,
      },
    ],
  },
];

export function PlaylistDetailView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  // `useShallow` keeps the selector's identity stable across store
  // updates that don't actually change the picked fields. Without
  // it, the view re-renders on every mutation anywhere in the
  // playlists store (e.g. sidebar refreshes, other playlists
  // loading) even when the active playlist is unchanged.
  const {
    detail,
    detailLoading,
    detailError,
    loadPlaylistDetail,
    renamePlaylist,
    deletePlaylist,
    removePlaylistEntries,
    playPlaylist,
  } = usePlaylistsStore(
    useShallow((s) => ({
      detail: s.detail,
      detailLoading: s.detailLoading,
      detailError: s.detailError,
      loadPlaylistDetail: s.loadPlaylistDetail,
      renamePlaylist: s.renamePlaylist,
      deletePlaylist: s.deletePlaylist,
      removePlaylistEntries: s.removePlaylistEntries,
      playPlaylist: s.playPlaylist,
    })),
  );

  const [renaming, setRenaming] = useState(false);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (id) void loadPlaylistDetail(decodeURIComponent(id));
  }, [id, loadPlaylistDetail]);

  useEffect(() => {
    // Listen for the menu's "Remove from playlist" action. The
    // handler reads the current playlist id and entries from the
    // store via `getState()` so it never captures a stale closure
    // when the playlist is renamed or refetched mid-flight.
    const onRemove = (e: Event) => {
      const ce = e as CustomEvent<{ index: number }>;
      const current = usePlaylistsStore.getState().detail;
      if (!current) return;
      void removePlaylistEntries(current.playlist.id, [String(ce.detail.index)]);
    };
    window.addEventListener("playlist:remove-track", onRemove as EventListener);
    return () => window.removeEventListener("playlist:remove-track", onRemove as EventListener);
  }, [removePlaylistEntries]);

  if (!id) {
    return <p className="text-muted-foreground text-sm">Missing playlist id.</p>;
  }

  if (detailLoading) {
    return (
      <p className="text-muted-foreground text-sm" role="status">
        Loading playlist…
      </p>
    );
  }

  if (detailError || !detail) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
        <div className="text-base font-medium text-red-400">Failed to load playlist</div>
        <p className="text-sm text-red-300">{detailError ?? "Playlist not found"}</p>
        <button
          type="button"
          onClick={() => void loadPlaylistDetail(decodeURIComponent(id))}
          className="btn-ghost text-sm"
        >
          Retry
        </button>
      </div>
    );
  }

  const { playlist, tracks } = detail;

  const onPlayAll = async () => {
    if (busy || tracks.length === 0) return;
    setBusy(true);
    try {
      await playPlaylist(playlist.id);
    } catch (e) {
      toast.error(`Couldn't play: ${extractError(e, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayTrack = async (track: Track) => {
    setBusy(true);
    try {
      await playTrack(track);
    } catch (e) {
      toast.error(`Couldn't play track: ${extractError(e, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onRename = async () => {
    if (!newName.trim() || newName.trim() === playlist.name) {
      setRenaming(false);
      return;
    }
    try {
      await renamePlaylist(playlist.id, newName.trim());
      toast.success(`Renamed to "${newName.trim()}"`);
      setRenaming(false);
    } catch (e) {
      toast.error(`Couldn't rename: ${extractError(e, "unknown error")}`);
    }
  };

  const onDelete = async () => {
    if (!confirm(`Delete playlist "${playlist.name}"? This cannot be undone.`)) return;
    try {
      await deletePlaylist(playlist.id);
      toast.success("Playlist deleted");
      navigate("/playlists");
    } catch (e) {
      toast.error(`Couldn't delete: ${extractError(e, "unknown error")}`);
    }
  };

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <div className="w-48 shrink-0">
          {playlist.imageRef ? (
            <AlbumCover
              source={{ id: playlist.id, title: playlist.name, imageRef: playlist.imageRef }}
              className="aspect-square w-full rounded-md shadow-sm"
              ariaLabel={`Cover art for playlist ${playlist.name}`}
            />
          ) : (
            <div className="flex aspect-square w-full items-center justify-center rounded-md bg-card text-6xl text-white/30">
              🎵
            </div>
          )}
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          <div className="text-xs uppercase tracking-wide text-muted-foreground">Playlist</div>
          {renaming ? (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void onRename();
                  if (e.key === "Escape") setRenaming(false);
                }}
                onBlur={() => void onRename()}
                className="rounded-md border border-border bg-muted px-2 py-1 text-2xl font-semibold text-foreground focus:border-primary focus:outline-none"
              />
            </div>
          ) : (
            <h1 className="truncate text-3xl font-semibold">{playlist.name}</h1>
          )}
          <div className="text-base text-muted-foreground">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"} ·{" "}
            {formatDuration(playlist.durationSeconds)}
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              onClick={onPlayAll}
              disabled={busy || tracks.length === 0}
              className="btn-primary"
            >
              Play all
            </button>
            <button
              type="button"
              onClick={() => {
                setRenaming(true);
                setNewName(playlist.name);
              }}
              className="btn-ghost text-sm"
            >
              Rename
            </button>
            <button type="button" onClick={onDelete} className="btn-ghost text-sm text-red-400">
              Delete
            </button>
          </div>
        </div>
      </header>

      {tracks.length === 0 ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
          <div className="text-base font-medium text-foreground">Playlist is empty</div>
          <p className="text-sm text-muted-foreground">
            Drag tracks here or use the queue to add them.
          </p>
        </div>
      ) : (
        <TrackTable
          tracks={tracks}
          columns={COLUMNS}
          onPlayTrack={onPlayTrack}
          dragSource="playlist-detail"
        />
      )}
    </section>
  );
}
