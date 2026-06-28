// Repeat-mode helpers.
//
// The transport cycles `off → all → one → off` on every toggle; the
// cycle order is intentionally `[off, all, one]` (not alphabetic) so
// the most useful state ("all") is one click away from the default
// ("off").
//
// Centralised so `QueueView` and `QueuePanel` share the exact same
// cycle order and label mapping.

import type { RepeatMode } from "@/types/domain";

export const REPEAT_CYCLE: ReadonlyArray<RepeatMode> = ["off", "all", "one"];

/** Return the next repeat mode in the cycle, wrapping around. */
export function nextRepeat(current: RepeatMode): RepeatMode {
  const idx = REPEAT_CYCLE.indexOf(current);
  return REPEAT_CYCLE[(idx + 1) % REPEAT_CYCLE.length] ?? "off";
}

/**
 * Human-readable label for the given repeat mode. The two views
 * historically used different label styles (the full-screen view
 * shows `Repeat all`, the side panel just `All`), so the `style`
 * parameter preserves both.
 */
export function repeatLabel(mode: RepeatMode, style: "short" | "long" = "long"): string {
  const long = {
    off: "Repeat off",
    all: "Repeat all",
    one: "Repeat one",
  } as const;
  const short = { off: "Off", all: "All", one: "One" } as const;
  return (style === "long" ? long : short)[mode];
}
