// Artists list for /library/artists. The cache is already loaded by
// `useLibraryAutoLoad` in the parent route, so this is a pure read.

import { Link } from "react-router-dom";

import { useLibraryStore } from "@/stores/libraryStore";

export function ArtistsTab() {
  const artists = useLibraryStore((s) => s.artists);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);

  if (loading && artists.length === 0) {
    return (
      <p className="text-muted-foreground text-sm" role="status">
        Loading artists…
      </p>
    );
  }

  if (loaded && artists.length === 0) {
    return (
      <p className="text-muted-foreground text-sm">
        No artists in the library yet. Sync your library to populate it.
      </p>
    );
  }

  return (
    <ul
      className="divide-y divide-border rounded-md border border-border"
      aria-label="Artists"
    >
      {artists.map((artist) => (
        <li key={artist.id}>
          <Link
            to={`/library/artist/${encodeURIComponent(artist.id)}`}
            className="flex items-center justify-between gap-3 px-3 py-2 hover:bg-muted"
          >
            <div className="min-w-0">
              <div className="truncate text-sm font-medium text-foreground">{artist.name}</div>
              <div className="truncate text-xs text-muted-foreground">
                {artist.albumCount} {artist.albumCount === 1 ? "album" : "albums"}
                {" · "}
                {artist.trackCount} {artist.trackCount === 1 ? "track" : "tracks"}
              </div>
            </div>
            {artist.favorite && (
              <span className="text-xs text-primary" aria-label="Favorite">
                ★
              </span>
            )}
          </Link>
        </li>
      ))}
    </ul>
  );
}
