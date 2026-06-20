// Library store — cached albums / artists / tracks views.

import { create } from "zustand";

import type { Album, Artist, Track } from "../types/domain";

export interface LibraryStore {
  albums: Album[];
  artists: Artist[];
  tracks: Track[];

  setAlbums: (albums: Album[]) => void;
  setArtists: (artists: Artist[]) => void;
  setTracks: (tracks: Track[]) => void;
}

export const useLibraryStore = create<LibraryStore>((set) => ({
  albums: [],
  artists: [],
  tracks: [],

  setAlbums: (albums) => set({ albums }),
  setArtists: (artists) => set({ artists }),
  setTracks: (tracks) => set({ tracks }),
}));
