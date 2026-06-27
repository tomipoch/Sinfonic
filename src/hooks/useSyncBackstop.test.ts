// useSyncBackstop — regression test for the timer-reset semantics.
//
// Original bug: `LoadingView`'s 30 s backstop was a mount-only
// effect, so it fired unconditionally 30 s after the view mounted
// regardless of sync state. Large local library scans (>30 s)
// got force-navigated away while still in `state: "scanning"`,
// which on Subsonic/Jellyfin surfaced as `activeServerId cleared
// → reset()` (login hadn't finished) and a bounce back to /setup.
//
// The fix moves the timer into a hook that depends on every sync
// value. Each progress event clears + re-arms the timer, so a
// legitimately progressing sync never trips the backstop. Only a
// silent sync for the full timeout triggers the navigation.

import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  type BackstopSyncState,
  useSyncBackstop,
} from "./useSyncBackstop";

// Short timeout for tests so we don't have to spin fake timers
// for minutes. The hook is timeout-agnostic — the value is just a
// knob — so 1_000 ms is enough to verify all the semantics.
const TIMEOUT = 1_000;

const IDLE: BackstopSyncState = {
  state: "preparing",
  progress: 0,
  done: false,
  active: false,
  error: null,
};

const ACTIVE: BackstopSyncState = {
  state: "scanning",
  progress: 0.2,
  done: false,
  active: true,
  error: null,
};

const DONE: BackstopSyncState = {
  state: "complete",
  progress: 1,
  done: true,
  active: false,
  error: null,
};

const ERROR: BackstopSyncState = {
  state: "started",
  progress: 0,
  done: false,
  active: true,
  error: "boom",
};

describe("useSyncBackstop", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not fire while the sync keeps emitting progress events", () => {
    const onStale = vi.fn();
    const { rerender } = renderHook(
      ({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale),
      { initialProps: { sync: ACTIVE } },
    );

    // Re-render with a fresh `state`/`progress` value every
    // (TIMEOUT / 2) — simulating a live scan. Each change clears
    // + re-arms the timer, so the backstop must NEVER fire even
    // after many timeout's worth of fake time has elapsed.
    const halfTimeout = TIMEOUT / 2;
    for (let tick = 0; tick < 10; tick += 1) {
      act(() => {
        vi.advanceTimersByTime(halfTimeout);
      });
      rerender({
        sync: { ...ACTIVE, progress: Math.min(1, (tick + 1) * 0.08) },
      });
    }
    expect(onStale).not.toHaveBeenCalled();
  });

  it("fires after the full timeout when the sync state stops changing", () => {
    const onStale = vi.fn();
    renderHook(({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale), {
      initialProps: { sync: ACTIVE },
    });

    // Just before the threshold.
    act(() => {
      vi.advanceTimersByTime(TIMEOUT - 1);
    });
    expect(onStale).not.toHaveBeenCalled();

    // Cross the threshold.
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onStale).toHaveBeenCalledTimes(1);
  });

  it("does not fire when sync is already done", () => {
    const onStale = vi.fn();
    renderHook(({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale), {
      initialProps: { sync: DONE },
    });

    act(() => {
      vi.advanceTimersByTime(TIMEOUT * 3);
    });
    expect(onStale).not.toHaveBeenCalled();
  });

  it("does not fire when sync has errored", () => {
    const onStale = vi.fn();
    renderHook(({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale), {
      initialProps: { sync: ERROR },
    });

    act(() => {
      vi.advanceTimersByTime(TIMEOUT * 3);
    });
    expect(onStale).not.toHaveBeenCalled();
  });

  it("cancels a pending timer when sync flips to done mid-flight", () => {
    const onStale = vi.fn();
    const { rerender } = renderHook(
      ({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale),
      { initialProps: { sync: ACTIVE } },
    );

    // Halfway to the timeout, the backend reports `complete`.
    act(() => {
      vi.advanceTimersByTime(TIMEOUT / 2);
    });
    rerender({ sync: DONE });

    // Advance well past the original timeout. The pending timer
    // must have been cleared by the effect cleanup; the backstop
    // must not fire even though `onStale` is still wired up.
    act(() => {
      vi.advanceTimersByTime(TIMEOUT * 3);
    });
    expect(onStale).not.toHaveBeenCalled();
  });

  it("cancels a pending timer when sync flips to error mid-flight", () => {
    const onStale = vi.fn();
    const { rerender } = renderHook(
      ({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale),
      { initialProps: { sync: ACTIVE } },
    );

    act(() => {
      vi.advanceTimersByTime(TIMEOUT / 2);
    });
    rerender({ sync: ERROR });

    act(() => {
      vi.advanceTimersByTime(TIMEOUT * 3);
    });
    expect(onStale).not.toHaveBeenCalled();
  });

  it("arms the timer from the initial idle state too", () => {
    // LoadingView mounts before the first Tauri event arrives.
    // `useSyncProgress` returns `ready: false, active: false` in
    // that brief window. The hook must still respect the timeout
    // so a misconfigured setup that never produces events does
    // not wedge the user on the loading screen.
    const onStale = vi.fn();
    renderHook(({ sync }) => useSyncBackstop(sync, TIMEOUT, onStale), {
      initialProps: { sync: IDLE },
    });

    act(() => {
      vi.advanceTimersByTime(TIMEOUT);
    });
    expect(onStale).toHaveBeenCalledTimes(1);
  });
});
