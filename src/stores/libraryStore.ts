// Library store — cached albums / artists / tracks views.
//
// Fetch actions hit the SQLite cache via the typed IPC wrappers in
// `lib/tauri.ts`. The store is the single source of truth for the
// UI: components subscribe to it instead of calling `getAlbums`
// directly so loading/error state lives in one place.

import { create } from "zustand";

import {
  getAlbums,
  getArtists,
  getTracks,
} from "@/lib/tauri";
import type { Album, Artist, Track } from "@/types/domain";

const PAGE_SIZE = 200;

export interface LibraryStore {
  albums: Album[];
  artists: Artist[];
  tracks: Track[];

  loading: boolean;
  loaded: boolean;
  error: string | null;

  loadAlbums: () => Promise<void>;
  loadArtists: () => Promise<void>;
  loadTracks: () => Promise<void>;
  loadAll: () => Promise<void>;
  reset: () => void;
}

export const useLibraryStore = create<LibraryStore>((set) => ({
  albums: [],
  artists: [],
  tracks: [],

  loading: false,
  loaded: false,
  error: null,

  loadAlbums: async () => {
    set({ loading: true, error: null });
    try {
      const page = await getAlbums(0, PAGE_SIZE);
      set({ albums: page.items, loading: false, loaded: true });
    } catch (e) {
      set({ loading: false, error: (e as Error).message ?? String(e) });
    }
  },

  loadArtists: async () => {
    set({ loading: true, error: null });
    try {
      const page = await getArtists(0, PAGE_SIZE);
      set({ artists: page.items, loading: false, loaded: true });
    } catch (e) {
      set({ loading: false, error: (e as Error).message ?? String(e) });
    }
  },

  loadTracks: async () => {
    set({ loading: true, error: null });
    try {
      const page = await getTracks(0, PAGE_SIZE);
      set({ tracks: page.items, loading: false, loaded: true });
    } catch (e) {
      set({ loading: false, error: (e as Error).message ?? String(e) });
    }
  },

  loadAll: async () => {
    set({ loading: true, error: null });
    try {
      const [albums, artists, tracks] = await Promise.all([
        getAlbums(0, PAGE_SIZE),
        getArtists(0, PAGE_SIZE),
        getTracks(0, PAGE_SIZE),
      ]);
      set({
        albums: albums.items,
        artists: artists.items,
        tracks: tracks.items,
        loading: false,
        loaded: true,
      });
    } catch (e) {
      set({ loading: false, error: (e as Error).message ?? String(e) });
    }
  },

  reset: () => {
    set({
      albums: [],
      artists: [],
      tracks: [],
      loading: false,
      loaded: false,
      error: null,
    });
  },
}));
