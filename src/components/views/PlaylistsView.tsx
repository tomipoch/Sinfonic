// PlaylistsView — grid of user playlists + "New playlist" CTA.
// Layout mirrors AlbumsView (no sort pills, no Play all).

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";

import { EmptyState } from "@/components/ui/EmptyState";
import { PlaylistCard } from "@/components/ui/PlaylistCard";
import { extractError } from "@/lib/errors";
import { usePlaylistsStore } from "@/stores/playlistsStore";
import { useServerStore } from "@/stores/serverStore";

export function PlaylistsView() {
  const navigate = useNavigate();
  const activeServerId = useServerStore((s) => s.activeServerId);
  const { playlists, loading, error, loadPlaylists, createPlaylist } = usePlaylistsStore(
    useShallow((s) => ({
      playlists: s.playlists,
      loading: s.loading,
      error: s.error,
      loadPlaylists: s.loadPlaylists,
      createPlaylist: s.createPlaylist,
    })),
  );
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);
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
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">No server connected</div>
        <p className="text-sm text-muted-foreground">
          Connect a server in Settings to see playlists.
        </p>
      </div>
    );
  }

  if (loading && playlists.length === 0) {
    return (
      <p className="p-6 text-sm text-muted-foreground" role="status">
        Loading playlists…
      </p>
    );
  }

  if (error) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6">
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
                  if (e.key === "Escape") {
                    setCreating(false);
                    setNewName("");
                  }
                }}
                placeholder="Playlist name"
                className="input max-w-xs"
                autoFocus
              />
              <button type="button" onClick={() => void onCreate()} className="btn-primary text-sm">
                Create
              </button>
              <button
                type="button"
                onClick={() => {
                  setCreating(false);
                  setNewName("");
                }}
                className="btn-ghost text-sm"
              >
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
        <EmptyState
          title="No playlists yet"
          description="Create a playlist to organize your music."
          syncLabel="Sync library"
          syncing={lastSync === "syncing"}
          onSync={() => syncLibrary()}
        />
      ) : (
        <ul
          className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
          aria-label="Playlists"
        >
          {playlists.map((pl) => (
            <li key={pl.id}>
              <PlaylistCard playlist={pl} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
