// Typed wrappers around `invoke` — the only place the frontend should
// call into Rust.
//
// DRY: every IPC call goes through this file. The command names are
// duplicated from `src-tauri/src/commands.rs`; that duplication is
// caught by `cargo check` and `pnpm build` respectively, so it costs
// nothing at runtime.

import { invoke } from "@tauri-apps/api/core";

import type {
  Album,
  Artist,
  ConnectedServer,
  DiscoveredServer,
  PagedResponse,
  PlaybackStatePayload,
  QueueSnapshot,
  SearchResults,
  Track,
} from "../types/domain";

interface AlbumDetail {
  album: Album;
  tracks: Track[];
}

// ─── Library ────────────────────────────────────────────────────

export const getAlbums = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Album>>("get_albums", { offset, limit });

export const getArtists = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Artist>>("get_artists", { offset, limit });

export const getTracks = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Track>>("get_tracks", { offset, limit });

export const getAlbumDetail = (albumId: string) =>
  invoke<AlbumDetail | null>("get_album_detail", { albumId });

// ─── Playback ───────────────────────────────────────────────────

export const getPlaybackState = () =>
  invoke<PlaybackStatePayload>("get_playback_state");

export const getQueue = () => invoke<QueueSnapshot>("get_queue");

export const playTrack = (track: Track) =>
  invoke<string>("play_track", { track });

export const playAlbum = (tracks: Track[]) =>
  invoke<void>("play_album", { tracks });

export const pause = () => invoke<void>("pause");
export const resume = () => invoke<void>("resume");
export const stop = () => invoke<void>("stop");
export const next = () => invoke<void>("next");
export const previous = () => invoke<void>("previous");

export const seek = (positionSeconds: number) =>
  invoke<void>("seek", { positionSeconds });

// ─── Queue mutations ───────────────────────────────────────────

export const queueRemove = (entryId: string) =>
  invoke<boolean>("queue_remove", { entryId });

export const queueJumpTo = (entryId: string) =>
  invoke<boolean>("queue_jump_to", { entryId });

export const queueMove = (entryId: string, targetIndex: number) =>
  invoke<void>("queue_move", { entryId, targetIndex });

export const queueClear = () => invoke<void>("queue_clear");

// ─── Repeat / shuffle ───────────────────────────────────────────

export const setRepeat = (repeat: "off" | "one" | "all") =>
  invoke<void>("set_repeat", { repeat });

export const setShuffle = (enabled: boolean) =>
  invoke<void>("set_shuffle", { enabled });

// ─── Volume ─────────────────────────────────────────────────────

export const setVolume = (volume: number) =>
  invoke<void>("set_volume", { volume });

export const setMuted = (muted: boolean) =>
  invoke<void>("set_muted", { muted });

export const setEqBand = (hz: number, gainDb: number) =>
  invoke<void>("set_eq_band", { band: { hz, gainDb } });

export const resetEq = () => invoke<void>("reset_eq");

// ─── Search ─────────────────────────────────────────────────────

export const search = (query: string, limit = 20) =>
  invoke<SearchResults>("search", { query, limit });

// ─── Provider (Jellyfin + Subsonic) ─────────────────────────────

export const jellyfinDiscover = () =>
  invoke<DiscoveredServer[]>("jellyfin_discover");

export const jellyfinLogin = (params: {
  baseUrl: string;
  username: string;
  password: string;
}) =>
  invoke<ConnectedServer>("jellyfin_login", { request: params });

export const subsonicLogin = (params: {
  baseUrl: string;
  username: string;
  password: string;
}) =>
  invoke<ConnectedServer>("subsonic_login", { request: params });

export const providerLogout = () => invoke<void>("provider_logout");

export const providerServers = () =>
  invoke<ConnectedServer[]>("provider_servers");

export const providerActiveServer = () =>
  invoke<string | null>("provider_active_server");

export const providerSyncLibrary = () =>
  invoke<void>("provider_sync_library");

// ─── Album art (Phase 7) ────────────────────────────────────────

export interface AlbumArtResponse {
  bytes: number[];
  contentType: string;
  cached: boolean;
}

export const providerImageBytes = (albumId: string, tag?: string | null) =>
  invoke<AlbumArtResponse>("provider_image_bytes", { albumId, tag });

// ─── Last.fm (Phase 7) ─────────────────────────────────────────

export interface LastFmStatus {
  configured: boolean;
  authenticated: boolean;
  username: string | null;
}

export const lastfmConnect = (params: {
  apiKey: string;
  apiSecret: string;
  username: string;
  password: string;
}) =>
  invoke<LastFmStatus>("lastfm_connect", {
    apiKey: params.apiKey,
    apiSecret: params.apiSecret,
    username: params.username,
    password: params.password,
  });

export const lastfmDisconnect = () =>
  invoke<LastFmStatus>("lastfm_disconnect");

export const lastfmStatus = () => invoke<LastFmStatus>("lastfm_status");

// ─── Misc ───────────────────────────────────────────────────────

export const greet = (name: string) => invoke<string>("greet", { name });