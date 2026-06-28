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
// P2 split: the previous 471-line monolith re-rendered on every
// `positionSeconds` tick (1Hz). The bar is now three memoized
// sections; `<SeekBar />` owns its own slice of position so the
// transport buttons and now-playing chrome don't repaint 1Hz. The
// `<TransportControls />` and `<VolumeControls />` only re-render
// when their specific inputs change.
//
// Seek slider commits on pointer-up / key-up / blur (not on every
// onChange) to avoid spamming the backend while the user drags.
//
// EQ panel lives in a popover anchored above the player bar; toggle
// state lives in the PlayerBar so the panel auto-closes when the
// user navigates away or hits Esc.

import {
  LeftToRightListBulletIcon,
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
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { AlbumCover } from "@/components/ui/AlbumCover";
import { EqPanel } from "@/components/views/EqPanel";
import { useDropTarget } from "@/hooks/useDropTarget";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import {
  next,
  pause,
  previous,
  queueAddMany,
  resume,
  seek,
  setMuted,
  setVolume,
} from "@/lib/tauri";
import { useLibraryStore } from "@/stores/libraryStore";
import { usePlaybackStore } from "@/stores/playbackStore";
import { useQueueStore } from "@/stores/queueStore";
import type { Album, Track } from "@/types/domain";

const VOLUME_STEP = 0.05;
const VOLUME_MIN = 0;
const VOLUME_MAX = 1;

type Props = {
  queueOpen: boolean;
  onToggleQueue: () => void;
};

export function PlayerBar({ queueOpen, onToggleQueue }: Props) {
  // PlayerBar itself only needs the toggle-state from the queue +
  // EQ panels; transport position is read inside <SeekBar /> so the
  // rest of the chrome doesn't repaint every second.
  const queueLength = useQueueStore((s) => s.entries.length);
  const tracks = useLibraryStore((s) => s.tracks);
  const albums = useLibraryStore((s) => s.albums);

  const [busy, setBusy] = useState(false);
  const [eqOpen, setEqOpen] = useState(false);

  const run = async (fn: () => Promise<void>, label: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await fn();
    } catch (err) {
      toast.error(`${label}: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onPrev = () => void run(() => previous(), "Previous");
  const onNext = () => void run(() => next(), "Next");
  const onCommitSeek = (value: number) =>
    void run(async () => {
      await seek(value);
      usePlaybackStore.getState().setPosition(value);
    }, "Seek");

  const onVolumeChange = (nextVolume: number) =>
    void run(async () => {
      await setVolume(nextVolume);
      usePlaybackStore.getState().setVolume(nextVolume);
    }, "Set volume");

  const onMuteToggle = () =>
    void run(async () => {
      const nextMuted = !usePlaybackStore.getState().muted;
      await setMuted(nextMuted);
      usePlaybackStore.getState().setMuted(nextMuted);
    }, "Toggle mute");

  useEffect(() => {
    if (!eqOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setEqOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [eqOpen]);

  const { dragOver, droppableProps } = useDropTarget({
    onDrop: async (dropped) => {
      if (dropped.length === 0) return;
      try {
        await queueAddMany(dropped);
        toast.success(`Added ${dropped.length} track${dropped.length !== 1 ? "s" : ""} to queue`);
      } catch (err) {
        toast.error(`Couldn't add to queue: ${extractError(err, "unknown error")}`);
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

      <NowPlaying albums={albums} tracks={tracks} />
      <TransportControls
        queueLength={queueLength}
        busy={busy}
        onPrev={onPrev}
        onNext={onNext}
        onCommitSeek={onCommitSeek}
      />
      <VolumeControls
        onVolumeChange={onVolumeChange}
        onMuteToggle={onMuteToggle}
        queueOpen={queueOpen}
        onToggleQueue={onToggleQueue}
        eqOpen={eqOpen}
        onToggleEq={() => setEqOpen((open) => !open)}
      />
    </footer>
  );
}

// ─── LEFT — cover + track meta ─────────────────────────────────────

interface NowPlayingProps {
  albums: Album[];
  tracks: Track[];
}

const NowPlaying = memo(function NowPlaying({ albums, tracks }: NowPlayingProps) {
  const currentTrack = usePlaybackStore((s) => s.currentTrack);

  // Build lookups only when the source list changes.
  const albumById = useMemo(() => {
    const map = new Map<string, Album>();
    for (const a of albums) map.set(a.id, a);
    return map;
  }, [albums]);
  const trackById = useMemo(() => {
    const map = new Map<string, Track>();
    for (const t of tracks) map.set(t.id, t);
    return map;
  }, [tracks]);

  const nowPlayingCover = useMemo(() => {
    if (!currentTrack) return null;
    const full = trackById.get(currentTrack.trackId);
    if (full?.imageRef) {
      return {
        id: full.id,
        title: full.album || full.title,
        imageRef: full.imageRef,
      };
    }
    if (full) {
      const album = albumById.get(full.albumId);
      if (album?.imageRef) {
        return {
          id: full.id,
          title: album.title,
          imageRef: album.imageRef,
        };
      }
    }
    return null;
  }, [currentTrack, trackById, albumById]);

  const hasTrack = currentTrack !== null;

  return (
    <div className="flex min-w-0 flex-1 items-center gap-3.5">
      <div className="h-14 w-14 shrink-0" aria-hidden={!nowPlayingCover}>
        {nowPlayingCover ? (
          <AlbumCover
            source={nowPlayingCover}
            ariaLabel={`Cover art for ${nowPlayingCover.title}`}
            className="h-14 w-14 rounded-md shadow-sm ring-1 ring-inset ring-border/40"
          />
        ) : (
          <div className="flex h-14 w-14 items-center justify-center rounded-md bg-gradient-to-br from-secondary to-muted ring-1 ring-inset ring-border/60">
            <HugeiconsIcon
              icon={LeftToRightListBulletIcon}
              size={20}
              strokeWidth={1.5}
              className="text-muted-foreground/70"
            />
          </div>
        )}
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
        <div className="truncate text-xs text-muted-foreground" title={currentTrack?.artist}>
          {currentTrack?.artist ?? "—"}
        </div>
      </div>
    </div>
  );
});

// ─── CENTER — transport + seek ─────────────────────────────────────

interface TransportControlsProps {
  queueLength: number;
  busy: boolean;
  onPrev: () => void;
  onNext: () => void;
  onCommitSeek: (value: number) => void;
}

const TransportControls = memo(function TransportControls({
  queueLength,
  busy,
  onPrev,
  onNext,
  onCommitSeek,
}: TransportControlsProps) {
  const isPlaying = usePlaybackStore((s) => s.isPlaying);

  const onTogglePlay = () => {
    if (busy) return;
    void (isPlaying ? pause() : resume()).then(() => {
      usePlaybackStore.getState().setIsPlaying(!isPlaying);
    });
  };

  const transportDisabled = !isPlaying ? false : busy;
  const canStep = queueLength > 0;

  return (
    <div className="flex w-full max-w-2xl flex-col items-center gap-1.5">
      <div className="flex items-center gap-1">
        <IconButton ariaLabel="Shuffle" disabled className="opacity-40">
          <HugeiconsIcon icon={ShuffleIcon} size={16} strokeWidth={1.75} />
        </IconButton>
        <IconButton
          ariaLabel="Previous track"
          onClick={onPrev}
          disabled={transportDisabled || !canStep}
        >
          <HugeiconsIcon icon={PreviousIcon} size={18} strokeWidth={1.75} />
        </IconButton>
        <button
          type="button"
          onClick={onTogglePlay}
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
          disabled={transportDisabled || !canStep}
        >
          <HugeiconsIcon icon={NextIcon} size={18} strokeWidth={1.75} />
        </IconButton>
        <IconButton ariaLabel="Repeat" disabled className="opacity-40">
          <HugeiconsIcon icon={RepeatIcon} size={16} strokeWidth={1.75} />
        </IconButton>
      </div>
      <SeekBar onCommit={onCommitSeek} disabled={busy} />
    </div>
  );
});

// ─── Seek bar — subscribes to positionSeconds directly so the rest
// of the transport doesn't repaint on every tick. ─────────────────

interface SeekBarProps {
  onCommit: (value: number) => void;
  disabled: boolean;
}

const SeekBar = memo(function SeekBar({ onCommit, disabled }: SeekBarProps) {
  const positionSeconds = usePlaybackStore((s) => s.positionSeconds);
  const durationSeconds = usePlaybackStore((s) => s.durationSeconds);
  const hasTrack = usePlaybackStore((s) => s.currentTrack !== null);

  const [drag, setDrag] = useState<number | null>(null);
  const dragRef = useRef<number | null>(null);

  const seekEnabled = hasTrack && durationSeconds > 0 && !disabled;
  const displayed = drag ?? positionSeconds;
  const progress = durationSeconds > 0 ? Math.min(100, (displayed / durationSeconds) * 100) : 0;

  return (
    <div className="flex w-full max-w-md items-center gap-2.5">
      <span className="w-10 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted-foreground">
        {formatDuration(displayed)}
      </span>
      <div className="relative h-3 flex-1">
        <svg
          className="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
          viewBox="0 0 100 12"
          preserveAspectRatio="none"
          aria-hidden
        >
          <path
            d={WAVE_PATH}
            fill="none"
            stroke="var(--muted)"
            strokeWidth={1}
            vectorEffect="non-scaling-stroke"
          />
          <path
            d={WAVE_PATH}
            fill="none"
            stroke="var(--primary)"
            strokeWidth={1.5}
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
          value={displayed}
          onChange={(e) => {
            const nextValue = Number(e.currentTarget.value);
            dragRef.current = nextValue;
            setDrag(nextValue);
          }}
          onPointerUp={() => {
            if (dragRef.current !== null) onCommit(dragRef.current);
            dragRef.current = null;
            setDrag(null);
          }}
          onKeyUp={(e) => {
            if (e.key === "Tab") return;
            if (dragRef.current !== null) onCommit(dragRef.current);
            dragRef.current = null;
            setDrag(null);
          }}
          onBlur={() => {
            if (dragRef.current !== null) onCommit(dragRef.current);
            dragRef.current = null;
            setDrag(null);
          }}
          disabled={!seekEnabled}
          aria-label="Seek"
          aria-valuemin={0}
          aria-valuemax={durationSeconds}
          aria-valuenow={displayed}
          className="player-range-wave absolute inset-0 h-full w-full cursor-pointer appearance-none bg-transparent outline-none disabled:cursor-not-allowed disabled:opacity-40"
        />
      </div>
      <span className="w-10 shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground">
        {formatDuration(durationSeconds)}
      </span>
    </div>
  );
});

// ─── RIGHT — volume + queue + EQ ───────────────────────────────────

interface VolumeControlsProps {
  onVolumeChange: (v: number) => void;
  onMuteToggle: () => void;
  queueOpen: boolean;
  onToggleQueue: () => void;
  eqOpen: boolean;
  onToggleEq: () => void;
}

const VolumeControls = memo(function VolumeControls({
  onVolumeChange,
  onMuteToggle,
  queueOpen,
  onToggleQueue,
  eqOpen,
  onToggleEq,
}: VolumeControlsProps) {
  const volume = usePlaybackStore((s) => s.volume);
  const muted = usePlaybackStore((s) => s.muted);
  const effectiveVolume = muted ? 0 : volume;

  return (
    <div className="flex min-w-0 flex-1 items-center justify-end gap-1">
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
          <VolumeIcon volume={effectiveVolume} muted={muted} />
        </button>
        <input
          type="range"
          min={VOLUME_MIN}
          max={VOLUME_MAX}
          step={VOLUME_STEP}
          value={effectiveVolume}
          onChange={(e) => onVolumeChange(Number(e.currentTarget.value))}
          aria-label="Volume"
          tabIndex={0}
          className="player-range h-1 w-0 cursor-pointer appearance-none rounded-full bg-muted opacity-0 outline-none accent-primary transition-[width,opacity,padding] duration-200 ease-out group-hover:w-24 group-hover:opacity-100 group-focus-within:w-24 group-focus-within:opacity-100 focus:w-24 focus:opacity-100"
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
        onClick={onToggleEq}
        aria-expanded={eqOpen}
        aria-pressed={eqOpen}
        active={eqOpen}
      >
        <HugeiconsIcon icon={SlidersHorizontalIcon} size={16} strokeWidth={1.75} />
      </IconButton>
    </div>
  );
});

function VolumeIcon({ volume, muted }: { volume: number; muted: boolean }) {
  if (muted || volume === 0) {
    return <HugeiconsIcon icon={VolumeOffIcon} size={16} strokeWidth={1.75} />;
  }
  if (volume < 0.5) {
    return <HugeiconsIcon icon={VolumeLowIcon} size={16} strokeWidth={1.75} />;
  }
  return <HugeiconsIcon icon={VolumeHighIcon} size={16} strokeWidth={1.75} />;
}

// 8 sine-like cycles stretched across the viewBox (preserveAspectRatio="none"
// scales the path to the live bar width). Each cycle is two quadratic-bezier
// segments of width 6.25 that smooth into a continuous wave.
const WAVE_PATH = (() => {
  let d = "M 0 6";
  for (let i = 0; i < 16; i += 1) d += " q 3.125 -6 6.25 0";
  return d;
})();

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
        disabled &&
          "cursor-not-allowed opacity-40 hover:bg-transparent hover:text-muted-foreground",
        className,
      )}
    >
      {children}
    </button>
  );
}
