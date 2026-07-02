// Volume control — mute toggle + click-to-open slider that
// overlays the rest of the right cluster. The slider only
// appears on click of the volume button (not on hover), and
// is positioned absolutely so it doesn't push the queue /
// lyrics / EQ toggles around.
//
// The slider shares the drag-then-commit pattern with the seek
// bar. Clicking outside the component (or pressing Escape)
// closes the popover.

import { VolumeHighIcon, VolumeLowIcon, VolumeOffIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { usePlaybackContext } from "@/playback";

import { useDragCommit } from "./useDragCommit";

const VOLUME_STEP = 0.05;
const VOLUME_MIN = 0;
const VOLUME_MAX = 1;

function volumeIconFor(volume: number, muted: boolean) {
  if (muted || volume === 0) return VolumeOffIcon;
  if (volume < 0.5) return VolumeLowIcon;
  return VolumeHighIcon;
}

export function VolumeControl() {
  const { snapshot, setVolume, setMuted } = usePlaybackContext();
  const { volume, muted } = snapshot;
  const volumeDrag = useDragCommit({ value: muted ? 0 : volume });
  const effectiveVolume = muted ? 0 : volume;
  const progress = Math.max(0, Math.min(1, volumeDrag.value)) * 100;
  const volumeIcon = volumeIconFor(effectiveVolume, muted);

  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // Close on outside click or Escape.
  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  const commit = useCallback(async () => {
    const drag = volumeDrag.value;
    volumeDrag.finish();
    const committed = muted ? 0 : volume;
    if (drag === committed) return;
    try {
      await setVolume(drag);
    } catch (err) {
      toast.error(`Set volume: ${extractError(err, "unknown error")}`);
    }
  }, [volumeDrag, muted, volume, setVolume]);

  const onButtonClick = () => {
    // First click: open the popover. Second click: mute the
    // volume and close. Subsequent clicks alternate: open, mute
    // + close, open, mute + close, ...
    if (open) {
      setMuted(!muted).catch((err) =>
        toast.error(`Toggle mute: ${extractError(err, "unknown error")}`),
      );
      setOpen(false);
    } else {
      setOpen(true);
    }
  };

  return (
    <div
      ref={wrapperRef}
      className="relative z-30 flex items-center gap-0.5 rounded-md bg-muted/40 p-0.5"
    >
      <button
        type="button"
        onClick={onButtonClick}
        aria-label={muted ? "Unmute" : "Mute"}
        aria-pressed={muted}
        aria-expanded={open}
        aria-haspopup="dialog"
        className={cn(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-all",
          "hover:bg-muted hover:text-foreground",
          "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          (muted || open) && "bg-muted text-primary hover:bg-muted hover:text-primary",
        )}
      >
        <HugeiconsIcon icon={volumeIcon} size={16} strokeWidth={1.75} />
      </button>
      <div
        className={cn(
          "absolute top-1/2 left-9 -translate-y-1/2 flex items-center rounded-md border border-border bg-card px-3 transition-[width,opacity] duration-200 ease-out",
          open ? "w-32 h-9 opacity-100" : "w-0 h-9 opacity-0 pointer-events-none",
        )}
      >
        <input
          type="range"
          min={VOLUME_MIN}
          max={VOLUME_MAX}
          step={VOLUME_STEP}
          value={volumeDrag.value}
          onChange={volumeDrag.onChange}
          onPointerUp={commit}
          onKeyUp={(event) => {
            if (event.key === "Tab") return;
            commit();
          }}
          onBlur={commit}
          aria-label="Volume"
          tabIndex={open ? 0 : -1}
          className="player-range h-1 w-full cursor-pointer appearance-none rounded-full outline-none accent-primary"
          style={{
            background: `linear-gradient(to right, var(--primary) 0%, var(--primary) ${progress}%, var(--muted) ${progress}%, var(--muted) 100%)`,
          }}
        />
      </div>
    </div>
  );
}
