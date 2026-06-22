// Artist detail view — header with album + track counts and a grid
// of the artist's albums (filtered from the cached `albums` list).
//
// Reads from the local SQLite cache so it works offline; the same
// `useLibraryAutoLoad` hook that powers the grid already populated
// it.

import { useMemo } from "react";
import { Link, useParams } from "react-router-dom";

import { AlbumCover } from "../ui/AlbumCover";
import { FavoriteButton } from "../ui/FavoriteButton";
import { useLibraryStore } from "../../stores/libraryStore";

export function ArtistDetailView() {
  const { id } = useParams<{ id: string }>();
  const artists = useLibraryStore((s) => s.artists);
  const albums = useLibraryStore((s) => s.albums);

  const artist = useMemo(
    () => (id ? artists.find((a) => a.id === id) ?? null : null),
    [artists, id],
  );

  const artistAlbums = useMemo(
    () =>
      id
        ? albums
            .filter((a) => a.artistId === id)
            .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }))
        : [],
    [albums, id],
  );

  if (!id) {
    return <p className="text-fg-subtle text-sm">Missing artist id.</p>;
  }

  if (!artist) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-bg-raised bg-bg-subtle p-6">
        <div className="text-base font-medium text-fg">Artist not found</div>
        <p className="text-sm text-fg-subtle">
          The library cache doesn't have this artist. Try syncing your library.
        </p>
      </div>
    );
  }

  return (
    <section className="flex flex-col gap-6">
      <header className="flex flex-col gap-1">
        <div className="text-xs uppercase tracking-wide text-fg-subtle">Artist</div>
        <h1 className="truncate text-3xl font-semibold">{artist.name}</h1>
        <div className="flex items-center gap-3 text-sm text-fg-subtle">
          <span>
            {artist.albumCount} {artist.albumCount === 1 ? "album" : "albums"}
            {" · "}
            {artist.trackCount} {artist.trackCount === 1 ? "track" : "tracks"}
          </span>
          <FavoriteButton kind="artist" itemId={artist.id} initialFavorite={artist.favorite} />
        </div>
      </header>

      {artistAlbums.length === 0 ? (
        <p className="text-fg-subtle text-sm">
          No albums cached for this artist yet.
        </p>
      ) : (
        <ul
          className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
          aria-label={`${artist.name} albums`}
        >
          {artistAlbums.map((album) => (
            <li key={album.id}>
              <Link
                to={`/library/album/${encodeURIComponent(album.id)}`}
                className="group block focus:outline-none"
              >
                <AlbumCover album={album} />
                <div className="mt-2 truncate text-sm font-medium text-fg group-hover:text-white">
                  {album.title}
                </div>
                {album.year ? (
                  <div className="text-xs text-fg-subtle">{album.year}</div>
                ) : null}
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
