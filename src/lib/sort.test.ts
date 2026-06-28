import { describe, expect, it } from "vitest";

import { compareNumber, compareNumberDesc, compareString } from "./sort";

describe("compareString", () => {
  it("uses case-insensitive locale compare (ascending)", () => {
    expect(compareString("alice", "Bob")).toBeLessThan(0);
    expect(compareString("ALICE", "alice")).toBe(0);
  });

  it("returns 0 for equal strings", () => {
    expect(compareString("xyz", "xyz")).toBe(0);
  });
});

describe("compareNumber", () => {
  it("ascending numeric", () => {
    expect(compareNumber(1, 5)).toBeLessThan(0);
    expect(compareNumber(5, 1)).toBeGreaterThan(0);
  });

  it("treats null and undefined as the fallback value (default 0)", () => {
    expect(compareNumber(null, null)).toBe(0);
    expect(compareNumber(undefined, 5)).toBeLessThan(0);
    expect(compareNumber(10, undefined)).toBeGreaterThan(0);
  });

  it("accepts a custom fallback", () => {
    expect(compareNumber(null, null, 100)).toBe(0);
    expect(compareNumber(null, 5, 100)).toBeGreaterThan(0);
  });
});

describe("compareNumberDesc", () => {
  it("descending numeric", () => {
    expect(compareNumberDesc(1, 5)).toBeGreaterThan(0);
    expect(compareNumberDesc(5, 1)).toBeLessThan(0);
  });

  it("treats null and undefined as the fallback (0)", () => {
    // Descending: 5 should come before null (which sorts as 0).
    expect(compareNumberDesc(null, 5)).toBeGreaterThan(0);
    expect(compareNumberDesc(5, null)).toBeLessThan(0);
  });
});