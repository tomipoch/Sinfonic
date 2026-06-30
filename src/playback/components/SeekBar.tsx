// Seek bar — drag-to-position with commit-on-release.
//
// Two paths share the same viewBox:
//   - A flat horizontal line covers the full width as the
//     unplayed segment.
//   - A wavy stroke (pathLength normalised to 100) is drawn on top
//     with `stroke-dasharray=100` and `stroke-dashoffset` advancing
//     with the playhead, so the played portion reveals itself as
//     the wavy path while the rest stays hidden.
//
// Using `pathLength="100"` + dasharray instead of `clip-path: inset()`
// makes the reveal robust across WebKit / Safari where SVG path
// clip-path can be flaky. The native <input type="range"> sits on top
// of the SVG with a transparent track so the visuals stay bound to
// the theme variables and the slider keeps full keyboard / a11y.

import { useCallback } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { usePlaybackContext } from "@/playback";
import { useTransportBusy } from "./TransportBusyContext";
import { useDragCommit } from "./useDragCommit";

interface SeekBarProps {
  /** True when there's a track with a known duration. */
  enabled: boolean;
}

// 8-cycle wave with amplitude ±5 inside a 16-unit-tall viewBox. Wider
// peaks than 16 cycles of ±6 because at this scale a denser wave
// turned into a blur; 8 sharp peaks reads as a waveform instead.
const WAVE_PATH = `M 0 8 ${"c 1.5 -5 4 -5 6.25 0 c 2.25 5 4.75 5 6.25 0 ".repeat(4).trimEnd()}`;
const LINE_PATH = "M 0 8 L 100 8";

export function SeekBar({ enabled }: SeekBarProps) {
  const { snapshot, seekTo } = usePlaybackContext();
  const { positionSeconds, durationSeconds } = snapshot;
  const seekDrag = useDragCommit({ value: positionSeconds });
  const { isBusy } = useTransportBusy();

  const commit = useCallback(async () => {
    const drag = seekDrag.value;
    seekDrag.finish();
    if (drag === positionSeconds) return;
    try {
      await seekTo(drag);
    } catch (err) {
      toast.error(`Seek: ${extractError(err, "unknown error")}`);
    }
  }, [seekDrag, positionSeconds, seekTo]);

  const progress =
    durationSeconds > 0 ? Math.min(100, (seekDrag.value / durationSeconds) * 100) : 0;

  return (
    <div className="flex w-full max-w-md items-center gap-2.5">
      <span className="w-10 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted-foreground">
        {formatDuration(seekDrag.value)}
      </span>
      <div className="relative h-4 flex-1">
        <svg
          className="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
          viewBox="0 0 100 16"
          preserveAspectRatio="none"
          aria-hidden
        >
          {/* Unplayed segment — a flat horizontal line at mid-height. */}
          <path
            d={LINE_PATH}
            fill="none"
            stroke="var(--muted-foreground)"
            strokeOpacity={0.4}
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
          {/* Played segment — same wavy path revealed up to progress%.
              pathLength="100" normalises the path so dashoffset maps
              directly to a 0–100 percentage. */}
          <path
            d={WAVE_PATH}
            fill="none"
            stroke="var(--primary)"
            strokeWidth={1.75}
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
            pathLength={100}
            strokeDasharray={100}
            strokeDashoffset={100 - progress}
          />
        </svg>
        <input
          type="range"
          min={0}
          max={durationSeconds || 0}
          step={1}
          value={seekDrag.value}
          onChange={seekDrag.onChange}
          onPointerUp={commit}
          onKeyUp={(event) => {
            if (event.key === "Tab") return;
            commit();
          }}
          onBlur={commit}
          disabled={!enabled || isBusy}
          aria-label="Seek"
          aria-valuemin={0}
          aria-valuemax={durationSeconds}
          aria-valuenow={seekDrag.value}
          className={cn(
            "player-range-wave absolute inset-0 h-full w-full cursor-pointer appearance-none bg-transparent outline-none",
            "disabled:cursor-not-allowed disabled:opacity-40",
          )}
        />
      </div>
      <span className="w-10 shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground">
        {formatDuration(durationSeconds)}
      </span>
    </div>
  );
}

// Lazy-load toast so the volume/transport modules don't pull in sonner
// just for an error path that almost never fires. Imported lazily here
// to keep this leaf component free of top-level side effects.
