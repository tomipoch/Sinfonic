import { describe, expect, it } from "vitest";

import {
  nextRepeat,
  REPEAT_CYCLE,
  repeatLabel,
} from "./repeat";

describe("nextRepeat", () => {
  it("cycles through the documented order", () => {
    expect(REPEAT_CYCLE).toEqual(["off", "all", "one"]);
    expect(nextRepeat("off")).toBe("all");
    expect(nextRepeat("all")).toBe("one");
    expect(nextRepeat("one")).toBe("off");
  });

  it("wraps around when the input is unknown", () => {
    expect(nextRepeat("off")).toBe("all");
  });
});

describe("repeatLabel", () => {
  it("returns the long-form labels by default", () => {
    expect(repeatLabel("off")).toBe("Repeat off");
    expect(repeatLabel("all")).toBe("Repeat all");
    expect(repeatLabel("one")).toBe("Repeat one");
  });

  it("returns the short-form labels when style is 'short'", () => {
    expect(repeatLabel("off", "short")).toBe("Off");
    expect(repeatLabel("all", "short")).toBe("All");
    expect(repeatLabel("one", "short")).toBe("One");
  });
});