// Repeat-mode cycle.
//
// The transport cycles `off → all → one → off` on every toggle. The
// order is intentional: "all" is one click away from the default
// "off", which is the most useful state.

import type { RepeatMode } from "@/types/domain";

export const REPEAT_CYCLE: ReadonlyArray<RepeatMode> = ["off", "all", "one"];

export function nextRepeat(current: RepeatMode): RepeatMode {
  const idx = REPEAT_CYCLE.indexOf(current);
  return REPEAT_CYCLE[(idx + 1) % REPEAT_CYCLE.length] ?? "off";
}

export function repeatLabel(mode: RepeatMode, style: "short" | "long" = "long"): string {
  const long = {
    off: "Repeat off",
    all: "Repeat all",
    one: "Repeat one",
  } as const;
  const short = { off: "Off", all: "All", one: "One" } as const;
  return (style === "long" ? long : short)[mode];
}
