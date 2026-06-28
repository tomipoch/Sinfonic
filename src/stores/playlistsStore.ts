// Playlist store — playlists list + active playlist detail.

import { create } from "zustand";
import { extractError } from "@/lib/errors";
import {
  createPlaylist,
  deletePlaylist,
  playAlbum,
  playlistDetail,
  playlistsGet,
  removePlaylistEntries,
  renamePlaylist,
} from "@/lib/tauri";
import type { Playlist, Track } from "@/types/domain";

export interface PlaylistWithTracks {
  playlist: Playlist;
  tracks: Track[];
}

interface PlaylistsStore {
  playlists: Playlist[];
  loading: boolean;
  error: string | null;
  detail: PlaylistWithTracks | null;
  detailLoading: boolean;
  detailError: string | null;

  loadPlaylists: () => Promise<void>;
  loadPlaylistDetail: (playlistId: string) => Promise<void>;
  createPlaylist: (name: string, trackIds?: string[]) => Promise<string>;
  renamePlaylist: (playlistId: string, name: string) => Promise<void>;
  deletePlaylist: (playlistId: string) => Promise<void>;
  removePlaylistEntries: (playlistId: string, entryIds: string[]) => Promise<void>;
  playPlaylist: (playlistId: string) => Promise<void>;
  reset: () => void;
}

export const usePlaylistsStore = create<PlaylistsStore>((set, get) => ({
  playlists: [],
  loading: false,
  error: null,
  detail: null,
  detailLoading: false,
  detailError: null,

  loadPlaylists: async () => {
    set({ loading: true, error: null });
    try {
      const playlists = await playlistsGet();
      set({ playlists, loading: false, error: null });
    } catch (e) {
      set({ loading: false, error: extractError(e, "couldn't load playlists") });
    }
  },

  loadPlaylistDetail: async (playlistId: string) => {
    set({ detailLoading: true, detailError: null });
    try {
      const detail = await playlistDetail(playlistId);
      set({ detail, detailLoading: false });
    } catch (e) {
      set({ detailLoading: false, detailError: extractError(e, "couldn't load playlist") });
    }
  },

  createPlaylist: async (name: string, trackIds: string[] = []) => {
    const id = await createPlaylist(name, trackIds);
    await get().loadPlaylists();
    return id;
  },

  renamePlaylist: async (playlistId: string, name: string) => {
    await renamePlaylist(playlistId, name);
    await get().loadPlaylists();
    const detail = get().detail;
    if (detail?.playlist.id === playlistId) {
      set({ detail: { ...detail, playlist: { ...detail.playlist, name } } });
    }
  },

  deletePlaylist: async (playlistId: string) => {
    await deletePlaylist(playlistId);
    set({ detail: null });
    await get().loadPlaylists();
  },

  removePlaylistEntries: async (playlistId: string, entryIds: string[]) => {
    await removePlaylistEntries(playlistId, entryIds);
    await get().loadPlaylistDetail(playlistId);
  },

  playPlaylist: async (playlistId: string) => {
    const detail = get().detail;
    if (!detail || detail.playlist.id !== playlistId) {
      await get().loadPlaylistDetail(playlistId);
    }
    const d = get().detail;
    if (d && d.tracks.length > 0) {
      await playAlbum(d.tracks);
    }
  },

  reset: () => {
    set({
      playlists: [],
      loading: false,
      error: null,
      detail: null,
      detailLoading: false,
      detailError: null,
    });
  },
}));
