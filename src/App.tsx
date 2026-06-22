// App — single source of routing. Nested routes use the Layout's
// <Outlet /> so Sidebar + PlayerBar persist across navigation.
//
// `PlaybackEventsBridge` mounts the global Tauri event listeners
// once at the root so playback state stays in sync with the
// backend regardless of which view is active. `AlbumArtPrewarm`
// fires `provider_image_bytes` for the first 24 albums after the
// library loads so the visible grid paints without per-cell lag.

import { Route, Routes } from "react-router-dom";

import { Layout } from "./components/layout/Layout";
import { AlbumDetailView } from "./components/views/AlbumDetailView";
import { AlbumsTab } from "./components/views/AlbumsTab";
import { ArtistDetailView } from "./components/views/ArtistDetailView";
import { ArtistsTab } from "./components/views/ArtistsTab";
import { HomeView } from "./components/views/HomeView";
import { LibraryView } from "./components/views/LibraryView";
import { FavoritesView } from "./components/views/FavoritesView";
import { PlaylistDetailView } from "./components/views/PlaylistDetailView";
import { PlaylistsView } from "./components/views/PlaylistsView";
import { QueueView } from "./components/views/QueueView";
import { SearchView } from "./components/views/SearchView";
import { SettingsView } from "./components/views/SettingsView";
import { TracksTab } from "./components/views/TracksTab";
import { useAlbumArtPrewarm } from "./hooks/useAlbumArtPrewarm";
import { usePlaybackEvents } from "./hooks/usePlaybackEvents";

function PlaybackEventsBridge(): null {
  usePlaybackEvents();
  return null;
}

function AlbumArtPrewarm(): null {
  useAlbumArtPrewarm();
  return null;
}

export default function App() {
  return (
    <>
      <PlaybackEventsBridge />
      <AlbumArtPrewarm />
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<HomeView />} />
          <Route path="library" element={<LibraryView />}>
            <Route index element={<AlbumsTab />} />
            <Route path="artists" element={<ArtistsTab />} />
            <Route path="tracks" element={<TracksTab />} />
            <Route path="album/:id" element={<AlbumDetailView />} />
            <Route path="artist/:id" element={<ArtistDetailView />} />
          </Route>
          <Route path="queue" element={<QueueView />} />
          <Route path="search" element={<SearchView />} />
          <Route path="settings" element={<SettingsView />} />
          <Route path="playlists" element={<PlaylistsView />} />
          <Route path="playlists/:id" element={<PlaylistDetailView />} />
          <Route path="favorites" element={<FavoritesView />} />
          <Route path="*" element={<HomeView />} />
        </Route>
      </Routes>
    </>
  );
}
