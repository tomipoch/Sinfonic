// Seek bar — drag-to-position with commit-on-release.
//
// The wavy SVG behind the slider gives the bar a visual identity
// when no track is loaded (the slider is then disabled, but the
// track still renders).

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

const WAVE_PATH = `M 0 4 ${"q 3.125 -3.5 6.25 0 ".repeat(16).trimEnd()}`;

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
    <div className="flex w-full items-center gap-2">
      <span className="w-9 shrink-0 text-right font-mono text-[10px] tabular-nums text-muted-foreground">
        {formatDuration(seekDrag.value)}
      </span>
      <div className="relative h-2 flex-1">
        <svg
          className="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
          viewBox="0 0 100 8"
          preserveAspectRatio="none"
          aria-hidden
        >
          <path
            d={WAVE_PATH}
            fill="none"
            stroke="var(--muted)"
            strokeWidth={0.75}
            vectorEffect="non-scaling-stroke"
          />
          <path
            d={WAVE_PATH}
            fill="none"
            stroke="var(--primary)"
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
            style={{
              clipPath: `inset(0 ${100 - progress}% 0 0)`,
              WebkitClipPath: `inset(0 ${100 - progress}% 0 0)`,
            }}
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
      <span className="w-9 shrink-0 font-mono text-[10px] tabular-nums text-muted-foreground">
        {formatDuration(durationSeconds)}
      </span>
    </div>
  );
}

// Lazy-load toast so the volume/transport modules don't pull in sonner
// just for an error path that almost never fires. Imported lazily here
// to keep this leaf component free of top-level side effects.
