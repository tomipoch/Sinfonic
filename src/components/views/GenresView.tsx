// GenresView — top-level /genres route. Tile of every genre present
// in the active server's cache, with the album and track counts that
// came back from `get_genres`. Each chip is a `Link` to the
// genre-specific detail view (`/genres/:id` → `GenreDetailView`).

import { useEffect } from "react";
import { Link } from "react-router-dom";

import { EmptyState } from "@/components/ui/EmptyState";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";

export function GenresView() {
  const genres = useLibraryStore((s) => s.genres);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
  const loadGenres = useLibraryStore((s) => s.loadGenres);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);
  const syncLibrary = useServerStore((s) => s.syncLibrary);

  // The shared `useLibraryAutoLoad` hook already preloads albums,
  // artists and tracks, but genres are a separate query. Fetch them
  // on mount and whenever the active server changes.
  useEffect(() => {
    if (activeServerId) {
      void loadGenres();
    }
  }, [activeServerId, loadGenres]);

  if (!activeServerId) {
    return <p className="text-sm text-muted-foreground">Connect a server to see your genres.</p>;
  }

  if (loading && genres.length === 0) {
    return (
      <p className="text-sm text-muted-foreground" role="status">
        Loading genres…
      </p>
    );
  }

  if (loaded && genres.length === 0) {
    return (
      <EmptyState
        title="No genres yet"
        description="Your library doesn't have genre tags on its albums yet. Sync from the provider to see them appear here."
        syncLabel="Sync library"
        syncing={lastSync === "syncing"}
        onSync={() => syncLibrary()}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4 p-6">
      <header>
        <h1 className="text-2xl font-semibold text-foreground">Genres</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {genres.length} {genres.length === 1 ? "genre" : "genres"} across the active library
        </p>
      </header>

      <ul
        className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5"
        aria-label="Genres"
      >
        {genres.map((genre) => (
          <li key={genre.id}>
            <Link
              to={`/genres/${encodeURIComponent(genre.id)}`}
              className="flex items-center justify-between gap-3 rounded-md border border-border bg-card px-3 py-2.5 transition-colors hover:border-primary/60 hover:bg-card/80 focus:outline-none focus:ring-2 focus:ring-primary/40"
              aria-label={`${genre.name} — ${genre.albumCount} albums, ${genre.trackCount} tracks`}
            >
              <div className="min-w-0">
                <div className="truncate text-sm font-medium text-foreground">{genre.name}</div>
                <div className="truncate text-xs text-muted-foreground">
                  {genre.albumCount} {genre.albumCount === 1 ? "album" : "albums"}
                  {" · "}
                  {genre.trackCount} {genre.trackCount === 1 ? "track" : "tracks"}
                </div>
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
