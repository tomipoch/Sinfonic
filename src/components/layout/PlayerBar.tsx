// PlayerBar — bottom transport bar.
//
// Three sections (Spotify / Apple Music-style):
//   left:   cover + track title + artist
//   center: shuffle + prev + play/pause + next + repeat, with seek row
//   right:  volume + queue toggle + EQ toggle
//
// All state reads from the playback + queue stores, which are kept
// in sync by the global event bridge at the app root. Click handlers
// call the typed IPC wrappers directly; the resulting events update
// the stores for every other component.
//
// Seek slider commits on pointer-up / key-up / blur (not on every
// onChange) to avoid spamming the backend while the user drags.
//
// EQ panel lives in a popover anchored above the player bar; toggle
// state lives in the PlayerBar so the panel auto-closes when the
// user navigates away or hits Esc.

import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  LeftToRightListBulletIcon,
  MusicNote01Icon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PreviousIcon,
  RepeatIcon,
  ShuffleIcon,
  SlidersHorizontalIcon,
  VolumeHighIcon,
  VolumeLowIcon,
  VolumeOffIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

import {
  next,
  pause,
  previous,
  resume,
  seek,
  setMuted,
  setVolume,
  queueAddMany,
} from "@/lib/tauri";
import { usePlaybackStore } from "@/stores/playbackStore";
import { useQueueStore } from "@/stores/queueStore";
import { cn } from "@/lib/cn";
import { formatDuration } from "@/lib/format";
import { EqPanel } from "@/components/views/EqPanel";
import { useDropTarget } from "@/hooks/useDropTarget";

const VOLUME_STEP = 0.05;
const VOLUME_MIN = 0;
const VOLUME_MAX = 1;

type Props = {
  queueOpen: boolean;
  onToggleQueue: () => void;
};

export function PlayerBar({ queueOpen, onToggleQueue }: Props) {
  const currentTrack = usePlaybackStore((s) => s.currentTrack);
  const isPlaying = usePlaybackStore((s) => s.isPlaying);
  const volume = usePlaybackStore((s) => s.volume);
  const muted = usePlaybackStore((s) => s.muted);
  const positionSeconds = usePlaybackStore((s) => s.positionSeconds);
  const durationSeconds = usePlaybackStore((s) => s.durationSeconds);

  const queueLength = useQueueStore((s) => s.entries.length);

  const [busy, setBusy] = useState(false);
  const [seekDrag, setSeekDrag] = useState<number | null>(null);
  const seekDragRef = useRef<number | null>(null);
  const [eqOpen, setEqOpen] = useState(false);

  const run = async (fn: () => Promise<void>, label: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await fn();
    } catch (err) {
      toast.error(`${label}: ${(err as Error).message ?? String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const onTogglePlay = () =>
    void run(
      () => (isPlaying ? pause() : resume()),
      isPlaying ? "Pause" : "Resume",
    );

  const onPrev = () => void run(() => previous(), "Previous");
  const onNext = () => void run(() => next(), "Next");

  const onVolumeChange = (nextVolume: number) =>
    void run(async () => {
      await setVolume(nextVolume);
      usePlaybackStore.getState().setVolume(nextVolume);
    }, "Set volume");

  const onMuteToggle = () =>
    void run(async () => {
      const nextMuted = !muted;
      await setMuted(nextMuted);
      usePlaybackStore.getState().setMuted(nextMuted);
    }, "Toggle mute");

  const commitSeek = (rawValue: number) => {
    const value = Math.max(0, Math.min(durationSeconds || 0, Math.round(rawValue)));
    seekDragRef.current = null;
    setSeekDrag(null);
    void run(async () => {
      await seek(value);
      usePlaybackStore.getState().setPosition(value);
    }, "Seek");
  };

  const onSeekChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextValue = Number(e.currentTarget.value);
    seekDragRef.current = nextValue;
    setSeekDrag(nextValue);
  };

  const hasTrack = currentTrack !== null;
  const effectiveVolume = muted ? 0 : volume;
  const transportDisabled = !hasTrack || busy;
  const seekEnabled = hasTrack && durationSeconds > 0;
  const displayedPosition = seekDrag ?? positionSeconds;
  const seekProgress =
    durationSeconds > 0
      ? Math.min(100, (displayedPosition / durationSeconds) * 100)
      : 0;

  useEffect(() => {
    if (!eqOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setEqOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [eqOpen]);

  const { dragOver, droppableProps } = useDropTarget({
    onDrop: async (tracks) => {
      if (tracks.length === 0) return;
      try {
        await queueAddMany(tracks);
        toast.success(`Added ${tracks.length} track${tracks.length !== 1 ? "s" : ""} to queue`);
      } catch (err) {
        toast.error(`Couldn't add to queue: ${(err as Error).message}`);
      }
    },
  });

  return (
    <footer
      {...droppableProps}
      className={cn(
        "relative flex h-[5.5rem] shrink-0 items-center justify-between gap-6 border-t border-border bg-card/80 px-5 backdrop-blur supports-[backdrop-filter]:bg-card/60 transition-colors",
        dragOver && "bg-primary/10 ring-1 ring-inset ring-primary/40",
      )}
      role="contentinfo"
      aria-label="Player controls"
    >
      {eqOpen && (
        <div className="absolute bottom-full right-5 mb-2 w-[min(36rem,calc(100vw-2rem))] z-20">
          <EqPanel />
        </div>
      )}

      {/* LEFT — cover + track meta */}
      <div className="flex min-w-0 flex-1 items-center gap-3.5">
        <div
          className="group relative flex h-14 w-14 shrink-0 items-center justify-center overflow-hidden rounded-md bg-gradient-to-br from-secondary to-muted ring-1 ring-inset ring-border/60"
          aria-hidden
        >
          <HugeiconsIcon
            icon={MusicNote01Icon}
            size={22}
            strokeWidth={1.5}
            className="text-muted-foreground/70 transition-colors group-hover:text-foreground/80"
          />
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <div
            className={cn(
              "truncate text-sm font-semibold tracking-tight",
              hasTrack ? "text-foreground" : "text-muted-foreground",
            )}
            title={currentTrack?.title}
          >
            {currentTrack?.title ?? "Nothing playing"}
          </div>
          <div
            className="truncate text-xs text-muted-foreground"
            title={currentTrack?.artist}
          >
            {currentTrack?.artist ?? "—"}
          </div>
        </div>
      </div>

      {/* CENTER — transport + seek */}
      <div className="flex w-full max-w-2xl flex-col items-center gap-1.5">
        <div className="flex items-center gap-1">
          <IconButton ariaLabel="Shuffle" disabled className="opacity-40">
            <HugeiconsIcon icon={ShuffleIcon} size={16} strokeWidth={1.75} />
          </IconButton>
          <IconButton
            ariaLabel="Previous track"
            onClick={onPrev}
            disabled={transportDisabled || queueLength === 0}
          >
            <HugeiconsIcon icon={PreviousIcon} size={18} strokeWidth={1.75} />
          </IconButton>
          <button
            type="button"
            onClick={onTogglePlay}
            disabled={transportDisabled}
            aria-label={isPlaying ? "Pause" : "Play"}
            className={cn(
              "group relative flex h-10 w-10 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-sm transition-all",
              "hover:scale-105 hover:shadow-md hover:shadow-primary/20 active:scale-95",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card",
              "disabled:opacity-40 disabled:hover:scale-100 disabled:hover:shadow-sm",
            )}
          >
            {isPlaying ? (
              <HugeiconsIcon icon={PauseIcon} size={18} strokeWidth={2} />
            ) : (
              <HugeiconsIcon
                icon={PlayIcon}
                size={18}
                strokeWidth={2}
                className="translate-x-[1px]"
              />
            )}
          </button>
          <IconButton
            ariaLabel="Next track"
            onClick={onNext}
            disabled={transportDisabled || queueLength === 0}
          >
            <HugeiconsIcon icon={NextIcon} size={18} strokeWidth={1.75} />
          </IconButton>
          <IconButton ariaLabel="Repeat" disabled className="opacity-40">
            <HugeiconsIcon icon={RepeatIcon} size={16} strokeWidth={1.75} />
          </IconButton>
        </div>

        <div className="flex w-full items-center gap-2.5">
          <span className="w-10 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted-foreground">
            {formatDuration(displayedPosition)}
          </span>
          <input
            type="range"
            min={0}
            max={durationSeconds || 0}
            step={1}
            value={displayedPosition}
            onChange={onSeekChange}
            onPointerUp={() => {
              if (seekDragRef.current !== null) commitSeek(seekDragRef.current);
            }}
            onKeyUp={(e) => {
              if (e.key === "Tab") return;
              if (seekDragRef.current !== null) commitSeek(seekDragRef.current);
            }}
            onBlur={() => {
              if (seekDragRef.current !== null) commitSeek(seekDragRef.current);
            }}
            disabled={!seekEnabled}
            aria-label="Seek"
            aria-valuemin={0}
            aria-valuemax={durationSeconds}
            aria-valuenow={displayedPosition}
            className="player-range h-1 flex-1 cursor-pointer appearance-none rounded-full bg-muted outline-none transition-colors disabled:cursor-not-allowed disabled:opacity-40"
            style={{
              background: `linear-gradient(to right, var(--primary) 0%, var(--primary) ${seekProgress}%, var(--muted) ${seekProgress}%, var(--muted) 100%)`,
            }}
          />
          <span className="w-10 shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground">
            {formatDuration(durationSeconds)}
          </span>
        </div>
      </div>

      {/* RIGHT — volume + queue + EQ */}
      <div className="flex min-w-0 flex-1 items-center justify-end gap-1">
        <div className="flex items-center gap-2 rounded-md px-1.5 py-1 hover:bg-muted/60">
          <button
            type="button"
            onClick={onMuteToggle}
            aria-label={muted ? "Unmute" : "Mute"}
            aria-pressed={muted}
            className={cn(
              "flex h-7 w-7 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:text-foreground",
              muted && "text-primary",
            )}
          >
            <VolumeIcon volume={effectiveVolume} muted={muted} />
          </button>
          <input
            type="range"
            min={VOLUME_MIN}
            max={VOLUME_MAX}
            step={VOLUME_STEP}
            value={effectiveVolume}
            onChange={(e) => void onVolumeChange(Number(e.currentTarget.value))}
            aria-label="Volume"
            className="player-range h-1 w-24 cursor-pointer appearance-none rounded-full bg-muted outline-none accent-primary transition-colors"
            style={{
              background: `linear-gradient(to right, var(--primary) 0%, var(--primary) ${
                effectiveVolume * 100
              }%, var(--muted) ${effectiveVolume * 100}%, var(--muted) 100%)`,
            }}
          />
        </div>

        <div className="mx-1 h-5 w-px shrink-0 bg-border" aria-hidden />

        <IconButton
          ariaLabel="Toggle queue"
          onClick={onToggleQueue}
          aria-expanded={queueOpen}
          aria-pressed={queueOpen}
          active={queueOpen}
        >
          <HugeiconsIcon icon={LeftToRightListBulletIcon} size={16} strokeWidth={1.75} />
        </IconButton>
        <IconButton
          ariaLabel="Toggle equalizer"
          onClick={() => setEqOpen((open) => !open)}
          aria-expanded={eqOpen}
          aria-pressed={eqOpen}
          active={eqOpen}
        >
          <HugeiconsIcon icon={SlidersHorizontalIcon} size={16} strokeWidth={1.75} />
        </IconButton>
      </div>
    </footer>
  );
}

function VolumeIcon({ volume, muted }: { volume: number; muted: boolean }) {
  if (muted || volume === 0) {
    return <HugeiconsIcon icon={VolumeOffIcon} size={16} strokeWidth={1.75} />;
  }
  if (volume < 0.5) {
    return <HugeiconsIcon icon={VolumeLowIcon} size={16} strokeWidth={1.75} />;
  }
  return <HugeiconsIcon icon={VolumeHighIcon} size={16} strokeWidth={1.75} />;
}

type IconButtonProps = {
  ariaLabel: string;
  children: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  active?: boolean;
  className?: string;
};

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
        disabled && "cursor-not-allowed opacity-40 hover:bg-transparent hover:text-muted-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}
