// useKeyboardShortcuts — regression test for the ArrowUp volume bug.
//
// The original bug: pressing ArrowUp called the Tauri IPC wrapper
// `setVolume` twice in a row (once via `await`, once via the bare
// `setVolume` identifier that shadowed the imported wrapper). The
// store's `setVolume` action (aliased as `updateVolume` inside the
// hook) was never called, so the local volume state lagged behind
// the backend until the next `playback-state-changed` event landed.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useKeyboardShortcuts } from "./useKeyboardShortcuts";
import { usePlaybackStore } from "@/stores/playbackStore";
import { invokeMock } from "@/test/setup";

afterEach(() => {
  invokeMock.mockReset();
});

beforeEach(() => {
  // Reset the store to a known baseline so each test starts fresh.
  usePlaybackStore.setState({
    isPlaying: false,
    currentTrack: null,
    positionSeconds: 0,
    durationSeconds: 0,
    volume: 0.5,
    muted: false,
    repeat: "off",
    shuffle: false,
  });
  invokeMock.mockReset();
});

describe("useKeyboardShortcuts — ArrowUp volume", () => {
  it("calls IPC once and updates the local store on ArrowUp", async () => {
    renderHook(() => useKeyboardShortcuts());

    const before = usePlaybackStore.getState().volume;

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
      // Let the awaited promise resolve before assertions.
      await Promise.resolve();
      await Promise.resolve();
    });

    // IPC fired exactly once.
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("set_volume", { volume: before + 0.05 });

    // Local store advanced — this is the regression assertion. Before
    // the fix, `updateVolume` was shadowed by the IPC wrapper, so
    // the local store stayed at the pre-ArrowUp value.
    const after = usePlaybackStore.getState().volume;
    expect(after).toBeCloseTo(before + 0.05, 5);
  });

  it("clamps volume at 1.0 when repeatedly pressing ArrowUp", async () => {
    usePlaybackStore.setState({ volume: 0.98 });
    renderHook(() => useKeyboardShortcuts());

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(usePlaybackStore.getState().volume).toBeCloseTo(1.0, 5);
    expect(invokeMock).toHaveBeenCalledWith("set_volume", { volume: 1.0 });
  });
});
