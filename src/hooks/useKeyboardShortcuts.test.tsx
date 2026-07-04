// useKeyboardShortcuts — regression test for the ArrowUp volume bug.
//
// The original bug: pressing ArrowUp called the Tauri IPC wrapper
// `setVolume` twice in a row (once via `await`, once via the bare
// `setVolume` identifier that shadowed the imported wrapper). The
// store's `setVolume` action (aliased as `updateVolume` inside the
// hook) was never called, so the local volume state lagged behind
// the backend until the next `playback-state-changed` event landed.
//
// After the playback-context migration the assertion lives at the
// usePlayback() snapshot level. We wrap the hook in a real
// PlaybackProvider so the consumer actually sees the controls.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";

import { PlaybackProvider, usePlaybackContext } from "@/playback";
import { useKeyboardShortcuts } from "./useKeyboardShortcuts";
import { invokeMock } from "@/test/setup";

afterEach(() => {
  invokeMock.mockReset();
});

beforeEach(() => {
  // Bootstrap snapshot: 0.8 volume, 0.0 position, not playing. The
  // default mock resolves to undefined, but the playback hook treats
  // that as "no change" so the DEFAULT_SNAPSHOT (volume=0.8) sticks.
  invokeMock.mockReset();
});

function wrapWithProvider(): (props: { children: ReactNode }) => ReactNode {
  return ({ children }) => <PlaybackProvider>{children}</PlaybackProvider>;
}

describe("useKeyboardShortcuts — ArrowUp volume", () => {
  it("calls IPC once and advances the snapshot on ArrowUp", async () => {
    const Provider = wrapWithProvider();
    const { result } = renderHook(
      () => {
        useKeyboardShortcuts();
        return usePlaybackContext();
      },
      { wrapper: Provider },
    );

    // Wait one tick for the bootstrap effects (playback + queue
    // bridge) to settle.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    const volumeBefore = result.current.snapshot.volume;

    // Reset call history AFTER bootstrap so the assertion below only
    // counts what the keyboard shortcut did.
    invokeMock.mockClear();

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
      await Promise.resolve();
      await Promise.resolve();
    });

    // IPC fired exactly once with the expected value.
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("set_volume", {
      volume: volumeBefore + 0.05,
    });
  });

  it("clamps volume at 1.0 when repeatedly pressing ArrowUp", async () => {
    const Provider = wrapWithProvider();
    const { result } = renderHook(
      () => {
        useKeyboardShortcuts();
        return usePlaybackContext();
      },
      { wrapper: Provider },
    );

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    // Seed the snapshot to 0.98 via the public command.
    await act(async () => {
      await result.current.setVolume(0.98);
    });

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(invokeMock).toHaveBeenCalledWith("set_volume", { volume: 1.0 });
  });
});