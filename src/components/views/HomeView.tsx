// HomeView — main landing page with scrolleable content sections.
//
// Sections:
//   - Welcome header
//   - Recently Added (albums)
//   - Artists
//   - Genres (placeholder pills)
//
// When the library is empty but a server is connected the empty
// section offers a "Sync now" CTA that triggers `provider_sync_library`
// and bounces to `/loading` so the user sees the full sync progress
// (connecting → scanning → indexing → caching → ready).

import { useNavigate } from "react-router-dom";
import { AlbumCard } from "@/components/ui/AlbumCard";
import { ArtistCard } from "@/components/ui/ArtistCard";
import { GenreChip } from "@/components/ui/GenreChip";
import { HorizontalSection } from "@/components/ui/HorizontalSection";
import { SyncOverlay } from "@/components/ui/SyncOverlay";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";

const PLACEHOLDER_GENRES = [
  "Rock",
  "Pop",
  "Jazz",
  "Classical",
  "Electronic",
  "Hip-Hop",
  "Country",
  "R&B",
  "Metal",
  "Folk",
];

export function HomeView() {
  const navigate = useNavigate();
  const albums = useLibraryStore((s) => s.albums);
  const artists = useLibraryStore((s) => s.artists);
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const lastSync = useServerStore((s) => s.lastSync);

  const activeServer = servers.find((s) => s.id === activeServerId);
  const recentAlbums = albums.slice(0, 12);
  const recentArtists = artists.slice(0, 8);
  const isEmpty = albums.length === 0 && artists.length === 0;

  const handleSync = () => {
    // LoadingView owns the actual sync call — just bounce there and
    // let it kick `provider_sync_library` exactly once.
    void navigate("/loading", { replace: true });
  };

  return (
    <div className="flex flex-col gap-8 p-6">
      {/* Welcome */}
      <section>
        <h1 className="text-2xl font-semibold text-foreground">Welcome back</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {activeServer
            ? `Browsing ${activeServer.name}.`
            : "Browse your library or search for something to play."}
        </p>
      </section>

      {/* Empty state when the library cache is empty but a server
          is connected — invite the user to trigger a sync. */}
      {isEmpty && activeServerId ? (
        <section className="flex flex-col items-center gap-4 rounded-lg border border-dashed border-border bg-card/40 px-6 py-10 text-center">
          <div className="flex flex-col gap-1">
            <h2 className="text-base font-medium text-foreground">Your library is empty</h2>
            <p className="text-sm text-muted-foreground">
              Sync to pull albums, artists, and tracks from your server.
            </p>
          </div>
          <button
            type="button"
            onClick={() => void handleSync()}
            disabled={lastSync === "syncing"}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
          >
            {lastSync === "syncing" ? "Syncing…" : "Sync library"}
          </button>
          <SyncOverlay />
        </section>
      ) : null}

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
      {recentAlbums.length > 0 && (
        <HorizontalSection title="Genres">
          {PLACEHOLDER_GENRES.map((genre) => (
            <GenreChip key={genre} label={genre} />
          ))}
        </HorizontalSection>
      )}
    </div>
  );
}
