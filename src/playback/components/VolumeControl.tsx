// Volume control — mute toggle + hidden-by-default slider that
// expands on hover / focus. The slider shares the drag-then-commit
// pattern with the seek bar.

import { VolumeHighIcon, VolumeLowIcon, VolumeOffIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback } from "react";
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

  const onMuteToggle = () => {
    setMuted(!muted).catch((err) =>
      toast.error(`Toggle mute: ${extractError(err, "unknown error")}`),
    );
  };

  return (
    <div className="group flex items-center gap-2 rounded-md px-1.5 py-1 hover:bg-muted/60 focus-within:bg-muted/60">
      <button
        type="button"
        onClick={onMuteToggle}
        aria-label={muted ? "Unmute" : "Mute"}
        aria-pressed={muted}
        className={cn(
          "flex h-7 w-7 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors group-hover:text-foreground",
          muted && "text-primary",
        )}
      >
        <HugeiconsIcon icon={volumeIcon} size={16} strokeWidth={1.75} />
      </button>
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
        tabIndex={0}
        className="player-range h-1 w-0 cursor-pointer appearance-none rounded-full bg-muted opacity-0 outline-none accent-primary transition-[width,opacity,padding] duration-200 ease-out group-hover:w-24 group-hover:opacity-100 group-focus-within:w-24 group-focus-within:opacity-100 focus:w-24 focus:opacity-100"
        style={{
          background: `linear-gradient(to right, var(--primary) 0%, var(--primary) ${progress}%, var(--muted) ${progress}%, var(--muted) 100%)`,
        }}
      />
    </div>
  );
}
