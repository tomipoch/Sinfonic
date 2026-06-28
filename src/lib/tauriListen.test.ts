import { describe, expect, it, vi } from "vitest";

import { safelyUnlisten } from "./tauriListen";

describe("safelyUnlisten", () => {
  it("does nothing when the listener is null", () => {
    expect(() => safelyUnlisten(null)).not.toThrow();
    expect(() => safelyUnlisten(undefined)).not.toThrow();
  });

  it("swallows sync throws from unlisten", () => {
    const fn = vi.fn(() => {
      throw new Error("listener already gone");
    });
    expect(() => safelyUnlisten(fn as never)).not.toThrow();
    expect(fn).toHaveBeenCalledOnce();
  });

  it("swallows async rejections from unlisten", async () => {
    const fn = vi.fn(() => Promise.reject(new Error("async rejection")));
    expect(() => safelyUnlisten(fn as never)).not.toThrow();
    // Drain the rejection so the test runtime doesn't flag an
    // unhandled rejection (the helper should already swallow it
    // but we want a clean test).
    await Promise.resolve();
  });

  it("tolerates unlisten returning a non-promise value", () => {
    const fn = vi.fn(() => undefined);
    expect(() => safelyUnlisten(fn as never)).not.toThrow();
    expect(fn).toHaveBeenCalledOnce();
  });

  it("tolerates unlisten returning null", () => {
    const fn = vi.fn(() => null);
    expect(() => safelyUnlisten(fn as never)).not.toThrow();
    expect(fn).toHaveBeenCalledOnce();
  });
});