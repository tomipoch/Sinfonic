// GenreDetailView — top-level `/genres/:id` route. Header with the
// genre name + album/track counts, then two sections:
//   - Albums grid (backed by `get_albums_by_genre`)
//   - Tracks list (backed by `get_tracks_by_genre`)
//
// Both commands are server-scoped paged queries; we load every
// page sequentially until we hit `total`. The `:id` URL param is
// the raw genre name (the Rust `Genre.id` is the name itself —
// see `crates/library/src/rows.rs:row_to_genre`).

import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { AlbumCard } from "@/components/ui/AlbumCard";
import { MarqueeText } from "@/components/ui/MarqueeText";
import { type TrackColumn, TrackTable } from "@/components/ui/TrackTable";
import { extractError } from "@/lib/errors";
import { getAlbumsByGenre, getTracksByGenre, playTrackWithContext } from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
import type { Album, Track } from "@/types/domain";

const TRACK_COLUMNS: TrackColumn[] = [
  { kind: "cover" },
  { kind: "song" },
  { kind: "album" },
  { kind: "time" },
  { kind: "favorite" },
  { kind: "menu" },
];

const PAGE_SIZE = 100;

export function GenreDetailView() {
  const { id } = useParams<{ id: string }>();
  const genres = useLibraryStore((s) => s.genres);
  const activeServerId = useServerStore((s) => s.activeServerId);

  const [albums, setAlbums] = useState<Album[]>([]);
  const [albumsTotal, setAlbumsTotal] = useState(0);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [tracksTotal, setTracksTotal] = useState(0);
  const [loadingAlbums, setLoadingAlbums] = useState(true);
  const [loadingTracks, setLoadingTracks] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const genre = useMemo(
    () => (id ? (genres.find((g) => g.id === id) ?? null) : null),
    [genres, id],
  );

  useEffect(() => {
    if (!id || !activeServerId) {
      setLoadingAlbums(false);
      setLoadingTracks(false);
      return;
    }
    let cancelled = false;
    setLoadingAlbums(true);
    setLoadingTracks(true);
    setError(null);

    const loadAllAlbums = async () => {
      const acc: Album[] = [];
      let offset = 0;
      while (!cancelled) {
        try {
          const page = await getAlbumsByGenre(id, offset, PAGE_SIZE);
          acc.push(...page.items);
          setAlbumsTotal(page.total);
          offset += page.items.length;
          if (offset >= page.total || page.items.length === 0) break;
        } catch (err) {
          if (!cancelled) setError(extractError(err, "Couldn't load albums"));
          return;
        }
      }
      if (!cancelled) setAlbums(acc);
    };

    const loadAllTracks = async () => {
      const acc: Track[] = [];
      let offset = 0;
      while (!cancelled) {
        try {
          const page = await getTracksByGenre(id, offset, PAGE_SIZE);
          acc.push(...page.items);
          setTracksTotal(page.total);
          offset += page.items.length;
          if (offset >= page.total || page.items.length === 0) break;
        } catch (err) {
          if (!cancelled) setError(extractError(err, "Couldn't load tracks"));
          return;
        }
      }
      if (!cancelled) setTracks(acc);
    };

    void loadAllAlbums().then(() => {
      if (!cancelled) setLoadingAlbums(false);
    });
    void loadAllTracks().then(() => {
      if (!cancelled) setLoadingTracks(false);
    });

    return () => {
      cancelled = true;
    };
  }, [id, activeServerId]);

  if (!id) {
    return <p className="p-6 text-sm text-muted-foreground">Missing genre id.</p>;
  }

  if (!genre) {
    return (
      <div className="m-6 flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">Genre not found</div>
        <p className="text-sm text-muted-foreground">
          The library cache doesn't have this genre. Try syncing your library.
        </p>
      </div>
    );
  }

  const onPlayTrack = async (track: Track) => {
    try {
      await playTrackWithContext(track, null);
    } catch (err) {
      setError(extractError(err, "Couldn't play track"));
    }
  };

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-col gap-1">
        <h1 className="text-3xl font-semibold">
          <MarqueeText>{genre.name}</MarqueeText>
        </h1>
        <div className="text-sm text-muted-foreground">
          {albumsTotal} {albumsTotal === 1 ? "album" : "albums"} · {tracksTotal}{" "}
          {tracksTotal === 1 ? "track" : "tracks"}
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}
      </header>

      {/* Albums */}
      <div className="flex flex-col gap-3">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Albums
        </h2>
        {loadingAlbums && albums.length === 0 ? (
          <p className="text-sm text-muted-foreground" role="status">
            Loading albums…
          </p>
        ) : albums.length === 0 ? (
          <p className="text-sm text-muted-foreground">No albums for this genre yet.</p>
        ) : (
          <ul
            className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
            aria-label={`${genre.name} albums`}
          >
            {albums.map((album) => (
              <li key={album.id}>
                <AlbumCard album={album} />
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Tracks */}
      <div className="flex flex-col gap-3">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Tracks
        </h2>
        {loadingTracks && tracks.length === 0 ? (
          <p className="text-sm text-muted-foreground" role="status">
            Loading tracks…
          </p>
        ) : tracks.length === 0 ? (
          <p className="text-sm text-muted-foreground">No tracks for this genre yet.</p>
        ) : (
          <TrackTable
            tracks={tracks}
            columns={TRACK_COLUMNS}
            onPlayTrack={(t) => void onPlayTrack(t)}
            sortableColumns={["title", "artist", "album", "durationSeconds"]}
            draggable={false}
            dragSource="genre-detail"
          />
        )}
      </div>

      {(loadingAlbums || loadingTracks) && (albums.length > 0 || tracks.length > 0) && (
        <p className="text-xs text-muted-foreground" role="status">
          Loading more…
        </p>
      )}
    </section>
  );
}
