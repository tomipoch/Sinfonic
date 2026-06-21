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

// ─── Library ────────────────────────────────────────────────────

export const getAlbums = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Album>>("get_albums", { offset, limit });

export const getArtists = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Artist>>("get_artists", { offset, limit });

export const getTracks = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Track>>("get_tracks", { offset, limit });

// ─── Playback ───────────────────────────────────────────────────

export const getPlaybackState = () =>
  invoke<PlaybackStatePayload>("get_playback_state");

export const getQueue = () => invoke<QueueSnapshot>("get_queue");

export const playTrack = (track: Track) =>
  invoke<string>("play_track", { track });

export const pause = () => invoke<void>("pause");
export const resume = () => invoke<void>("resume");
export const stop = () => invoke<void>("stop");
export const next = () => invoke<void>("next");
export const previous = () => invoke<void>("previous");

export const seek = (positionSeconds: number) =>
  invoke<void>("seek", { positionSeconds });

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

// ─── Jellyfin provider ──────────────────────────────────────────

export const jellyfinDiscover = () =>
  invoke<DiscoveredServer[]>("jellyfin_discover");

export const jellyfinLogin = (params: {
  baseUrl: string;
  username: string;
  password: string;
}) =>
  invoke<ConnectedServer>("jellyfin_login", { request: params });

export const jellyfinLogout = () => invoke<void>("jellyfin_logout");

export const jellyfinServers = () =>
  invoke<ConnectedServer[]>("jellyfin_servers");

export const jellyfinActiveServer = () =>
  invoke<string | null>("jellyfin_active_server");

export const jellyfinSyncLibrary = () =>
  invoke<void>("jellyfin_sync_library");

// ─── Misc ───────────────────────────────────────────────────────

export const greet = (name: string) => invoke<string>("greet", { name });