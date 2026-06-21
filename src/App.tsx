// App — single source of routing. Nested routes use the Layout's
// <Outlet /> so Sidebar + PlayerBar persist across navigation.
//
// `PlaybackEventsBridge` mounts the global Tauri event listeners
// once at the root so playback state stays in sync with the
// backend regardless of which view is active.

import { Route, Routes } from "react-router-dom";

import { Layout } from "./components/layout/Layout";
import { AlbumDetailView } from "./components/views/AlbumDetailView";
import { AlbumsTab } from "./components/views/AlbumsTab";
import { ArtistDetailView } from "./components/views/ArtistDetailView";
import { ArtistsTab } from "./components/views/ArtistsTab";
import { HomeView } from "./components/views/HomeView";
import { LibraryView } from "./components/views/LibraryView";
import { QueueView } from "./components/views/QueueView";
import { SearchView } from "./components/views/SearchView";
import { SettingsView } from "./components/views/SettingsView";
import { TracksTab } from "./components/views/TracksTab";
import { usePlaybackEvents } from "./hooks/usePlaybackEvents";

function PlaybackEventsBridge(): null {
  usePlaybackEvents();
  return null;
}

export default function App() {
  return (
    <>
      <PlaybackEventsBridge />
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
          <Route path="*" element={<HomeView />} />
        </Route>
      </Routes>
    </>
  );
}
