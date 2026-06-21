// PlayerBar — fixed bottom bar. Three sections:
//   left: cover + track title + artist
//   center: transport controls + seek slider + position/total
//   right: mute toggle + volume slider
//
// All state reads from the playback + queue stores, which are kept
// in sync by the global event bridge at the app root. Click
// handlers call the typed IPC wrappers directly; the resulting
// events update the stores for every other component.
//
// Seek slider commits on pointer-up / key-up / blur (not on every
// onChange) to avoid spamming the backend while the user drags.

import { useRef, useState } from "react";
import { toast } from "sonner";

import {
  next,
  pause,
  previous,
  resume,
  seek,
  setMuted,
  setVolume,
} from "../../lib/tauri";
import { usePlaybackStore } from "../../stores/playbackStore";
import { useQueueStore } from "../../stores/queueStore";
import { cn } from "../../lib/cn";
import { formatDuration } from "../../lib/format";

const VOLUME_STEP = 0.05;
const VOLUME_MIN = 0;
const VOLUME_MAX = 1;

function PlayIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor" aria-hidden>
      <path d="M8 5v14l11-7z" />
    </svg>
  );
}

function PauseIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor" aria-hidden>
      <rect x="6" y="5" width="4" height="14" rx="1" />
      <rect x="14" y="5" width="4" height="14" rx="1" />
    </svg>
  );
}

function PrevIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor" aria-hidden>
      <path d="M6 6h2v12H6zM9.5 12l8.5 6V6z" />
    </svg>
  );
}

function NextIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor" aria-hidden>
      <path d="M16 6h2v12h-2zM14 6L5.5 12 14 18z" />
    </svg>
  );
}

export function PlayerBar() {
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

  return (
    <footer
      className="flex h-20 shrink-0 items-center justify-between gap-4 border-t border-bg-raised bg-bg-subtle px-4"
      role="contentinfo"
      aria-label="Player controls"
    >
      <div className="flex min-w-0 items-center gap-3">
        <div
          className="flex h-12 w-12 shrink-0 items-center justify-center rounded-md bg-bg-raised text-lg font-bold text-white/80"
          aria-hidden
        >
          {currentTrack?.title?.trim().charAt(0).toUpperCase() ?? "♪"}
        </div>
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-fg">
            {currentTrack?.title ?? "Nothing playing"}
          </div>
          <div className="truncate text-xs text-fg-subtle">
            {currentTrack?.artist ?? "—"}
          </div>
        </div>
      </div>

      <div className="flex min-w-0 flex-1 max-w-xl flex-col items-center gap-1">
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onPrev}
            disabled={transportDisabled || queueLength === 0}
            aria-label="Previous track"
            className="rounded-full p-2 text-fg-subtle hover:bg-bg-raised hover:text-fg focus:outline-none disabled:opacity-40"
          >
            <PrevIcon className="h-5 w-5" />
          </button>
          <button
            type="button"
            onClick={onTogglePlay}
            disabled={transportDisabled}
            aria-label={isPlaying ? "Pause" : "Play"}
            className="rounded-full bg-fg p-2 text-bg hover:bg-white focus:outline-none disabled:opacity-40"
          >
            {isPlaying ? (
              <PauseIcon className="h-5 w-5" />
            ) : (
              <PlayIcon className="h-5 w-5" />
            )}
          </button>
          <button
            type="button"
            onClick={onNext}
            disabled={transportDisabled || queueLength === 0}
            aria-label="Next track"
            className="rounded-full p-2 text-fg-subtle hover:bg-bg-raised hover:text-fg focus:outline-none disabled:opacity-40"
          >
            <NextIcon className="h-5 w-5" />
          </button>
        </div>
        <div className="flex w-full items-center gap-2">
          <span className="w-10 shrink-0 text-right font-mono text-xs text-fg-muted">
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
            className="h-1 flex-1 cursor-pointer accent-accent disabled:cursor-not-allowed disabled:opacity-40"
          />
          <span className="w-10 shrink-0 font-mono text-xs text-fg-muted">
            {formatDuration(durationSeconds)}
          </span>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onMuteToggle}
          aria-label={muted ? "Unmute" : "Mute"}
          aria-pressed={muted}
          className={cn(
            "rounded-md px-2 py-1 text-xs",
            muted
              ? "bg-accent/20 text-accent"
              : "text-fg-subtle hover:bg-bg-raised hover:text-fg",
          )}
        >
          {muted ? "Muted" : "Mute"}
        </button>
        <input
          type="range"
          min={VOLUME_MIN}
          max={VOLUME_MAX}
          step={VOLUME_STEP}
          value={effectiveVolume}
          onChange={(e) => void onVolumeChange(Number(e.currentTarget.value))}
          aria-label="Volume"
          className="h-1 w-28 cursor-pointer accent-accent"
        />
      </div>
    </footer>
  );
}
