// Albums grid for /library. Reads from `useLibraryStore` and triggers
// an initial load when the active server changes (via the
// `useLibraryAutoLoad` hook in the parent route).
//
// Empty states:
// - No active server: the parent route shows a "connect a server"
//   hint, so this view only renders the grid itself.
// - Server connected but cache empty: invite the user to trigger a
//   sync from Settings.

import { Link } from "react-router-dom";
import { toast } from "sonner";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
import { formatDuration } from "@/lib/format";

export function AlbumsTab() {
  const albums = useLibraryStore((s) => s.albums);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
  const loadAlbums = useLibraryStore((s) => s.loadAlbums);

  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  const onSync = async () => {
    try {
      await syncLibrary();
      if (useServerStore.getState().lastSync === "success") {
        toast.success("Library synced");
      }
      await loadAlbums();
    } catch {
      // error already on the store
    }
  };

  if (!activeServerId) return null;

  if (loading && albums.length === 0) {
    return (
      <p className="text-muted-foreground text-sm" role="status">
        Loading albums…
      </p>
    );
  }

  if (loaded && albums.length === 0) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">Library is empty</div>
        <p className="text-sm text-muted-foreground">
          Sync the library from your provider to populate the grid.
        </p>
        <button
          type="button"
          onClick={onSync}
          disabled={lastSync === "syncing"}
          className="btn-primary"
        >
          {lastSync === "syncing" ? "Syncing…" : "Sync library"}
        </button>
      </div>
    );
  }

  return (
    <ul
      className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
      aria-label="Albums"
    >
      {albums.map((album) => (
        <li key={album.id}>
          <Link
            to={`/library/album/${encodeURIComponent(album.id)}`}
            className="group block focus:outline-none"
          >
            <AlbumCover album={album} />
            <div className="mt-2 truncate text-sm font-medium text-foreground group-hover:text-white">
              {album.title}
            </div>
            <div className="truncate text-xs text-muted-foreground">
              {album.artist}
              {album.year ? ` · ${album.year}` : ""}
            </div>
            <div className="text-xs text-foreground-muted">
              {album.trackCount} {album.trackCount === 1 ? "track" : "tracks"}
              {" · "}
              {formatDuration(album.durationSeconds)}
            </div>
          </Link>
        </li>
      ))}
    </ul>
  );
}
