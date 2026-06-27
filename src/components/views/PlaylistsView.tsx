// PlaylistsView — grid of user playlists + "New playlist" CTA.

import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { usePlaylistsStore } from "@/stores/playlistsStore";
import { useServerStore } from "@/stores/serverStore";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";

export function PlaylistsView() {
  const navigate = useNavigate();
  const activeServerId = useServerStore((s) => s.activeServerId);
  // `useShallow` keeps the selector reference stable when the picked
  // fields are reference-equal, so the view only re-renders on
  // meaningful changes (e.g. a new playlist) instead of every store
  // update (e.g. detail error flips).
  const { playlists, loading, error, loadPlaylists, createPlaylist } = usePlaylistsStore(
    useShallow((s) => ({
      playlists: s.playlists,
      loading: s.loading,
      error: s.error,
      loadPlaylists: s.loadPlaylists,
      createPlaylist: s.createPlaylist,
    })),
  );
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  useEffect(() => {
    if (activeServerId) void loadPlaylists();
  }, [activeServerId, loadPlaylists]);

  const onCreate = async () => {
    if (!newName.trim()) return;
    setCreating(true);
    try {
      const id = await createPlaylist(newName.trim());
      toast.success(`Playlist "${newName.trim()}" created`);
      setNewName("");
      setCreating(false);
      navigate(`/playlists/${encodeURIComponent(id)}`);
    } catch (e) {
      toast.error(`Failed to create playlist: ${extractError(e, "unknown error")}`);
      setCreating(false);
    }
  };

  if (!activeServerId) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">No server connected</div>
        <p className="text-sm text-muted-foreground">Connect a server in Settings to see playlists.</p>
      </div>
    );
  }

  if (loading && playlists.length === 0) {
    return <p className="text-muted-foreground text-sm" role="status">Loading playlists…</p>;
  }

  if (error) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
        <div className="text-base font-medium text-red-400">Failed to load playlists</div>
        <p className="text-sm text-red-300">{error}</p>
        <button type="button" onClick={() => void loadPlaylists()} className="btn-ghost text-sm">
          Retry
        </button>
      </div>
    );
  }

  return (
    <section className="flex flex-col gap-4 p-6">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 className="text-2xl font-semibold">Playlists</h1>
          <p className="text-sm text-muted-foreground">
            {playlists.length === 0 ? "No playlists yet" : `${playlists.length} playlists`}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {creating ? (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void onCreate();
                  if (e.key === "Escape") { setCreating(false); setNewName(""); }
                }}
                placeholder="Playlist name"
                autoFocus
                className="rounded-md border border-border bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted focus:border-primary focus:outline-none"
              />
              <button type="button" onClick={() => void onCreate()} className="btn-primary text-sm">
                Create
              </button>
              <button type="button" onClick={() => { setCreating(false); setNewName(""); }} className="btn-ghost text-sm">
                Cancel
              </button>
            </div>
          ) : (
            <button type="button" onClick={() => setCreating(true)} className="btn-primary text-sm">
              New playlist
            </button>
          )}
        </div>
      </header>

      {playlists.length === 0 && !creating ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
          <div className="text-base font-medium text-foreground">No playlists yet</div>
          <p className="text-sm text-muted-foreground">Create a playlist to organize your music.</p>
          <button type="button" onClick={() => setCreating(true)} className="btn-primary text-sm">
            Create playlist
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(16rem,1fr))] gap-4">
          {playlists.map((pl) => (
            <Link
              key={pl.id}
              to={`/playlists/${encodeURIComponent(pl.id)}`}
              className="flex flex-col gap-2 rounded-md border border-border bg-muted p-4 transition-colors hover:border-primary/50 hover:bg-card"
            >
              {pl.imageRef ? (
                <AlbumCover
                  source={{ id: pl.id, title: pl.name, imageRef: pl.imageRef }}
                  className="aspect-square w-full rounded-md"
                  ariaLabel={`Cover art for playlist ${pl.name}`}
                />
              ) : (
                <div className="flex aspect-square w-full items-center justify-center rounded-md bg-card text-5xl text-white/30">
                  🎵
                </div>
              )}
              <div className="min-w-0">
                <div className="truncate text-sm font-medium text-foreground">{pl.name}</div>
                <div className="truncate text-xs text-muted-foreground">
                  {pl.trackCount} {pl.trackCount === 1 ? "track" : "tracks"} · {formatDuration(pl.durationSeconds)}
                </div>
              </div>
            </Link>
          ))}
        </div>
      )}
    </section>
  );
}
