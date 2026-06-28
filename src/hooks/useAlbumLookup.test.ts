// useAlbumLookup — regression tests for the P0 race + ref-stability fixes.
//
// The original bug: `ensureLoaded` depended on `extras` state so its
// identity changed on every successful lookup, which in turn
// re-triggered the prewarm hook's effect (and any other consumer
// that depended on it) on every successful fetch. P0 fixes that by
// reading `extras` from a ref so the callback is stable.
//
// The race-fix: a server switch between firing `getAlbum` and its
// resolution must not let the previous server's album land in the
// current server's lookup map. `ensureLoaded` captures the active
// server at fire time and bails at resolve time if it changed.

import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAlbumLookup } from "./useAlbumLookup";
import { useLibraryStore } from "@/stores/libraryStore";
import { useServerStore } from "@/stores/serverStore";
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
    activeServerId: "server-A",
    servers: [],
    discovered: [],
    lastSync: "idle",
    error: null,
    pendingConnection: null,
  });
  invokeMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useAlbumLookup — ref stability", () => {
  it("ensureLoaded keeps the same identity across successful lookups", async () => {
    const { result, rerender } = renderHook(() => useAlbumLookup());

    const initial = result.current.ensureLoaded;
    invokeMock.mockResolvedValueOnce(makeAlbum("a-0"));
    await act(async () => {
      result.current.ensureLoaded("a-0");
      await Promise.resolve();
      await Promise.resolve();
    });

    // Re-render to pick up the new extras map.
    rerender();
    expect(result.current.ensureLoaded).toBe(initial);

    invokeMock.mockResolvedValueOnce(makeAlbum("a-1"));
    await act(async () => {
      result.current.ensureLoaded("a-1");
      await Promise.resolve();
      await Promise.resolve();
    });

    rerender();
    // Identity must NOT change across lookups — that's the whole
    // reason ref stability exists (downstream effects that depend
    // on `ensureLoaded` re-fire otherwise).
    expect(result.current.ensureLoaded).toBe(initial);
  });
});

describe("useAlbumLookup — cache hits", () => {
  it("ensureLoaded is a no-op when the album is already in the store", async () => {
    useLibraryStore.setState({ albums: [makeAlbum("a-store")] });
    const { result } = renderHook(() => useAlbumLookup());

    act(() => {
      result.current.ensureLoaded("a-store");
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("ensureLoaded is a no-op for an empty id", () => {
    const { result } = renderHook(() => useAlbumLookup());

    act(() => {
      result.current.ensureLoaded("");
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("ensureLoaded dedupes concurrent calls for the same albumId", async () => {
    let resolveFetch: (v: unknown) => void = () => {};
    const pending = new Promise((r) => {
      resolveFetch = r;
    });
    invokeMock.mockReturnValueOnce(pending);

    const { result } = renderHook(() => useAlbumLookup());

    act(() => {
      result.current.ensureLoaded("a-dup");
      result.current.ensureLoaded("a-dup");
      result.current.ensureLoaded("a-dup");
    });

    expect(invokeMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFetch(makeAlbum("a-dup"));
      await pending;
      await Promise.resolve();
    });
  });
});

describe("useAlbumLookup — server-switch race", () => {
  it("drops the response if the active server changes mid-flight", async () => {
    let resolveFetch: (v: unknown) => void = () => {};
    const pending = new Promise((r) => {
      resolveFetch = r;
    });
    invokeMock.mockReturnValueOnce(pending);

    const { result } = renderHook(() => useAlbumLookup());

    act(() => {
      result.current.ensureLoaded("a-cross-server");
    });

    // Simulate the user switching servers while the IPC is in flight.
    act(() => {
      useServerStore.setState({ activeServerId: "server-B" });
    });

    // Now resolve with server-A's album. The fix should drop it.
    await act(async () => {
      resolveFetch(makeAlbum("a-cross-server"));
      await pending;
      await Promise.resolve();
    });

    // albumById should be empty — the stale album was discarded.
    expect(result.current.albumById.size).toBe(0);
  });

  it("still accepts the response when the server hasn't changed", async () => {
    invokeMock.mockResolvedValueOnce(makeAlbum("a-same-server"));

    const { result } = renderHook(() => useAlbumLookup());

    await act(async () => {
      result.current.ensureLoaded("a-same-server");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.albumById.get("a-same-server")).toBeDefined();
  });

  it("discards null responses without throwing", async () => {
    invokeMock.mockResolvedValueOnce(null);

    const { result } = renderHook(() => useAlbumLookup());

    await act(async () => {
      result.current.ensureLoaded("a-null");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.albumById.size).toBe(0);
  });
});