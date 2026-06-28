// albumArtCache — regression test for the blob-URL leak fix
// (P0) and the bounded-LRU eviction (P2).

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  __resetForTests,
  buildBlobUrl,
  getCached,
  MAX_ENTRIES,
  revokeAll,
  setCached,
  size,
} from "./albumArtCache";

describe("albumArtCache.setCached", () => {
  beforeEach(() => {
    // Use the spy-free reset so the per-test spy starts clean.
    __resetForTests();
  });

  afterEach(() => {
    __resetForTests();
    vi.restoreAllMocks();
  });

  it("stores the blob URL keyed by (itemId, tag)", () => {
    const url = buildBlobUrl([1, 2, 3], "image/png");
    setCached("album-1", null, url);
    expect(getCached("album-1", null)).toBe(url);
    expect(size()).toBe(1);
  });

  it("revokes the prior blob URL when overwriting the same key", () => {
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => undefined);

    const first = buildBlobUrl([1], "image/png");
    const second = buildBlobUrl([2], "image/png");
    setCached("album-1", null, first);
    setCached("album-1", null, second);

    expect(revokeSpy).toHaveBeenCalledWith(first);
    expect(getCached("album-1", null)).toBe(second);
    expect(size()).toBe(1);
  });

  it("revokes the prior blob URL when overwriting a different tag", () => {
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => undefined);

    const primary = buildBlobUrl([1], "image/png");
    const backdrop = buildBlobUrl([2], "image/png");
    setCached("album-1", "primary", primary);
    setCached("album-1", "backdrop", backdrop);

    expect(revokeSpy).not.toHaveBeenCalled();
    expect(getCached("album-1", "primary")).toBe(primary);
    expect(getCached("album-1", "backdrop")).toBe(backdrop);
    expect(size()).toBe(2);
  });

  it("does not revoke when overwriting with the same URL identity", () => {
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => undefined);

    const url = buildBlobUrl([1], "image/png");
    setCached("album-1", null, url);
    setCached("album-1", null, url);

    expect(revokeSpy).not.toHaveBeenCalled();
    expect(size()).toBe(1);
  });

  it("revokes every cached URL on revokeAll", () => {
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => undefined);

    setCached("album-1", null, buildBlobUrl([1], "image/png"));
    setCached("album-2", null, buildBlobUrl([2], "image/png"));
    setCached("album-3", "backdrop", buildBlobUrl([3], "image/png"));

    revokeAll();

    expect(revokeSpy).toHaveBeenCalledTimes(3);
    expect(size()).toBe(0);
    expect(getCached("album-1", null)).toBeNull();
  });
});

describe("albumArtCache — bounded LRU", () => {
  beforeEach(() => {
    __resetForTests();
  });

  afterEach(() => {
    __resetForTests();
    vi.restoreAllMocks();
  });

  it("evicts the oldest entry when crossing MAX_ENTRIES", () => {
    // Set up the spy AFTER any setup, after `__resetForTests` cleared
    // the cache, so the only revokeObjectURL calls it sees come
    // from the eviction path under test.
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => undefined);

    const oldestUrl = buildBlobUrl([1], "image/png");
    setCached("oldest", null, oldestUrl);
    for (let i = 0; i < MAX_ENTRIES; i++) {
      setCached(`album-${i}`, null, buildBlobUrl([i + 100], "image/png"));
    }

    expect(size()).toBe(MAX_ENTRIES);
    expect(getCached("oldest", null)).toBeNull();
    expect(revokeSpy).toHaveBeenCalledWith(oldestUrl);
    expect(getCached(`album-${MAX_ENTRIES - 1}`, null)).not.toBeNull();
  });

  it("promotes a re-set key to most-recently-used in insertion order", () => {
    // Fill the cache to capacity with old entries.
    for (let i = 0; i < MAX_ENTRIES; i++) {
      setCached(`old-${i}`, null, buildBlobUrl([i], "image/png"));
    }
    expect(size()).toBe(MAX_ENTRIES);

    // Re-set "old-0" — this deletes the old entry and re-inserts
    // it at the tail of the Map. "old-1" is now the oldest.
    setCached("old-0", null, buildBlobUrl([99], "image/png"));

    // One more set evicts the oldest entry. "old-0" survives.
    setCached("new", null, buildBlobUrl([1], "image/png"));
    expect(getCached("old-0", null)).not.toBeNull();
    expect(getCached("old-1", null)).toBeNull();
  });

  it("does not evict when at or below MAX_ENTRIES", () => {
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => undefined);

    for (let i = 0; i < MAX_ENTRIES; i++) {
      setCached(`album-${i}`, null, buildBlobUrl([i], "image/png"));
    }
    expect(size()).toBe(MAX_ENTRIES);
    expect(revokeSpy).not.toHaveBeenCalled();
  });
});