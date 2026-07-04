// Transport controls — shuffle, prev, play/pause, next, repeat.
//
// The play/pause button is rendered inline because it's the only
// one with custom sizing + the primary-coloured circle that
// anchors the transport row; the others are IconButton instances.
// The "busy" lock (while an IPC is in flight) is published to
// `useTransportBusy()` so the SeekBar can lock too.

import { type ReactNode, useCallback } from "react";
import { toast } from "sonner";

import { MaterialSymbol } from "@/components/ui/MaterialSymbol";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { usePlaybackContext } from "@/playback";
import { repeatLabel } from "@/playback/repeat";

import { useTransportBusy } from "./TransportBusyContext";

interface IconButtonProps {
  ariaLabel: string;
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  active?: boolean;
  className?: string;
}

function IconButton({
  ariaLabel,
  children,
  onClick,
  disabled,
  active,
  className,
}: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      title={ariaLabel}
      aria-pressed={active ? true : undefined}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-all",
        "hover:bg-muted hover:text-foreground",
        "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        active && "bg-muted text-primary hover:bg-muted hover:text-primary",
        disabled &&
          "cursor-not-allowed opacity-40 hover:bg-transparent hover:text-muted-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}

interface TransportControlsProps {
  /** True when the queue has entries (controls prev/next can step). */
  canStep: boolean;
}

/**
 * Play / pause toggle.
 *
 * Renders the Google Material Symbols glyph for the current
 * state inside a coloured circular button:
 *   - isPlaying=true  → 'pause'
 *   - isPlaying=false → 'play_arrow'
 *
 * The circle uses `bg-primary` so the play / pause button stays
 * the visual anchor of the transport row even with the larger
 * skip buttons around it. `weight={700}` keeps the glyph solid
 * at this size; the rounded variant of play_arrow without
 * `fill` looks outline-y next to the rest of the transport row.
 */
function PlayPauseButton({
  isPlaying,
  disabled,
  onToggle,
}: {
  isPlaying: boolean;
  disabled: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      disabled={disabled}
      aria-label={isPlaying ? "Pause" : "Play"}
      title={isPlaying ? "Pause" : "Play"}
      className={cn(
        "group relative flex h-8 w-8 sm:h-9 sm:w-9 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-sm transition-all",
        "hover:scale-105 hover:shadow-md hover:shadow-primary/20 active:scale-95",
        "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card",
        "disabled:opacity-40 disabled:hover:scale-100 disabled:hover:shadow-sm",
      )}
    >
      <MaterialSymbol
        name={isPlaying ? "pause" : "play_arrow"}
        size={22}
        weight={700}
        fill
        className="[--mat-symbol-size:20px] sm:[--mat-symbol-size:22px] translate-x-[1px]"
      />
    </button>
  );
}

export function TransportControls({ canStep }: TransportControlsProps) {
  const { snapshot, togglePlay, next, previous, cycleRepeat, setShuffle } = usePlaybackContext();
  const { isPlaying, repeat, shuffle } = snapshot;
  const { setBusy, busy } = useTransportBusy();
  const actionLock = busy !== null;

  const run = useCallback(
    async <T,>(
      action: "play" | "prev" | "next",
      fn: () => Promise<T>,
      label: string,
    ): Promise<void> => {
      if (busy !== null) return;
      setBusy(action);
      try {
        await fn();
      } catch (err) {
        toast.error(`${label}: ${extractError(err, "unknown error")}`);
      } finally {
        setBusy(null);
      }
    },
    [busy, setBusy],
  );

  const onTogglePlay = () => run("play", () => togglePlay(), "Playback");
  const onPrev = () => run("prev", () => previous(), "Previous");
  const onNext = () => run("next", () => next(), "Next");

  const onShuffle = () => {
    void (async () => {
      try {
        await setShuffle(!shuffle);
      } catch (err) {
        toast.error(`Shuffle: ${extractError(err, "unknown error")}`);
      }
    })();
  };

  const onRepeat = () => {
    void (async () => {
      try {
        await cycleRepeat();
      } catch (err) {
        toast.error(`Repeat: ${extractError(err, "unknown error")}`);
      }
    })();
  };

  return (
    <div className="flex items-center gap-1 sm:gap-1.5">
      <IconButton
        ariaLabel={shuffle ? "Disable shuffle" : "Enable shuffle"}
        onClick={onShuffle}
        active={shuffle}
        disabled={actionLock}
        className="hidden data-[panels-open=true]:inline-flex"
      >
        <MaterialSymbol name="shuffle" size={18} fill={shuffle} />
      </IconButton>
      <IconButton ariaLabel="Previous track" onClick={onPrev} disabled={!canStep || actionLock}>
        <MaterialSymbol
          name="skip_previous"
          size={22}
          fill
          className="[--mat-symbol-size:18px] sm:[--mat-symbol-size:22px]"
        />
      </IconButton>
      <PlayPauseButton isPlaying={isPlaying} disabled={busy === "play"} onToggle={onTogglePlay} />
      <IconButton ariaLabel="Next track" onClick={onNext} disabled={!canStep || actionLock}>
        <MaterialSymbol
          name="skip_next"
          size={22}
          fill
          className="[--mat-symbol-size:18px] sm:[--mat-symbol-size:22px]"
        />
      </IconButton>
      <IconButton
        ariaLabel={`Repeat: ${repeatLabel(repeat)}`}
        onClick={onRepeat}
        active={repeat !== "off"}
        disabled={actionLock}
        className="hidden data-[panels-open=true]:inline-flex"
      >
        <MaterialSymbol
          name={repeat === "one" ? "repeat_one" : "repeat"}
          size={18}
          fill={repeat !== "off"}
        />
      </IconButton>
    </div>
  );
}
