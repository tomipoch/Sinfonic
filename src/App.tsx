// App — single source of routing. Nested routes use the Layout's
// <Outlet /> so Sidebar + PlayerBar persist across navigation.
//
// `PlaybackProvider` owns the global playback state (current track,
// transport controls, repeat/shuffle). Components consume it via
// `usePlaybackContext()`. `AlbumArtPrewarm` fires
// `provider_image_bytes` for the first 24 albums after the library
// loads so the visible grid paints without per-cell lag.
// `ServerGate` hydrates the server store and routes the user
// between `/setup` and the rest of the app based on whether a
// server is connected.

import { useEffect } from "react";
import { Route, Routes } from "react-router-dom";

import { Layout } from "@/components/layout/Layout";
import { ServerGate } from "@/components/ServerGate";
import { AlbumDetailView } from "@/components/views/AlbumDetailView";
import { AlbumsView } from "@/components/views/AlbumsView";
import { ArtistDetailView } from "@/components/views/ArtistDetailView";
import { ArtistsView } from "@/components/views/ArtistsView";
import { FavoritesView } from "@/components/views/FavoritesView";
import { GenreDetailView } from "@/components/views/GenreDetailView";
import { GenresView } from "@/components/views/GenresView";
import { HomeView } from "@/components/views/HomeView";
import { LoadingView } from "@/components/views/LoadingView";
import { PlaylistDetailView } from "@/components/views/PlaylistDetailView";
import { PlaylistsView } from "@/components/views/PlaylistsView";
import { QueueView } from "@/components/views/QueueView";
import { SearchView } from "@/components/views/SearchView";
import { SetupView } from "@/components/views/SetupView";
import { SmartPlaylistDetailView } from "@/components/views/SmartPlaylistDetailView";
import { SmartPlaylistsView } from "@/components/views/SmartPlaylistsView";
import { SongsView } from "@/components/views/SongsView";
import { useAlbumArtPrewarm } from "@/hooks/useAlbumArtPrewarm";
import { revokeAll } from "@/lib/albumArtCache";
import { PlaybackProvider } from "@/playback";
import { useServerStore } from "@/stores/serverStore";

function AlbumArtPrewarm(): null {
  useAlbumArtPrewarm();
  return null;
}

function AlbumArtCacheReset(): null {
  const activeServerId = useServerStore((s) => s.activeServerId);
  useEffect(() => {
    // Blob URLs are server-scoped. When the user switches library
    // (login, logout, server switch) drop every entry — the next
    // prewarm pass will refill the cache for the new server.
    revokeAll();
  }, [activeServerId]);
  return null;
}

export default function App() {
  return (
    <PlaybackProvider>
      <AlbumArtPrewarm />
      <AlbumArtCacheReset />
      <ServerGate>
        <Routes>
          <Route path="/setup" element={<SetupView />} />
          <Route path="/loading" element={<LoadingView />} />
          <Route element={<Layout />}>
            <Route index element={<HomeView />} />
            {/* Dedicated top-level library routes — the sidebar
                links to these directly rather than to a tab inside
                a /library shell. */}
            <Route path="songs" element={<SongsView />} />
            <Route path="albums" element={<AlbumsView />} />
            <Route path="albums/:id" element={<AlbumDetailView />} />
            <Route path="artists" element={<ArtistsView />} />
            <Route path="artists/:id" element={<ArtistDetailView />} />
            <Route path="genres" element={<GenresView />} />
            <Route path="genres/:id" element={<GenreDetailView />} />
            <Route path="queue" element={<QueueView />} />
            <Route path="search" element={<SearchView />} />
            <Route path="playlists" element={<PlaylistsView />} />
            <Route path="playlists/:id" element={<PlaylistDetailView />} />
            <Route path="favorites" element={<FavoritesView />} />
            <Route path="smart-playlists" element={<SmartPlaylistsView />} />
            <Route path="smart-playlists/:id" element={<SmartPlaylistDetailView />} />
            <Route path="*" element={<HomeView />} />
          </Route>
        </Routes>
      </ServerGate>
    </PlaybackProvider>
  );
}
