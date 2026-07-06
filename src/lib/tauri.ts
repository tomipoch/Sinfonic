// Typed wrappers around `@tauri-apps/api/core`'s `invoke`.
//
// This is the **single place** the frontend calls into Rust. Two
// reasons for centralising:
//
// 1. **DRY**: stores / hooks / components never call `invoke` directly
//    — they import one of these typed wrappers. The wrapper owns the
//    return type and the wire-format keys.
// 2. **Documentation surface**: each wrapper is one line, but the
//    behaviour around it (when does it throw? what does the
//    `cached` flag on art responses mean?) lives in JSDoc next to
//    the import. New contributors grep `lib/tauri.ts`, not every
//    call site.
//
// The Rust command names (e.g. `"get_albums"`) are duplicated from
// `src-tauri/src/commands.rs`. `cargo check` and `pnpm tsc` both
// run on CI so the duplication is caught immediately.

import { invoke } from "@tauri-apps/api/core";

import type {
  Album,
  Artist,
  ArtistDetail,
  ConnectedServer,
  DiscoveredServer,
  EqBandPayload,
  Genre,
  PagedResponse,
  PlaybackStatePayload,
  PlayContext,
  Playlist,
  QueueSnapshot,
  SearchResults,
  Track,
} from "@/types/domain";

export type { ConnectedServer };

/**
 * Album with its full track list. Returned by `getAlbumDetail` and
 * `providerAlbumDetail`. `null` is returned for a missing id so
 * callers can render a not-found state without catching.
 */
interface AlbumDetail {
  album: Album;
  tracks: Track[];
}

// ─── Library ────────────────────────────────────────────────────

/** Fetch every genre (rarely paginated — usually < 200 entries). */
export const getGenres = () => invoke<Genre[]>("get_genres");

/** Paged albums carrying the given genre tag. */
export const getAlbumsByGenre = (genre: string, offset = 0, limit = 50) =>
  invoke<PagedResponse<Album>>("get_albums_by_genre", { genre, offset, limit });

/** Paged tracks under the given genre (via album tag). */
export const getTracksByGenre = (genre: string, offset = 0, limit = 200) =>
  invoke<PagedResponse<Track>>("get_tracks_by_genre", { genre, offset, limit });

/** Single album by id; `null` if missing. */
export const getAlbum = (albumId: string) => invoke<Album | null>("get_album", { albumId });

// ─── Provider-direct reads (Phase 1 of feature/direct-fetch-providers) ─
//
// Bypass the SQLite cache and hit the upstream server directly. Each
// call returns within one HTTP round-trip per page, so the UI can
// show data without waiting for `provider_sync_library` to finish.
//
// Behaviour matches the cached equivalents: empty pages / `null`
// when no provider is connected, so the "connect a server" hint
// still renders.

/** Paginated albums from the active provider. Empty page when offline. */
export const providerListAlbums = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Album>>("provider_list_albums", { offset, limit });

/** Paginated artists from the active provider. */
export const providerListArtists = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Artist>>("provider_list_artists", { offset, limit });

/** Paginated tracks from the active provider. */
export const providerListTracks = (offset = 0, limit = 50) =>
  invoke<PagedResponse<Track>>("provider_list_tracks", { offset, limit });

/** Album + tracks from the active provider. `null` when offline or unknown. */
export const providerAlbumDetail = (albumId: string) =>
  invoke<AlbumDetail | null>("provider_album_detail", { albumId });

/** Artist + their albums from the active provider. `null` when offline or unknown. */
export const providerArtistDetail = (artistId: string) =>
  invoke<ArtistDetail | null>("provider_artist_detail", { artistId });

// ─── Playback ───────────────────────────────────────────────────

/** Current transport state (playing, position, volume, etc.). */
export const getPlaybackState = () => invoke<PlaybackStatePayload>("get_playback_state");

/** Snapshot of the queue (entries + current index + repeat / shuffle). */
export const getQueue = () => invoke<QueueSnapshot>("get_queue");

/** Replace the queue with `[track]` and start playing it. Returns the new entry id. */
export const playTrack = (track: Track) => invoke<string>("play_track", { track });

/**
 * Like `playTrack` but also records a `PlayContext` so the
 * backend can auto-append the rest of the album / playlist /
 * favourites to the queue. Pass `null` for no context.
 */
export const playTrackWithContext = (track: Track, context: PlayContext | null) =>
  invoke<string>("play_track_with_context", { track, context });

/** Replace the queue with the supplied tracks. */
export const playAlbum = (tracks: Track[]) => invoke<void>("play_album", { tracks });

/** Like `playAlbum` but records a `PlayContext` for auto-fill. */
export const playAlbumWithContext = (tracks: Track[], context: PlayContext | null) =>
  invoke<void>("play_album_with_context", { tracks, context });

export const pause = () => invoke<void>("pause");
export const resume = () => invoke<void>("resume");
export const stop = () => invoke<void>("stop");
export const next = () => invoke<void>("next");
export const previous = () => invoke<void>("previous");

/** Seek to `positionSeconds` in the current track. */
export const seek = (positionSeconds: number) => invoke<void>("seek", { positionSeconds });

// ─── Queue mutations ───────────────────────────────────────────

/** Remove one entry by id. `false` if the id was already gone. */
export const queueRemove = (entryId: string) => invoke<boolean>("queue_remove", { entryId });

/** Skip playback to the given entry. `false` if the id was already gone. */
export const queueJumpTo = (entryId: string) => invoke<boolean>("queue_jump_to", { entryId });

/** Drop every entry from the queue. */
export const queueClear = () => invoke<void>("queue_clear");

// ─── Queue bulk + Playlist CRUD ────────────────────────────────

/** Append every supplied track to the end of the queue. Returns the new entry ids. */
export const queueAddMany = (tracks: Track[]) => invoke<string[]>("queue_add_many", { tracks });

/** Insert every supplied track just after the currently-playing entry. */
export const queuePlayNextMany = (tracks: Track[]) =>
  invoke<string[]>("queue_play_next_many", { tracks });

/**
 * Append up to `n` more tracks from the active `PlayContext` (set
 * by a previous `playTrackWithContext` / `playAlbumWithContext`
 * call) to the end of the queue. Returns the number actually
 * added (0 if no context or the context is exhausted).
 */
export const queueExtendMore = (n: number) => invoke<number>("queue_extend_more", { n });

/** Every saved user playlist (metadata only). */
export const playlistsGet = () => invoke<Playlist[]>("playlists_get");

/** Playlist with its tracks. */
export interface PlaylistDetail {
  playlist: Playlist;
  tracks: Track[];
}

export const playlistDetail = (playlistId: string) =>
  invoke<PlaylistDetail>("playlist_detail", { playlistId });

/** Create a new playlist with the given name and tracks. Returns the new playlist id. */
export const createPlaylist = (name: string, trackIds: string[]) =>
  invoke<string>("create_playlist", { name, trackIds });

export const renamePlaylist = (playlistId: string, name: string) =>
  invoke<void>("rename_playlist", { playlistId, name });

export const deletePlaylist = (playlistId: string) =>
  invoke<void>("delete_playlist", { playlistId });

export const removePlaylistEntries = (playlistId: string, entryIds: string[]) =>
  invoke<void>("remove_playlist_entries", { playlistId, entryIds });

export const addPlaylistTracks = (playlistId: string, trackIds: string[]) =>
  invoke<void>("add_playlist_tracks", { playlistId, trackIds });

// ─── Favorites ─────────────────────────────────────────────────

export interface FavoritesPayload {
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
}

export const setTrackFavorite = (trackId: string, favorite: boolean) =>
  invoke<void>("set_track_favorite", { trackId, favorite });

export const setAlbumFavorite = (albumId: string, favorite: boolean) =>
  invoke<void>("set_album_favorite", { albumId, favorite });

export const setArtistFavorite = (artistId: string, favorite: boolean) =>
  invoke<void>("set_artist_favorite", { artistId, favorite });

/** Get every favorited track / album / artist in one call. */
export const getFavorites = () => invoke<FavoritesPayload>("get_favorites");

// ─── Smart Playlists ──────────────────────────────────────────

/** Field a smart-playlist rule can match against. */
export type SmartPlaylistRuleField =
  | "title"
  | "artist"
  | "album"
  | "genre"
  | "duration_seconds"
  | "track_number"
  | "year"
  | "favorite"
  | "play_count";

/** Comparison operator. Numeric operators auto-coerce both sides. */
export type SmartPlaylistRuleOperator =
  | "contains"
  | "starts_with"
  | "ends_with"
  | "equals"
  | "less_than"
  | "greater_than"
  | "not_contains"
  | "not_equals";

/** Field the playlist results are sorted by. */
export type SmartPlaylistSortField =
  | "title"
  | "artist"
  | "album"
  | "duration_seconds"
  | "year"
  | "random"
  | "date_added";

export type SmartPlaylistSortDirection = "asc" | "desc";

export interface SmartPlaylistRule {
  field: SmartPlaylistRuleField;
  operator: SmartPlaylistRuleOperator;
  value: string;
}

export interface SmartPlaylist {
  id: string;
  name: string;
  rule: SmartPlaylistRule;
  sortField: SmartPlaylistSortField;
  sortDir: SmartPlaylistSortDirection;
  limitN: number;
}

export interface CreateSmartPlaylistArgs {
  name: string;
  field: SmartPlaylistRuleField;
  operator: SmartPlaylistRuleOperator;
  value: string;
  sortField: SmartPlaylistSortField;
  sortDir: SmartPlaylistSortDirection;
  limitN: number;
}

export const getSmartPlaylists = () => invoke<SmartPlaylist[]>("get_smart_playlists");

export const createSmartPlaylist = (args: CreateSmartPlaylistArgs) =>
  invoke<SmartPlaylist>("create_smart_playlist", { args });

export const deleteSmartPlaylist = (spId: string) =>
  invoke<void>("delete_smart_playlist", { spId });

/** Evaluate the rule against the current library cache. */
export const evaluateSmartPlaylist = (spId: string) =>
  invoke<Track[]>("evaluate_smart_playlist", { spId });

// ─── Repeat / shuffle ───────────────────────────────────────────

/**
 * Set the repeat mode.
 *
 * @param repeat `"off"` | `"all"` | `"one"`. `lib/repeat` owns the
 *   cycle order (`off → all → one → off`).
 */
export const setRepeat = (repeat: "off" | "one" | "all") => invoke<void>("set_repeat", { repeat });

export const setShuffle = (enabled: boolean) => invoke<void>("set_shuffle", { enabled });

// ─── Volume / EQ ───────────────────────────────────────────────

/** Set the output volume. Backend clamps to [0, 1]. */
export const setVolume = (volume: number) => invoke<void>("set_volume", { volume });

export const setMuted = (muted: boolean) => invoke<void>("set_muted", { muted });

export type { EqBandPayload } from "@/types/domain";

/** Fetch the current 10-band EQ state. */
export const getEqBands = () => invoke<EqBandPayload[]>("get_eq_bands");

/** Set one band's gain. `hz` is the band's centre frequency in Hz; `gainDb` is in dB. */
export const setEqBand = (hz: number, gainDb: number) =>
  invoke<void>("set_eq_band", { band: { hz, gainDb } });

export const resetEq = () => invoke<void>("reset_eq");

// ─── Crossfade (Phase 3) ────────────────────────────────────────

/**
 * Configure crossfade. `seconds` is clamped to [0, 12] on the
 * Rust side so a hostile or buggy caller can't schedule
 * hour-long fades. The configuration is persisted via
 * `library.set_preference` so the next launch restores it
 * before the first track plays.
 */
export const setCrossfade = (enabled: boolean, seconds: number) =>
  invoke<void>("set_crossfade", { enabled, seconds });

/** Snapshot the current crossfade configuration. */
export const getCrossfadeConfig = () =>
  invoke<{ crossfadeEnabled: boolean; crossfadeSeconds: number }>("get_crossfade_config");

// ─── Search ─────────────────────────────────────────────────────

/** Full-text search across the cached library. `limit` defaults to 20 per category. */
export const search = (query: string, limit = 20) =>
  invoke<SearchResults>("search", { query, limit });

// ─── Provider (Jellyfin + Subsonic) ─────────────────────────────

/** Jellyfin SSDP / mDNS discovery — returns the servers on the LAN. */
export const jellyfinDiscover = () => invoke<DiscoveredServer[]>("jellyfin_discover");

export const jellyfinLogin = (params: { baseUrl: string; username: string; password: string }) =>
  invoke<ConnectedServer>("jellyfin_login", { request: params });

export const subsonicLogin = (params: { baseUrl: string; username: string; password: string }) =>
  invoke<ConnectedServer>("subsonic_login", { request: params });

export const providerLogout = () => invoke<void>("provider_logout");

export const providerDelete = (serverId: string) => invoke<void>("provider_delete", { serverId });

export const providerServers = () => invoke<ConnectedServer[]>("provider_servers");

export const providerActiveServer = () => invoke<string | null>("provider_active_server");

export interface BootstrapState {
  ready: boolean;
  activeServerId: string | null;
  savedServers: ConnectedServer[];
}

/** One-shot snapshot of every server-side field the boot flow needs. */
export const bootstrapState = () => invoke<BootstrapState>("bootstrap_state");

export const providerSetActive = (serverId: string) =>
  invoke<ConnectedServer>("provider_set_active", { serverId });

/** Kick off a full library rescan. Listen for `library-sync-status` events for progress. */
export const providerSyncLibrary = () => invoke<void>("provider_sync_library");

// ─── Local files ─────────────────────────────────────────────

/** Result of a local folder scan. Counts are the totals the backend found. */
export interface LocalScanResult {
  serverId: string;
  serverName: string;
  root: string;
  tracks: number;
  albums: number;
  artists: number;
  errors: number;
}

export const localLogin = (path: string) => invoke<LocalScanResult>("local_login", { path });

/** Re-scan the active local folder. */
export const localRescan = () => invoke<LocalScanResult>("local_rescan");

// ─── Album art ─────────────────────────────────────────────────

/** Raw image bytes plus the Rust-side `cached` flag for diagnostics. */
export interface AlbumArtResponse {
  bytes: number[];
  contentType: string;
  cached: boolean;
}

export const providerImageBytes = (albumId: string, tag?: string | null) =>
  invoke<AlbumArtResponse>("provider_image_bytes", { albumId, tag });

export interface AlbumArtRequest {
  albumId: string;
  tag?: string | null;
}

export interface AlbumArtBulkItem {
  albumId: string;
  tag: string | null;
  bytes: number[];
  contentType: string;
  cached: boolean;
}

export interface AlbumArtBulkResponse {
  images: AlbumArtBulkItem[];
  notFound: string[];
}

/** Batch image fetch — prefer over many single `providerImageBytes` calls. */
export const providerImageBytesBulk = (requests: AlbumArtRequest[]) =>
  invoke<AlbumArtBulkResponse>("provider_image_bytes_bulk", { requests });

// ─── Lyrics ──────────────────────────────────────────────────────

export interface LyricsPayload {
  /** Plain-text lyrics (most providers). */
  plain: string | null;
  /**
   * LRC-flavoured synced lyrics, one line per row, lines joined by
   * `\n`. Empty when the provider only returns plain text or nothing.
   */
  synced: string | null;
  /** Provider identifier that produced the lyrics (e.g. `subsonic`). */
  source: string | null;
}

/**
 * Fetch lyrics for the given track through the active provider.
 * Returns `null` when the provider is disconnected or has no lyrics
 * for this track — the lyrics panel renders a "no lyrics" placeholder
 * in that case instead of an error toast.
 */
export const getLyrics = (trackId: string, allowRemote = true) =>
  invoke<LyricsPayload | null>("get_lyrics", { trackId, allowRemote });

// ─── Last.fm ──────────────────────────────────────────────────

export interface LastFmStatus {
  configured: boolean;
  authenticated: boolean;
  username: string | null;
}

/** Connect a Last.fm account. The password is md5-hashed server-side before use. */
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

export const lastfmDisconnect = () => invoke<LastFmStatus>("lastfm_disconnect");

export const lastfmStatus = () => invoke<LastFmStatus>("lastfm_status");
