// PlayerBar — bottom transport bar.
//
// Three sections (Spotify / Apple Music-style):
//   left:   cover + track title + artist
//   center: shuffle + prev + play/pause + next + repeat, with seek row
//   right:  volume + queue toggle + lyrics toggle + EQ toggle
//
// All state reads from the playback + queue stores, which are kept
// in sync by the global event bridge at the app root. Click handlers
// call the typed IPC wrappers directly; the resulting events update
// the stores for every other component.
//
// Seek + volume sliders commit on pointer-up / key-up / blur (not on
// every onChange) to avoid spamming the backend while the user drags.
// The pattern lives in a small `useDragCommit` hook below so seek
// and volume stay in lock-step.

import {
  LeftToRightListBulletIcon,
  SlidersHorizontalIcon,
  VolumeHighIcon,
  VolumeLowIcon,
  VolumeOffIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { memo, type ReactNode, useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { AlbumCover } from "@/components/ui/AlbumCover";
import { MaterialSymbol } from "@/components/ui/MaterialSymbol";
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

/** Per-action in-flight flag. Only the matching button is locked
 *  while its command is in flight — other actions stay live. */
type Busy = null | "play" | "prev" | "next";

type Props = {
  queueOpen: boolean;
  onToggleQueue: () => void;
  lyricsOpen: boolean;
  onToggleLyrics: () => void;
};

const renderCountRef = { current: 0 };

export function PlayerBar({ queueOpen, onToggleQueue, lyricsOpen, onToggleLyrics }: Props) {
  renderCountRef.current += 1;
  if (import.meta.env.DEV) {
    console.log(
      `[PlayerBar render #${renderCountRef.current}] isPlaying=${usePlaybackStore.getState().isPlaying} pos=${usePlaybackStore.getState().positionSeconds}`,
    );
  }
  // ── global state ──────────────────────────────────────────────
  const queueLength = useQueueStore((s) => s.entries.length);
  const tracks = useLibraryStore((s) => s.tracks);
  const albums = useLibraryStore((s) => s.albums);
  const currentTrack = usePlaybackStore((s) => s.currentTrack);
  const isPlaying = usePlaybackStore((s) => s.isPlaying);
  const position = usePlaybackStore((s) => s.positionSeconds);
  const duration = usePlaybackStore((s) => s.durationSeconds);
  const volume = usePlaybackStore((s) => s.volume);
  const muted = usePlaybackStore((s) => s.muted);

  // ── UI state ──────────────────────────────────────────────────
  const [eqOpen, setEqOpen] = useState(false);
  const [busy, setBusy] = useState<Busy>(null);

  const seekDrag = useDragCommit({ value: position });
  const volumeDrag = useDragCommit({ value: muted ? 0 : volume });

  // ── handlers ─────────────────────────────────────────────────
  const run = useCallback(
    async <T,>(action: Busy, fn: () => Promise<T>, label: string): Promise<void> => {
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
    [busy],
  );

  const onTogglePlay = () => {
    if (import.meta.env.DEV) console.log("[PlayerBar] onTogglePlay clicked, isPlaying=", isPlaying);
    run(
      "play",
      async () => {
        const next = !isPlaying;
        // Optimistic flip — the backend takes a few hundred ms to flip
        // the rodio sink. Showing the state immediately feels responsive.
        usePlaybackStore.getState().setIsPlaying(next);
        if (isPlaying) await pause();
        else await resume();
        if (import.meta.env.DEV) console.log("[PlayerBar] onTogglePlay done");
      },
      "Playback",
    );
  };

  const onPrev = () => {
    if (import.meta.env.DEV) console.log("[PlayerBar] onPrev clicked");
    run(
      "prev",
      async () => {
        usePlaybackStore.getState().setIsPlaying(false);
        await previous();
      },
      "Previous",
    );
  };

  const onNext = () => {
    if (import.meta.env.DEV) console.log("[PlayerBar] onNext clicked");
    run(
      "next",
      async () => {
        usePlaybackStore.getState().setIsPlaying(false);
        await next();
      },
      "Next",
    );
  };

  const seekEnabled = currentTrack !== null && duration > 0;

  const finishSeek = useCallback(() => {
    if (!seekEnabled) return;
    const drag = seekDrag.value;
    seekDrag.finish();
    if (drag === position) return;
    if (import.meta.env.DEV) console.log(`[PlayerBar] finishSeek to ${drag}`);
    run(
      null,
      async () => {
        await seek(drag);
        usePlaybackStore.getState().setPosition(drag);
        if (import.meta.env.DEV) console.log("[PlayerBar] finishSeek done");
      },
      "Seek",
    );
  }, [run, seekDrag, seekEnabled, position]);

  const finishVolume = useCallback(() => {
    const drag = volumeDrag.value;
    volumeDrag.finish();
    const committed = muted ? 0 : volume;
    if (drag === committed) return;
    if (import.meta.env.DEV) console.log(`[PlayerBar] finishVolume to ${drag}`);
    run(
      null,
      async () => {
        await setVolume(drag);
        usePlaybackStore.getState().setVolume(drag);
        if (import.meta.env.DEV) console.log("[PlayerBar] finishVolume done");
      },
      "Set volume",
    );
  }, [run, volumeDrag, muted, volume]);

  const onMuteToggle = () => {
    const nextMuted = !muted;
    if (import.meta.env.DEV) console.log("[PlayerBar] onMuteToggle", nextMuted);
    setMuted(nextMuted)
      .then(() => usePlaybackStore.getState().setMuted(nextMuted))
      .catch((err) => toast.error(`Toggle mute: ${extractError(err, "unknown error")}`));
  };

  // Esc closes the EQ popover.
  useEffect(() => {
    if (!eqOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setEqOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [eqOpen]);

  // ── derived data ──────────────────────────────────────────────
  const cover = useCurrentCover(albums, tracks, currentTrack);
  const hasTrack = currentTrack !== null;
  const canStep = queueLength > 0;
  const seekProgress = duration > 0 ? Math.min(100, (seekDrag.value / duration) * 100) : 0;
  const effectiveVolume = muted ? 0 : volume;
  const volumeProgress = Math.max(0, Math.min(1, volumeDrag.value)) * 100;
  const volumeIcon = volumeIconFor(effectiveVolume, muted);

  // ── drop target ───────────────────────────────────────────────
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

      {/* ── Left: now-playing ──────────────────────────────────── */}
      <div className="flex min-w-0 flex-1 items-center gap-3.5">
        <div className="h-14 w-14 shrink-0" aria-hidden={!cover}>
          {cover ? (
            <AlbumCover
              source={cover}
              ariaLabel={`Cover art for ${cover.title}`}
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

      {/* ── Center: transport + seek ──────────────────────────── */}
      <div className="flex w-full max-w-2xl flex-col items-center gap-1.5">
        <div className="flex items-center gap-1">
          <IconButton ariaLabel="Shuffle" disabled className="opacity-40">
            <MaterialSymbol name="shuffle" size={18} />
          </IconButton>
          <IconButton
            ariaLabel="Previous track"
            onClick={onPrev}
            disabled={!canStep || busy !== null}
          >
            <MaterialSymbol name="skip_previous" size={20} fill />
          </IconButton>
          <button
            type="button"
            onClick={onTogglePlay}
            disabled={busy === "play"}
            aria-label={isPlaying ? "Pause" : "Play"}
            title={isPlaying ? "Pause" : "Play"}
            className={cn(
              "group relative flex h-10 w-10 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-sm transition-all",
              "hover:scale-105 hover:shadow-md hover:shadow-primary/20 active:scale-95",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card",
              "disabled:opacity-40 disabled:hover:scale-100 disabled:hover:shadow-sm",
            )}
          >
            <MaterialSymbol
              name={isPlaying ? "pause" : "play_arrow"}
              size={22}
              fill
              className={isPlaying ? "" : "translate-x-[1px]"}
            />
          </button>
          <IconButton ariaLabel="Next track" onClick={onNext} disabled={!canStep || busy !== null}>
            <MaterialSymbol name="skip_next" size={20} fill />
          </IconButton>
          <IconButton ariaLabel="Repeat" disabled className="opacity-40">
            <MaterialSymbol name="repeat" size={18} />
          </IconButton>
        </div>

        {/* Seek bar */}
        <div className="flex w-full max-w-md items-center gap-2.5">
          <span className="w-10 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted-foreground">
            {formatDuration(seekDrag.value)}
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
                  clipPath: `inset(0 ${100 - seekProgress}% 0 0)`,
                  WebkitClipPath: `inset(0 ${100 - seekProgress}% 0 0)`,
                }}
              />
            </svg>
            <input
              type="range"
              min={0}
              max={duration || 0}
              step={1}
              value={seekDrag.value}
              onChange={seekDrag.onChange}
              onPointerUp={finishSeek}
              onKeyUp={(e) => {
                if (e.key === "Tab") return;
                finishSeek();
              }}
              onBlur={finishSeek}
              disabled={!seekEnabled || busy === "prev" || busy === "next" || busy === "play"}
              aria-label="Seek"
              aria-valuemin={0}
              aria-valuemax={duration}
              aria-valuenow={seekDrag.value}
              className="player-range-wave absolute inset-0 h-full w-full cursor-pointer appearance-none bg-transparent outline-none disabled:cursor-not-allowed disabled:opacity-40"
            />
          </div>
          <span className="w-10 shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground">
            {formatDuration(duration)}
          </span>
        </div>
      </div>

      {/* ── Right: volume + queue + lyrics + EQ ─────────────────── */}
      <div className="flex min-w-0 flex-1 items-center justify-end gap-1">
        {/* Volume group */}
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
            onPointerUp={finishVolume}
            onKeyUp={(e) => {
              if (e.key === "Tab") return;
              finishVolume();
            }}
            onBlur={finishVolume}
            aria-label="Volume"
            tabIndex={0}
            className="player-range h-1 w-0 cursor-pointer appearance-none rounded-full bg-muted opacity-0 outline-none accent-primary transition-[width,opacity,padding] duration-200 ease-out group-hover:w-24 group-hover:opacity-100 group-focus-within:w-24 group-focus-within:opacity-100 focus:w-24 focus:opacity-100"
            style={{
              background: `linear-gradient(to right, var(--primary) 0%, var(--primary) ${volumeProgress}%, var(--muted) ${volumeProgress}%, var(--muted) 100%)`,
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
          ariaLabel="Toggle lyrics"
          onClick={onToggleLyrics}
          aria-expanded={lyricsOpen}
          aria-pressed={lyricsOpen}
          active={lyricsOpen}
        >
          <MaterialSymbol name="lyrics" size={18} />
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

// ─── helpers ───────────────────────────────────────────────────────

/** Drag-then-commit pattern shared by the seek and volume sliders.
 *
 * The slider reports every pixel of movement via `onChange`; we store
 * the latest in state but only commit to the backend on
 * `pointerup` / `keyup` / `blur`. The hook exposes `value` (the
 * drag shadow or the upstream value), `onChange` (drag handler)
 * and `finish` (commit handler — call once on release).
 */
function useDragCommit({ value }: { value: number }) {
  const [drag, setDrag] = useState<number | null>(null);

  const onChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setDrag(Number(e.currentTarget.value));
  }, []);

  return {
    value: drag ?? value,
    onChange,
    finish: () => setDrag(null),
    setDrag,
  };
}

/** Get the cover art for the current track, falling back to the
 *  album's imageRef or a gradient placeholder. */
function useCurrentCover(
  albums: Album[],
  tracks: Track[],
  currentTrack: { trackId: string; title: string; artist: string; album: string } | null,
) {
  const albumById = new Map(albums.map((a) => [a.id, a]));
  const trackById = new Map(tracks.map((t) => [t.id, t]));
  if (!currentTrack) return null;
  const full = trackById.get(currentTrack.trackId);
  if (full?.imageRef) {
    return { id: full.id, title: full.album || full.title, imageRef: full.imageRef };
  }
  if (full) {
    const album = albumById.get(full.albumId);
    if (album?.imageRef) {
      return { id: full.id, title: album.title, imageRef: album.imageRef };
    }
  }
  return null;
}

function volumeIconFor(volume: number, muted: boolean) {
  if (muted || volume === 0) return VolumeOffIcon;
  if (volume < 0.5) return VolumeLowIcon;
  return VolumeHighIcon;
}

const WAVE_PATH = (() => {
  let d = "M 0 6";
  for (let i = 0; i < 16; i += 1) d += " q 3.125 -6 6.25 0";
  return d;
})();

type IconButtonProps = {
  ariaLabel: string;
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  active?: boolean;
  className?: string;
};

const IconButton = memo(function IconButton({
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
});
