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

export interface QueueSnapshotPayload {
  entries: QueueEntry[];
  currentIndex: number | null;
  repeat: RepeatMode;
  shuffle: boolean;
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

export interface EqBand {
  hz: number;
  gainDb: number;
}

export interface DiscoveredServer {
  name: string;
  baseUrl: string;
  serverId: string;
}

export interface ConnectedServer {
  serverId: string;
  kind: string;
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

export type LibrarySyncState = "started" | "complete" | "failed";

export interface LibrarySyncStatus {
  serverId: string | null;
  state: LibrarySyncState | string;
  progress: number;
}
