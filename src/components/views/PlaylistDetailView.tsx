// PlaylistDetailView — header + track table for one playlist.
// Actions: play all, rename, delete, remove individual tracks.

import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { toast } from "sonner";

import { usePlaylistsStore } from "../../stores/playlistsStore";
import { formatDuration } from "../../lib/format";
import { playTrack } from "../../lib/tauri";
import type { Track } from "../../types/domain";

export function PlaylistDetailView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { detail, detailLoading, detailError, loadPlaylistDetail, renamePlaylist, deletePlaylist, removePlaylistEntries, playPlaylist } =
    usePlaylistsStore();

  const [renaming, setRenaming] = useState(false);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (id) void loadPlaylistDetail(decodeURIComponent(id));
  }, [id, loadPlaylistDetail]);

  if (!id) {
    return <p className="text-fg-subtle text-sm">Missing playlist id.</p>;
  }

  if (detailLoading) {
    return <p className="text-fg-subtle text-sm" role="status">Loading playlist…</p>;
  }

  if (detailError || !detail) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
        <div className="text-base font-medium text-red-400">Failed to load playlist</div>
        <p className="text-sm text-red-300">{detailError ?? "Playlist not found"}</p>
        <button type="button" onClick={() => void loadPlaylistDetail(decodeURIComponent(id))} className="btn-ghost text-sm">
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
      toast.error(`Couldn't play: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const onPlayTrack = async (track: Track) => {
    setBusy(true);
    try {
      await playTrack(track);
    } catch (e) {
      toast.error(`Couldn't play track: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const onRemoveTrack = async (trackIndex: number) => {
    try {
      await removePlaylistEntries(playlist.id, [String(trackIndex)]);
    } catch (e) {
      toast.error(`Couldn't remove track: ${(e as Error).message}`);
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
      toast.error(`Couldn't rename: ${(e as Error).message}`);
    }
  };

  const onDelete = async () => {
    if (!confirm(`Delete playlist "${playlist.name}"? This cannot be undone.`)) return;
    try {
      await deletePlaylist(playlist.id);
      toast.success("Playlist deleted");
      navigate("/playlists");
    } catch (e) {
      toast.error(`Couldn't delete: ${(e as Error).message}`);
    }
  };

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <div className="flex h-24 w-24 shrink-0 items-center justify-center rounded-md bg-bg-raised text-4xl font-bold text-white/40">
          🎵
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          <div className="text-xs uppercase tracking-wide text-fg-subtle">Playlist</div>
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
                autoFocus
                className="rounded-md border border-bg-raised bg-bg-subtle px-2 py-1 text-2xl font-semibold text-fg focus:border-accent focus:outline-none"
              />
            </div>
          ) : (
            <h1 className="truncate text-3xl font-semibold">{playlist.name}</h1>
          )}
          <div className="text-base text-fg-subtle">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"} · {formatDuration(playlist.durationSeconds)}
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <button type="button" onClick={onPlayAll} disabled={busy || tracks.length === 0} className="btn-primary">
              Play all
            </button>
            <button
              type="button"
              onClick={() => { setRenaming(true); setNewName(playlist.name); }}
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
        <div className="flex flex-col items-start gap-3 rounded-md border border-bg-raised bg-bg-subtle p-6">
          <div className="text-base font-medium text-fg">Playlist is empty</div>
          <p className="text-sm text-fg-subtle">Drag tracks here or use the queue to add them.</p>
        </div>
      ) : (
        <ol className="divide-y divide-bg-raised rounded-md border border-bg-raised">
          {tracks.map((track, index) => (
            <li
              key={track.id}
              className="grid grid-cols-[2.5rem_1fr_auto_auto] items-center gap-3 px-3 py-2 text-sm"
            >
              <div className="text-right font-mono text-xs text-fg-muted">{index + 1}</div>
              <button
                type="button"
                onClick={() => void onPlayTrack(track)}
                disabled={busy}
                className="min-w-0 text-left focus:outline-none"
              >
                <div className="truncate font-medium text-fg hover:text-white">{track.title}</div>
                <div className="truncate text-xs text-fg-subtle">{track.artist}</div>
              </button>
              <div className="text-xs text-fg-muted">{formatDuration(track.durationSeconds)}</div>
              <button
                type="button"
                onClick={() => void onRemoveTrack(index)}
                disabled={busy}
                aria-label={`Remove ${track.title}`}
                className="rounded-md p-1 text-fg-muted hover:bg-bg-raised hover:text-fg focus:outline-none disabled:opacity-40"
              >
                ✕
              </button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
