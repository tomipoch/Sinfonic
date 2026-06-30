// Transport controls — shuffle, prev, play/pause, next, repeat.
//
// The play/pause button is rendered inline because it's the only one
// with custom sizing + primary background; the others are IconButton
// instances. The "busy" lock (while an IPC is in flight) is
// published to `useTransportBusy()` so the SeekBar can lock too.

import { type ReactNode, useCallback } from "react";
import { toast } from "sonner";

import { MaterialSymbol } from "@/components/ui/MaterialSymbol";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { usePlaybackContext } from "@/playback";

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
 * state, sized to match the surrounding transport icons:
 *   - isPlaying=true  → 'pause'
 *   - isPlaying=false → 'play_arrow'
 *
 * No background — the icon itself sits in the accent colour
 * (`text-primary`) so it stands out without a coloured circle.
 * `weight={700}` keeps the glyph solid at this size; the rounded
 * variant of play_arrow without `fill` looks outline-y next to
 * the rest of the transport row.
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
        "flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-primary transition-all",
        "hover:bg-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        "disabled:opacity-40",
      )}
    >
      <MaterialSymbol
        name={isPlaying ? "pause" : "play_arrow"}
        size={22}
        weight={700}
        fill
        className={isPlaying ? "" : "translate-x-[1px]"}
      />
    </button>
  );
}

export function TransportControls({ canStep }: TransportControlsProps) {
  const { snapshot, togglePlay, next, previous } = usePlaybackContext();
  const { isPlaying } = snapshot;
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

  return (
    <div className="flex items-center gap-1">
      <IconButton ariaLabel="Shuffle" disabled className="opacity-40">
        <MaterialSymbol name="shuffle" size={18} />
      </IconButton>
      <IconButton ariaLabel="Previous track" onClick={onPrev} disabled={!canStep || actionLock}>
        <MaterialSymbol name="skip_previous" size={20} fill />
      </IconButton>
      <PlayPauseButton isPlaying={isPlaying} disabled={busy === "play"} onToggle={onTogglePlay} />
      <IconButton ariaLabel="Next track" onClick={onNext} disabled={!canStep || actionLock}>
        <MaterialSymbol name="skip_next" size={20} fill />
      </IconButton>
      <IconButton ariaLabel="Repeat" disabled className="opacity-40">
        <MaterialSymbol name="repeat" size={18} />
      </IconButton>
    </div>
  );
}
