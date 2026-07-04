// Domain types — mirrors of the Rust types in `src-tauri/crates/domain/`.
//
// We mirror manually (no `ts-rs` / `specta` build step yet) because the
// surface is small. Once a feature phase stabilises its IPC contract,
// codegen can be added without changing the call sites.

export type RepeatMode = "off" | "one" | "all";

export type ImageKind = "primary" | "backdrop";

export type EqBandPayload = {
  hz: number;
  gainDb: number;
};

export interface ImageRef {
  itemId: string;
  kind: string;
  tag?: string | null;
}

export interface Album {
  id: string;
  title: string;
  artist: string;
  artistId?: string | null;
  year?: number | null;
  trackCount: number;
  durationSeconds: number;
  favorite: boolean;
  imageRef?: ImageRef | null;
  genres: string[];
}

export interface Track {
  id: string;
  albumId: string;
  title: string;
  artist: string;
  artistId?: string | null;
  album: string;
  durationSeconds: number;
  trackNumber: number;
  discNumber: number;
  favorite: boolean;
  imageRef?: ImageRef | null;
}

export interface Artist {
  id: string;
  name: string;
  albumCount: number;
  trackCount: number;
  favorite: boolean;
  imageRef?: ImageRef | null;
}

/**
 * Artist + the albums they appear on. Mirrors the Rust
 * `sinfonic_domain::ArtistDetail`. Returned by `providerArtistDetail`
 * so the artist view can render without a follow-up album fetch.
 */
export interface ArtistDetail {
  artist: Artist;
  albums: Album[];
}

export interface Genre {
  id: string;
  name: string;
  albumCount: number;
  trackCount: number;
}

export interface Playlist {
  id: string;
  name: string;
  trackCount: number;
  durationSeconds: number;
  owner?: string | null;
  public: boolean;
  imageRef?: ImageRef | null;
}

export interface PagedResponse<T> {
  items: T[];
  total: number;
}

export interface SearchResults {
  albums: Album[];
  tracks: Track[];
  artists: Artist[];
  playlists: Playlist[];
}

export interface QueueEntry {
  id: string;
  trackId: string;
  title: string;
  artist: string;
  album: string;
  durationSeconds: number;
}

export interface QueueSnapshot {
  serverId: string | null;
  entries: QueueEntry[];
  currentIndex: number | null;
  repeat: RepeatMode;
  shuffle: boolean;
  shuffleSeed: number;
}

/**
 * Closed union mirroring the Rust `PlayContext` enum. Used by the
 * playback commands to tell the backend "the user clicked this
 * track from album X / playlist Y / favourites / a flat track
 * list" so it can auto-append the rest of the context to the
 * upcoming queue.
 *
 * - `album` / `playlist` / `favorites` resolve to a sibling-style
 *   collection (album tracks in track-number order, playlist
 *   entries in position order, favourites by title).
 * - `all` resolves to the entire library sorted by title — used
 *   by SongsView where there's no obvious album / playlist
 *   grouping.
 */
export type PlayContext =
  | { kind: "album"; albumId: string }
  | { kind: "playlist"; playlistId: string; serverId: string }
  | { kind: "favorites"; serverId: string }
  | { kind: "all"; serverId: string };

export interface QueueSnapshotPayload {
  entries: QueueEntry[];
  currentIndex: number | null;
  repeat: RepeatMode;
  shuffle: boolean;
  /**
   * Number of additional tracks available from the active play
   * context (album / playlist / favourites) that aren't yet in
   * `entries`. The QueuePanel uses this for the "+N más"
   * affordance. `undefined` when no context is set.
   */
  contextRemaining?: number | null;
}

export interface PlaybackStatePayload {
  isPlaying: boolean;
  positionSeconds: number;
  durationSeconds: number;
  volume: number;
  muted: boolean;
  repeat: RepeatMode;
  shuffle: boolean;
}

export interface TrackChangedPayload {
  trackId: string;
  title: string;
  artist: string;
  album: string;
}

/**
 * Payload for `playback-config-changed`. Mirrors
 * `sinfonic_domain::events::PlaybackConfigPayload`. The settings UI
 * listens so the slider stays in sync after the user touches it
 * from elsewhere (or after the restore-on-launch path applies the
 * persisted value).
 */
export interface PlaybackConfigPayload {
  crossfadeEnabled: boolean;
  crossfadeSeconds: number;
}

export interface EqBand {
  hz: number;
  gainDb: number;
}

export interface DiscoveredServer {
  name: string;
  baseUrl: string;
  serverId: string;
}

/**
 * Closed union mirroring the Rust `ProviderKind` enum used by every
 * saved / active server. The Rust side emits one of these three
 * strings as `kind`; we narrow it here so the TS side rejects an
 * unexpected value at compile time instead of silently passing
 * "plex" / "" / whatever through the `KIND_ICONS` map and falling
 * back to a placeholder.
 */
export type ServerKind = "jellyfin" | "subsonic" | "local";

export interface ConnectedServer {
  serverId: string;
  kind: ServerKind;
  name: string;
  baseUrl: string;
}

export interface JellyfinLoginRequest {
  baseUrl: string;
  username: string;
  password: string;
}

export interface SubsonicLoginRequest {
  baseUrl: string;
  username: string;
  password: string;
}

export interface LocalLoginRequest {
  path: string;
}

/**
 * Closed union mirroring the `state` field of the
 * `library-sync-status` event emitted by the Rust backend during
 * login and explicit `provider_sync_library` calls. Frontend
 * consumers branch on this exhaustive set instead of falling
 * back to a `| string` escape hatch (which silently defeats the
 * point of a discriminated union).
 */
export type SyncState =
  | "preparing"
  | "started"
  | "scanning"
  | "indexing"
  | "caching"
  | "syncing"
  | "complete";

export interface LibrarySyncStatus {
  serverId: string | null;
  state: SyncState;
  progress: number;
}
