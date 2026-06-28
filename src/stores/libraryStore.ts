// Library store — cached albums / artists / genres / tracks views.
//
// Fetch actions hit the SQLite cache via the typed IPC wrappers in
// `lib/tauri.ts`. The store is the single source of truth for the
// UI: components subscribe to it instead of calling `getAlbums`
// directly so loading/error state lives in one place.
//
// P1: real pagination. Each list (albums / artists / tracks) is
// loaded one page at a time. The initial load via `loadAll` (or
// the per-list `loadX`) fetches the first `PAGE_SIZE` rows;
// subsequent pages are appended by `loadMoreX` (called by the
// view's scroll-sentinel `IntersectionObserver`). `*FullyLoaded`
// flips to `true` once the store has as many rows as `*Total`
// reports.

import { create } from "zustand";

import { extractError } from "@/lib/errors";
import { getAlbums, getArtists, getGenres, getTracks } from "@/lib/tauri";
import type { Album, Artist, Genre, Track } from "@/types/domain";

export const PAGE_SIZE = 200;

export interface LibraryStore {
  albums: Album[];
  artists: Artist[];
  genres: Genre[];
  tracks: Track[];

  loading: boolean;
  loaded: boolean;
  error: string | null;

  // Per-list pagination state.
  albumsTotal: number;
  artistsTotal: number;
  tracksTotal: number;
  loadingMoreAlbums: boolean;
  loadingMoreArtists: boolean;
  loadingMoreTracks: boolean;

  loadAlbums: () => Promise<void>;
  loadArtists: () => Promise<void>;
  loadGenres: () => Promise<void>;
  loadTracks: () => Promise<void>;
  loadAll: () => Promise<void>;
  loadMoreAlbums: () => Promise<void>;
  loadMoreArtists: () => Promise<void>;
  loadMoreTracks: () => Promise<void>;
  reset: () => void;
}

export const useLibraryStore = create<LibraryStore>((set, get) => ({
  albums: [],
  artists: [],
  genres: [],
  tracks: [],

  loading: false,
  loaded: false,
  error: null,

  albumsTotal: 0,
  artistsTotal: 0,
  tracksTotal: 0,
  loadingMoreAlbums: false,
  loadingMoreArtists: false,
  loadingMoreTracks: false,

  loadAlbums: async () => {
    set({ loading: true, error: null });
    try {
      const page = await getAlbums(0, PAGE_SIZE);
      set({
        albums: page.items,
        albumsTotal: page.total,
        loading: false,
        loaded: true,
      });
    } catch (e) {
      set({ loading: false, error: extractError(e, "couldn't load albums") });
    }
  },

  loadArtists: async () => {
    set({ loading: true, error: null });
    try {
      const page = await getArtists(0, PAGE_SIZE);
      set({
        artists: page.items,
        artistsTotal: page.total,
        loading: false,
        loaded: true,
      });
    } catch (e) {
      set({ loading: false, error: extractError(e, "couldn't load artists") });
    }
  },

  loadGenres: async () => {
    set({ loading: true, error: null });
    try {
      const genres = await getGenres();
      set({ genres, loading: false, loaded: true });
    } catch (e) {
      set({ loading: false, error: extractError(e, "couldn't load genres") });
    }
  },

  loadTracks: async () => {
    set({ loading: true, error: null });
    try {
      const page = await getTracks(0, PAGE_SIZE);
      set({
        tracks: page.items,
        tracksTotal: page.total,
        loading: false,
        loaded: true,
      });
    } catch (e) {
      set({ loading: false, error: extractError(e, "couldn't load tracks") });
    }
  },

  loadAll: async () => {
    set({ loading: true, error: null });
    try {
      const [albums, artists, tracks, genres] = await Promise.all([
        getAlbums(0, PAGE_SIZE),
        getArtists(0, PAGE_SIZE),
        getTracks(0, PAGE_SIZE),
        getGenres(),
      ]);
      set({
        albums: albums.items,
        albumsTotal: albums.total,
        artists: artists.items,
        artistsTotal: artists.total,
        tracks: tracks.items,
        tracksTotal: tracks.total,
        genres,
        loading: false,
        loaded: true,
      });
    } catch (e) {
      set({ loading: false, error: extractError(e, "couldn't load library") });
    }
  },

  loadMoreAlbums: async () => {
    const { albums, albumsTotal, loadingMoreAlbums } = get();
    if (loadingMoreAlbums) return;
    if (albums.length >= albumsTotal && albumsTotal > 0) return;
    set({ loadingMoreAlbums: true });
    try {
      const page = await getAlbums(albums.length, PAGE_SIZE);
      // Dedupe by id: the backend cache may have shifted between
      // page reads if the user synced mid-fetch.
      const seen = new Set(albums.map((a) => a.id));
      const merged = albums.concat(page.items.filter((a) => !seen.has(a.id)));
      set({
        albums: merged,
        albumsTotal: Math.max(albumsTotal, page.total),
        loadingMoreAlbums: false,
      });
    } catch (e) {
      set({
        loadingMoreAlbums: false,
        error: extractError(e, "couldn't load more albums"),
      });
    }
  },

  loadMoreArtists: async () => {
    const { artists, artistsTotal, loadingMoreArtists } = get();
    if (loadingMoreArtists) return;
    if (artists.length >= artistsTotal && artistsTotal > 0) return;
    set({ loadingMoreArtists: true });
    try {
      const page = await getArtists(artists.length, PAGE_SIZE);
      const seen = new Set(artists.map((a) => a.id));
      const merged = artists.concat(page.items.filter((a) => !seen.has(a.id)));
      set({
        artists: merged,
        artistsTotal: Math.max(artistsTotal, page.total),
        loadingMoreArtists: false,
      });
    } catch (e) {
      set({
        loadingMoreArtists: false,
        error: extractError(e, "couldn't load more artists"),
      });
    }
  },

  loadMoreTracks: async () => {
    const { tracks, tracksTotal, loadingMoreTracks } = get();
    if (loadingMoreTracks) return;
    if (tracks.length >= tracksTotal && tracksTotal > 0) return;
    set({ loadingMoreTracks: true });
    try {
      const page = await getTracks(tracks.length, PAGE_SIZE);
      const seen = new Set(tracks.map((t) => t.id));
      const merged = tracks.concat(page.items.filter((t) => !seen.has(t.id)));
      set({
        tracks: merged,
        tracksTotal: Math.max(tracksTotal, page.total),
        loadingMoreTracks: false,
      });
    } catch (e) {
      set({
        loadingMoreTracks: false,
        error: extractError(e, "couldn't load more tracks"),
      });
    }
  },

  reset: () => {
    set({
      albums: [],
      artists: [],
      genres: [],
      tracks: [],
      loading: false,
      loaded: false,
      error: null,
      albumsTotal: 0,
      artistsTotal: 0,
      tracksTotal: 0,
      loadingMoreAlbums: false,
      loadingMoreArtists: false,
      loadingMoreTracks: false,
    });
  },
}));
