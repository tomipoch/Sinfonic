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

export const playTrack = (trackId: string) =>
  invoke<void>("play_track", { trackId });

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

// ─── Search ─────────────────────────────────────────────────────

export const search = (query: string, limit = 20) =>
  invoke<SearchResults>("search", { query, limit });

// ─── Jellyfin provider ──────────────────────────────────────────

export const jellyfinDiscover = () =>
  invoke<DiscoveredServer[]>("jellyfin_discover");

export const jellyfinLogin = (
  baseUrl: string,
  username: string,
  password: string,
) =>
  invoke<string>("jellyfin_login", { baseUrl, username, password });

// ─── Misc ───────────────────────────────────────────────────────

export const greet = (name: string) => invoke<string>("greet", { name });
