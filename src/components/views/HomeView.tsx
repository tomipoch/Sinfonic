// HomeView — main landing page with scrolleable content sections.
//
// Sections:
//   - Welcome header
//   - Recently Added (albums)
//   - Artists
//   - Genres (placeholder pills)

import { HorizontalSection } from "@/components/ui/HorizontalSection";
import { GenreChip } from "@/components/ui/GenreChip";
import { AlbumCard } from "@/components/ui/AlbumCard";
import { ArtistCard } from "@/components/ui/ArtistCard";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";

const PLACEHOLDER_GENRES = [
  "Rock", "Pop", "Jazz", "Classical", "Electronic",
  "Hip-Hop", "Country", "R&B", "Metal", "Folk",
];

export function HomeView() {
  const albums = useLibraryStore((s) => s.albums);
  const artists = useLibraryStore((s) => s.artists);
  const activeServerId = useServerStore((s) => s.activeServerId);

  const recentAlbums = albums.slice(0, 12);
  const recentArtists = artists.slice(0, 8);

  return (
    <div className="flex flex-col gap-8 p-6">
      {/* Welcome */}
      <section>
        <h1 className="text-2xl font-semibold text-foreground">
          {activeServerId ? "Welcome back" : "Welcome to Sinfonic"}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {activeServerId
            ? "Browse your library or search for something to play."
            : "Connect a server in the sidebar to get started."}
        </p>
      </section>

      {/* Recently Added Albums */}
      {recentAlbums.length > 0 && (
        <HorizontalSection title="Recently Added">
          {recentAlbums.map((album) => (
            <AlbumCard key={album.id} album={album} />
          ))}
        </HorizontalSection>
      )}

      {/* Artists */}
      {recentArtists.length > 0 && (
        <HorizontalSection title="Artists">
          {recentArtists.map((artist) => (
            <ArtistCard key={artist.id} artist={artist} />
          ))}
        </HorizontalSection>
      )}

      {/* Genres */}
      <HorizontalSection title="Genres">
        {PLACEHOLDER_GENRES.map((genre) => (
          <GenreChip key={genre} label={genre} />
        ))}
      </HorizontalSection>
    </div>
  );
}
