// libraryStore — regression tests for the P1 real pagination path.
//
// Pre-P1 the store had a single `loadX` per list that fetched page
// 1 of 200 and never fetched more. Libraries with more than 200
// albums / artists / tracks silently lost the overflow because
// every view consumed `state.albums` / `state.artists` /
// `state.tracks` directly. P1 adds `loadMoreX` actions with
// dedup-by-id and a `loadingMoreX` re-entry guard so concurrent
// intersections don't double-fire.

import { beforeEach, describe, expect, it, vi } from "vitest";

import { PAGE_SIZE, useLibraryStore } from "./libraryStore";
import { useServerStore } from "./serverStore";
import { invokeMock } from "@/test/setup";

const makeAlbum = (id: string) => ({
  id,
  title: `Album ${id}`,
  artist: `Artist ${id}`,
  artistId: null,
  year: null,
  trackCount: 1,
  durationSeconds: 100,
  favorite: false,
  imageRef: null,
  genres: [],
});

const makeArtist = (id: string) => ({
  id,
  name: `Artist ${id}`,
  albumCount: 1,
  trackCount: 1,
  favorite: false,
  imageRef: null,
});

const makeTrack = (id: string) => ({
  id,
  albumId: `album-${id}`,
  title: `Track ${id}`,
  artist: `Artist ${id}`,
  artistId: null,
  album: `Album ${id}`,
  durationSeconds: 100,
  trackNumber: 1,
  discNumber: 1,
  favorite: false,
  imageRef: null,
});

beforeEach(() => {
  useLibraryStore.setState({
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
  useServerStore.setState({
    activeServerId: "server-test",
    servers: [],
    discovered: [],
    lastSync: "idle",
    error: null,
    pendingConnection: null,
  });
  invokeMock.mockReset();
});

describe("libraryStore — pagination", () => {
  it("loadAlbums stores the first page and the total count", async () => {
    const items = Array.from({ length: 50 }, (_, i) => makeAlbum(`a-${i}`));
    invokeMock.mockResolvedValueOnce({ items, total: 250 });

    await useLibraryStore.getState().loadAlbums();

    const state = useLibraryStore.getState();
    expect(state.albums).toHaveLength(50);
    expect(state.albumsTotal).toBe(250);
    expect(state.loaded).toBe(true);
    expect(state.loading).toBe(false);
  });

  it("loadMoreAlbums appends the next page and dedupes by id", async () => {
    const page1 = Array.from({ length: PAGE_SIZE }, (_, i) => makeAlbum(`a-${i}`));
    const page2 = Array.from({ length: PAGE_SIZE }, (_, i) => makeAlbum(`a-${PAGE_SIZE + i}`));
    // Simulate a stale read by repeating the last entry of page 1
    // at the head of page 2 — the dedupe should drop it.
    const page2WithDup = [page1[page1.length - 1]!, ...page2];

    invokeMock
      .mockResolvedValueOnce({ items: page1, total: page1.length + page2WithDup.length - 1 })
      .mockResolvedValueOnce({ items: page2WithDup, total: page1.length + page2WithDup.length - 1 });

    await useLibraryStore.getState().loadAlbums();
    await useLibraryStore.getState().loadMoreAlbums();

    const state = useLibraryStore.getState();
    expect(state.albums).toHaveLength(PAGE_SIZE + page2.length);
    expect(state.loadingMoreAlbums).toBe(false);
    // The duplicate from page 1 was dropped from page 2.
    const ids = state.albums.map((a) => a.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("loadMoreAlbums is a no-op once the list is fully loaded", async () => {
    const items = [makeAlbum("only")];
    invokeMock
      .mockResolvedValueOnce({ items, total: 1 })
      .mockResolvedValueOnce({ items: [], total: 1 }); // would be called if not guarded

    await useLibraryStore.getState().loadAlbums();
    await useLibraryStore.getState().loadMoreAlbums();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(useLibraryStore.getState().albums).toHaveLength(1);
  });

  it("concurrent loadMoreAlbums calls are guarded by loadingMoreAlbums", async () => {
    const page1 = [makeAlbum("a-0")];
    const page2 = [makeAlbum("a-1")];

    let resolveFirst: (v: unknown) => void = () => {};
    const slow = new Promise((r) => {
      resolveFirst = r;
    });

    invokeMock
      .mockReturnValueOnce(slow) // first loadMore call
      .mockResolvedValueOnce({ items: page2, total: 2 }); // would be wrong if it ran

    // First call starts and parks.
    const first = useLibraryStore.getState().loadMoreAlbums();
    // Second call must short-circuit while the first is in flight.
    await useLibraryStore.getState().loadMoreAlbums();

    resolveFirst({ items: page1, total: 2 });
    await first;

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("loadMoreArtists and loadMoreTracks follow the same pattern", async () => {
    const artists = [makeArtist("ar-0"), makeArtist("ar-1")];
    const tracks = [makeTrack("t-0")];

    invokeMock
      .mockResolvedValueOnce({ items: artists, total: 5 }) // 2 returned, 5 total → page 2 exists
      .mockResolvedValueOnce({ items: [], total: 5 }) // loadMoreArtists
      .mockResolvedValueOnce({ items: tracks, total: 1 }) // loadTracks
      // loadMoreTracks short-circuits because total === loaded length

      .mockResolvedValueOnce({ items: [], total: 1 }); // not called, but harmless

    await useLibraryStore.getState().loadArtists();
    await useLibraryStore.getState().loadMoreArtists();
    await useLibraryStore.getState().loadTracks();
    await useLibraryStore.getState().loadMoreTracks();

    expect(useLibraryStore.getState().artists).toEqual(artists);
    expect(useLibraryStore.getState().tracks).toEqual(tracks);
    // 3 actual fetches (loadArtists, loadMoreArtists, loadTracks) +
    // loadMoreTracks short-circuits on `tracks.length >= tracksTotal`.
    expect(invokeMock).toHaveBeenCalledTimes(3);
  });

  it("loadMoreAlbums surfaces errors via extractError fallback", async () => {
    invokeMock
      .mockResolvedValueOnce({ items: [], total: 0 })
      .mockRejectedValueOnce(new Error("network down"));

    await useLibraryStore.getState().loadAlbums();
    await useLibraryStore.getState().loadMoreAlbums();

    expect(useLibraryStore.getState().loadingMoreAlbums).toBe(false);
    expect(useLibraryStore.getState().error).toBe("network down");
  });

  it("reset clears pagination state", () => {
    useLibraryStore.setState({
      albums: [makeAlbum("a-0")],
      albumsTotal: 500,
      loadingMoreAlbums: true,
      artists: [makeArtist("ar-0")],
      artistsTotal: 300,
      tracks: [makeTrack("t-0")],
      tracksTotal: 1000,
      loadingMoreTracks: true,
      loading: true,
      loaded: true,
      error: "boom",
    });

    useLibraryStore.getState().reset();

    const state = useLibraryStore.getState();
    expect(state.albums).toHaveLength(0);
    expect(state.albumsTotal).toBe(0);
    expect(state.artistsTotal).toBe(0);
    expect(state.tracksTotal).toBe(0);
    expect(state.loadingMoreAlbums).toBe(false);
    expect(state.loadingMoreTracks).toBe(false);
    expect(state.loaded).toBe(false);
    expect(state.error).toBeNull();
  });
});

describe("libraryStore — error handling", () => {
  it("loadAll reports the failure via the error field", async () => {
    invokeMock.mockRejectedValueOnce("ipc: connection refused");

    await useLibraryStore.getState().loadAll();

    const state = useLibraryStore.getState();
    expect(state.error).toBe("ipc: connection refused");
    expect(state.loaded).toBe(false);
    expect(state.loading).toBe(false);
  });

  it("loadGenres error fallback is friendly", async () => {
    invokeMock.mockRejectedValueOnce(new Error(""));

    await useLibraryStore.getState().loadGenres();

    expect(useLibraryStore.getState().error).toBe("couldn't load genres");
  });
});

// Defensive: ensure `vi` import is not stripped by Biome.
void vi;