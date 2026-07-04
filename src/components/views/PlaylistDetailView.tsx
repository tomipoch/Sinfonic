// PlaylistDetailView — header + track table for one playlist.
//
// Actions: Play all (primary), Shuffle (secondary), Rename / Delete
// (icon buttons), and an overflow menu with the rest of the actions
// (Add to queue, Play next). The playlist art falls back to a 2x2
// mosaic of the first four tracks when no cover image is available.

import { Delete02Icon, MoreVerticalIcon, Pen01Icon, ShuffleIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";

import { DropdownMenu } from "@/components/ui/DropdownMenu";
import { MarqueeText } from "@/components/ui/MarqueeText";
import { PlaylistArt } from "@/components/ui/PlaylistArt";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { playTrackWithContext, queueAddMany, queuePlayNextMany, setShuffle } from "@/lib/tauri";
import { usePlaylistsStore } from "@/stores/playlistsStore";
import { useServerStore } from "@/stores/serverStore";
import type { Track } from "@/types/domain";

const COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "song" },
  { kind: "album" },
  { kind: "time" },
  { kind: "favorite" },
  {
    kind: "menu",
    extraItems: (_, index) => [
      {
        label: "Remove from playlist",
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
    return <p className="p-6 text-muted-foreground text-sm">Missing playlist id.</p>;
  }

  if (detailLoading) {
    return (
      <p className="p-6 text-muted-foreground text-sm" role="status">
        Loading playlist…
      </p>
    );
  }

  if (detailError || !detail) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
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

  const onPlayShuffled = async () => {
    if (busy || tracks.length === 0) return;
    setBusy(true);
    try {
      await setShuffle(true);
      await playPlaylist(playlist.id);
    } catch (e) {
      toast.error(`Couldn't shuffle: ${extractError(e, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayTrack = async (track: Track) => {
    setBusy(true);
    try {
      const serverId = useServerStore.getState().activeServerId;
      await playTrackWithContext(
        track,
        serverId ? { kind: "playlist", playlistId: playlist.id, serverId } : null,
      );
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

  const onAddToQueue = async () => {
    try {
      await queueAddMany(tracks);
      toast.success(`Added ${tracks.length} track${tracks.length === 1 ? "" : "s"} to queue`);
    } catch (e) {
      toast.error(`Couldn't add to queue: ${extractError(e, "unknown error")}`);
    }
  };

  const onPlayNext = async () => {
    try {
      await queuePlayNextMany(tracks);
      toast.success(`Queued ${tracks.length} track${tracks.length === 1 ? "" : "s"} next`);
    } catch (e) {
      toast.error(`Couldn't queue next: ${extractError(e, "unknown error")}`);
    }
  };

  const overflowItems = [
    {
      label: "Add to queue",
      onClick: () => void onAddToQueue(),
      disabled: tracks.length === 0,
    },
    {
      label: "Play next",
      onClick: () => void onPlayNext(),
      disabled: tracks.length === 0,
    },
  ];

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <div className="w-48 shrink-0">
          <PlaylistArt playlist={playlist} previewTracks={tracks.slice(0, 4)} />
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          {renaming ? (
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void onRename();
                if (e.key === "Escape") setRenaming(false);
              }}
              onBlur={() => void onRename()}
              autoFocus
              className="rounded-md border border-border bg-muted px-2 py-1 text-3xl font-semibold text-foreground focus:border-primary focus:outline-none"
            />
          ) : (
            <h1 className="text-3xl font-semibold">
              <MarqueeText>{playlist.name}</MarqueeText>
            </h1>
          )}
          <div className="text-base text-muted-foreground">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"} ·{" "}
            {formatDuration(playlist.durationSeconds)}
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-2">
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
              onClick={onPlayShuffled}
              disabled={busy || tracks.length === 0}
              className="btn-ghost"
              aria-label="Shuffle playlist"
              title="Shuffle playlist"
            >
              <HugeiconsIcon icon={ShuffleIcon} size={16} strokeWidth={1.75} />
              <span className="ml-1.5">Shuffle</span>
            </button>
            <button
              type="button"
              onClick={() => {
                setRenaming(true);
                setNewName(playlist.name);
              }}
              className="rounded p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
              aria-label="Rename playlist"
              title="Rename"
            >
              <HugeiconsIcon icon={Pen01Icon} size={16} strokeWidth={1.75} />
            </button>
            <button
              type="button"
              onClick={onDelete}
              className="rounded p-1.5 text-red-400 hover:bg-red-950/40"
              aria-label="Delete playlist"
              title="Delete"
            >
              <HugeiconsIcon icon={Delete02Icon} size={16} strokeWidth={1.75} />
            </button>
            <DropdownMenu
              ariaLabel="More playlist actions"
              trigger={
                <HugeiconsIcon icon={MoreVerticalIcon} size={16} strokeWidth={1.75} aria-hidden />
              }
              items={overflowItems}
            />
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
