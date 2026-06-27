// GenreDetailView — top-level `/genres/:id` route. Header with the
// genre name + album/track counts, then a grid of every cached album
// that carries this genre tag, sorted by title.
//
// Pure cache-filter pattern (mirrors ArtistDetailView): no extra
// Tauri command is required because the genre tags are already
// populated on `Album.genres` during sync. The `id` URL parameter
// is the raw genre name (the Rust `Genre.id` is the name itself —
// see `crates/library/src/rows.rs:row_to_genre`), so the match is
// case-insensitive on the album side.

import { useMemo } from "react";
import { Link, useParams } from "react-router-dom";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { useLibraryStore } from "@/stores/libraryStore";

export function GenreDetailView() {
  const { id } = useParams<{ id: string }>();
  const genres = useLibraryStore((s) => s.genres);
  const albums = useLibraryStore((s) => s.albums);

  const genre = useMemo(
    () => (id ? genres.find((g) => g.id === id) ?? null : null),
    [genres, id],
  );

  const genreAlbums = useMemo(() => {
    if (!id) return [];
    const needle = id.toLowerCase();
    return albums
      .filter((a) => a.genres.some((g) => g.toLowerCase() === needle))
      .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }));
  }, [albums, id]);

  if (!id) {
    return <p className="text-sm text-muted-foreground">Missing genre id.</p>;
  }

  if (!genre) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">Genre not found</div>
        <p className="text-sm text-muted-foreground">
          The library cache doesn't have this genre. Try syncing your library.
        </p>
      </div>
    );
  }

  return (
    <section className="flex flex-col gap-6">
      <header className="flex flex-col gap-1">
        <div className="text-xs uppercase tracking-wide text-muted-foreground">Genre</div>
        <h1 className="truncate text-3xl font-semibold">{genre.name}</h1>
        <div className="text-sm text-muted-foreground">
          {genre.albumCount} {genre.albumCount === 1 ? "album" : "albums"}
          {" · "}
          {genre.trackCount} {genre.trackCount === 1 ? "track" : "tracks"}
        </div>
      </header>

      {genreAlbums.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No albums cached for this genre yet.
        </p>
      ) : (
        <ul
          className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
          aria-label={`${genre.name} albums`}
        >
          {genreAlbums.map((album) => (
            <li key={album.id}>
              <Link
                to={`/albums/${encodeURIComponent(album.id)}`}
                className="group block focus:outline-none"
              >
                <AlbumCover source={album} />
                <div className="mt-2 truncate text-sm font-medium text-foreground group-hover:text-white">
                  {album.title}
                </div>
                <div className="truncate text-xs text-muted-foreground">
                  {album.artist}
                  {album.year ? ` · ${album.year}` : ""}
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}