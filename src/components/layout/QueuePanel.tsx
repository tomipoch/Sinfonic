// QueuePanel — slide-in panel from the right.
//
// The panel itself owns no header / no title / no mode toggle:
// the mode (queue vs lyrics) is controlled by the PanelToggles in
// the PlayerBar so the header here would duplicate that choice
// against the sidebar's NowPlaying.
//
// When mode === "queue" the panel shows:
//   1. A row of two full-width buttons:
//        - "Resume" (accent colour) calls resume() so a paused
//          queue continues; disabled when already playing.
//        - "Crossfade" opens the Settings window at the Playback
//          tab (where the crossfade toggle + slider live).
//   2. Three sections, each with a clear button next to the title:
//        - "Now playing" — the entry at currentIndex (if any),
//          highlighted with the primary colour and a "▶" indicator.
//        - "History" — entries[0..currentIndex] (already played)
//        - "Seguir reproduciendo" — entries[currentIndex+1..]
//          (upcoming). Each section's clear button removes its
//          entries via a Promise.all of queueRemove calls (no batch
//          command exists in the backend yet).
//
// When mode === "lyrics" the panel shows the synced lyrics in an
// Apple Music-ish layout: track title + artist at the top, the
// current line large in primary colour, the rest dimmed, with
// smooth auto-scroll keeping the active line centred.
//
// On either mode an empty state explains what would appear (now
// playing / queue empty / no lyrics for this track).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { MaterialSymbol } from "@/components/ui/MaterialSymbol";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import {
  getLyrics,
  type LyricsPayload,
  queueExtendMore,
  queueRemove,
  resume,
  stop,
} from "@/lib/tauri";
import { openSettingsWindow } from "@/modules/preferences/openSettingsWindow";
import { usePlaybackContext } from "@/playback";
import { useQueueStore } from "@/stores/queueStore";
import { useServerStore } from "@/stores/serverStore";

type Mode = "queue" | "lyrics";

interface Props {
  /** Optional — the panel can be closed via the queue/lyrics
   *  toggles in the PlayerBar; an explicit `onClose` is kept for
   *  future callers that want to drive the close from the panel. */
  onClose?: () => void;
}

export function QueuePanel(_: Props = {}) {
  const storedMode = useQueueStore((s) => s.panelMode);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const { snapshot } = usePlaybackContext();
  const hasTrack = snapshot.currentTrack !== null;

  // Default to Up next when there's something to show; otherwise
  // fall back to Lyrics, which has a more informative empty state
  // ("Nothing playing") than two empty queue sections. The user's
  // explicit toggle is preserved via `storedMode`.
  const defaultMode: Mode = activeServerId !== null && hasTrack ? "queue" : "lyrics";
  const mode = storedMode ?? defaultMode;

  return (
    <div className="absolute inset-y-0 right-0 z-40 flex w-56 flex-col border-l border-border bg-card shadow-xl">
      {mode === "queue" ? <QueueView /> : <LyricsView />}
    </div>
  );
}

// ─── Queue view ─────────────────────────────────────────────────────

function QueueView() {
  const entries = useQueueStore((s) => s.entries);
  const currentIndex = useQueueStore((s) => s.currentIndex);
  const contextRemaining = useQueueStore((s) => s.contextRemaining);
  const { snapshot } = usePlaybackContext();
  const isPlaying = snapshot.isPlaying;
  const [busy, setBusy] = useState(false);
  // Tracks whether the user has scrolled the panel up — when
  // `true`, the "History" section above the visible viewport is
  // partially in view and we draw a subtle top shadow so they know
  // they can keep scrolling. Apple Music does the same.
  const [canScrollUp, setCanScrollUp] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const upNextRef = useRef<HTMLDivElement>(null);

  // Pin the scroll position to the start of the Up next
  // container on mount so the first row of Up next is visible at
  // the top of the viewport. History (which sits above Up next in
  // the DOM) is scrolled out of view. Both sections have sticky
  // headers, so scrolling up reveals the History header pinned at
  // top; scrolling back down past Up next lets the Up next header
  // take over.
  useEffect(() => {
    const el = scrollRef.current;
    const upNext = upNextRef.current;
    if (!el || !upNext) return;
    el.scrollTop = upNext.offsetTop;
    setCanScrollUp(el.scrollTop > 4);
    // intentionally empty deps: run once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const historyEntries = useMemo(
    () => (currentIndex === null ? [] : entries.slice(0, Math.min(currentIndex, entries.length))),
    [entries, currentIndex],
  );
  // Up next = current track (if any) + remaining entries. When
  // currentIndex is null the whole queue is "next" but no row is
  // highlighted as current. The current track (when there is one)
  // is always the first row, with the ▶ indicator + accent
  // colour from QueueRow's `isCurrent` prop.
  const nextEntries = useMemo(
    () => (currentIndex === null ? entries : entries.slice(currentIndex)),
    [entries, currentIndex],
  );

  const run = useCallback(
    async (fn: () => Promise<unknown>, label: string) => {
      if (busy) return;
      setBusy(true);
      try {
        await fn();
      } catch (err) {
        toast.error(`${label}: ${extractError(err, "unknown error")}`);
      } finally {
        setBusy(false);
      }
    },
    [busy],
  );

  const onResume = () => run(() => resume(), "Resume");
  const onClearHistory = () =>
    run(async () => {
      if (historyEntries.length === 0) return;
      await Promise.all(historyEntries.map((entry) => queueRemove(entry.id)));
    }, "Clear history");
  // Apple Music-style "Clear" for Up Next: stops playback, then
  // removes every entry from the END so the engine's current_index
  // stays valid until we reach it. After the last removal the
  // engine clamps current_index to the new last entry (or null),
  // which preserves history (entries before the old current).
  const onClearUpNext = () =>
    run(async () => {
      if (nextEntries.length === 0) return;
      await stop();
      for (const id of nextEntries.map((e) => e.id).reverse()) {
        await queueRemove(id);
      }
    }, "Clear up next");
  const onExtendMore = (n: number) =>
    run(async () => {
      if (n <= 0) return;
      const added = await queueExtendMore(n);
      if (added > 0) {
        toast.success(`Added ${added} more ${added === 1 ? "track" : "tracks"}`);
      } else {
        toast.info("No more tracks available from this source");
      }
    }, "Load more");

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Two full-width action buttons */}
      <div className="flex shrink-0 flex-col gap-1.5 border-b border-border p-2">
        <button
          type="button"
          onClick={onResume}
          disabled={busy || isPlaying || entries.length === 0}
          aria-label="Resume playback"
          className="flex h-9 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-40"
        >
          <MaterialSymbol name="play_arrow" size={18} weight={700} fill />
          Resume
        </button>
        <button
          type="button"
          onClick={() => {
            void openSettingsWindow("playback");
          }}
          className="flex h-8 w-full items-center justify-center gap-2 rounded-md border border-border text-sm font-medium bg-background text-foreground transition-colors hover:bg-muted"
        >
          <MaterialSymbol name="graphic_eq" size={16} />
          Crossfade
        </button>
      </div>

      {/* Scrollable sections — DOM order is History (hidden above
          the viewport by the bottom-pin scroll), then Up next.
          Pinning to scrollHeight on mount puts the user at the
          bottom; scrolling up reveals History. */}
      <div
        ref={scrollRef}
        onScroll={() => {
          const el = scrollRef.current;
          if (!el) return;
          setCanScrollUp(el.scrollTop > 4);
        }}
        className="relative min-h-0 flex-1 overflow-y-auto"
      >
        {/* Top shadow hints that there's hidden content above when
            the user has scrolled into the History section. */}
        <div
          aria-hidden
          className={cn(
            "pointer-events-none sticky top-0 z-10 h-3 -mb-3 bg-gradient-to-b from-card to-transparent transition-opacity duration-200",
            canScrollUp ? "opacity-100" : "opacity-0",
          )}
        />

        <div>
          <header className="sticky top-0 z-10 flex items-center justify-between border-b border-border bg-card/95 px-3 py-2 backdrop-blur-sm">
            <h3 className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              History
              {historyEntries.length > 0 ? (
                <span className="ml-1 text-muted-foreground/60">· {historyEntries.length}</span>
              ) : null}
            </h3>
            <button
              type="button"
              onClick={onClearHistory}
              disabled={busy || historyEntries.length === 0}
              aria-label="Clear History"
              className="size-6 rounded p-0.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-30"
            >
              <MaterialSymbol name="delete" size={14} />
            </button>
          </header>

          {historyEntries.length === 0 ? (
            <p className="px-3 py-2 text-xs text-muted-foreground">
              Nothing played yet in this session.
            </p>
          ) : (
            <ol className="divide-y divide-border">
              {historyEntries.map((entry) => (
                <QueueRow
                  key={entry.id}
                  title={entry.title}
                  artist={entry.artist}
                  duration={formatDuration(entry.durationSeconds)}
                />
              ))}
            </ol>
          )}
        </div>

        <div ref={upNextRef}>
          <header className="sticky top-0 z-10 flex items-center justify-between border-b border-border bg-card/95 px-3 py-2 backdrop-blur-sm">
            <h3 className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              Up next
              {nextEntries.length > 0 ? (
                <span className="ml-1 text-muted-foreground/60">· {nextEntries.length}</span>
              ) : null}
            </h3>
            <button
              type="button"
              onClick={onClearUpNext}
              disabled={busy || nextEntries.length === 0}
              aria-label="Clear Up next"
              className="size-6 rounded p-0.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-30"
            >
              <MaterialSymbol name="delete" size={14} />
            </button>
          </header>

          {nextEntries.length === 0 ? (
            <p className="px-3 py-2 text-xs text-muted-foreground">
              Queue is empty — the next track will start when the current one ends.
            </p>
          ) : (
            <ol className="divide-y divide-border">
              {nextEntries.map((entry, index) => {
                // The first row is the current track when there is
                // a current track. The accent colour, background
                // tint, and left border on QueueRow's `isCurrent`
                // styling make it obvious without a position
                // indicator.
                const isCurrent = currentIndex !== null && index === 0;
                return (
                  <QueueRow
                    key={entry.id}
                    title={entry.title}
                    artist={entry.artist}
                    duration={formatDuration(entry.durationSeconds)}
                    isCurrent={isCurrent}
                  />
                );
              })}
            </ol>
          )}
          {contextRemaining !== null && contextRemaining > 0 ? (
            <button
              type="button"
              onClick={() => onExtendMore(contextRemaining)}
              disabled={busy}
              aria-label={`Load ${contextRemaining} more tracks from this source`}
              className="mx-3 mb-2 mt-1 inline-flex items-center justify-center gap-1.5 rounded-md border border-border bg-muted/40 px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted hover:text-primary disabled:opacity-40"
            >
              <MaterialSymbol name="add" size={14} weight={700} />
              {contextRemaining} more from this source
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

interface QueueRowProps {
  title: string;
  artist: string;
  duration: string;
  /** When true, renders the row with the primary colour, a tinted
   *  background, and a left accent border — the visual cue that
   *  this is the currently-playing track (Apple Music style). */
  isCurrent?: boolean;
}

function QueueRow({ title, artist, duration, isCurrent = false }: QueueRowProps) {
  return (
    <li
      className={cn(
        "flex items-center gap-2 px-3 py-2 text-sm",
        isCurrent && "bg-primary/10 border-l-2 border-primary",
      )}
    >
      <div className="min-w-0 flex-1">
        <div
          className={cn(
            "truncate text-sm",
            isCurrent ? "font-semibold text-primary" : "text-foreground",
          )}
        >
          {title}
        </div>
        <div className="truncate text-[11px] text-muted-foreground">{artist}</div>
      </div>
      <span className="shrink-0 font-mono text-[11px] text-muted-foreground">{duration}</span>
    </li>
  );
}

// ─── Lyrics view ────────────────────────────────────────────────────

interface SyncedLine {
  timeMs: number;
  text: string;
}

const LRC_TIME_RE = /\[(\d{1,2}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

function parseLrc(input: string): SyncedLine[] | null {
  const lines = input.split(/\r?\n/);
  const parsed: SyncedLine[] = [];
  let sawTimestamp = false;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.length === 0) continue;
    LRC_TIME_RE.lastIndex = 0;
    const matches: number[] = [];
    let m: RegExpExecArray | null = LRC_TIME_RE.exec(line);
    while (m !== null) {
      sawTimestamp = true;
      const min = Number(m[1]);
      const sec = Number(m[2]);
      const fracRaw = m[3] ?? "0";
      const frac = Number(fracRaw.padEnd(3, "0").slice(0, 3));
      matches.push(min * 60_000 + sec * 1000 + frac);
      m = LRC_TIME_RE.exec(line);
    }
    const text = line.replace(LRC_TIME_RE, "").trim();
    for (const ms of matches) {
      parsed.push({ timeMs: ms, text });
    }
  }
  if (!sawTimestamp) return null;
  parsed.sort((a, b) => a.timeMs - b.timeMs);
  return parsed;
}

function LyricsView() {
  const { snapshot } = usePlaybackContext();
  const { currentTrack, positionSeconds } = snapshot;
  const trackId = currentTrack?.trackId ?? null;

  const [state, setState] = useState<
    | { kind: "loading" }
    | { kind: "ready"; lyrics: LyricsPayload }
    | { kind: "empty" }
    | { kind: "error"; message: string }
  >({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    if (!trackId) {
      setState({ kind: "empty" });
      return;
    }
    setState({ kind: "loading" });
    getLyrics(trackId, true)
      .then((lyrics) => {
        if (cancelled) return;
        if (!lyrics || (!lyrics.plain && !lyrics.synced)) {
          setState({ kind: "empty" });
        } else {
          setState({ kind: "ready", lyrics });
        }
      })
      .catch((err) => {
        if (cancelled) return;
        setState({
          kind: "error",
          message: extractError(err, "Couldn't load lyrics"),
        });
      });
    return () => {
      cancelled = true;
    };
  }, [trackId]);

  if (!currentTrack) {
    return (
      <LyricsEmpty
        icon={<MaterialSymbol name="music_off" size={28} className="text-muted-foreground/70" />}
        title="Nothing playing"
      />
    );
  }

  if (state.kind === "loading") {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center p-6 text-sm text-muted-foreground">
        Loading lyrics…
      </div>
    );
  }

  if (state.kind === "empty") {
    return (
      <LyricsEmpty
        icon={<MaterialSymbol name="lyrics" size={28} className="text-muted-foreground/70" />}
        title="No lyrics for this track"
        subtitle={`${currentTrack.title} — ${currentTrack.artist}`}
      />
    );
  }

  if (state.kind === "error") {
    return (
      <LyricsEmpty
        icon={<MaterialSymbol name="error" size={28} className="text-destructive" />}
        title="Couldn't load lyrics"
        subtitle={state.message}
      />
    );
  }

  return (
    <LyricsBody
      lyrics={state.lyrics}
      positionSeconds={positionSeconds}
      title={currentTrack.title}
      artist={currentTrack.artist}
    />
  );
}

interface LyricsEmptyProps {
  icon: React.ReactNode;
  title: string;
  subtitle?: string;
}

function LyricsEmpty({ icon, title, subtitle }: LyricsEmptyProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1 p-6 text-center">
      {icon}
      <p className="mt-2 text-sm text-foreground">{title}</p>
      {subtitle ? <p className="text-xs text-muted-foreground">{subtitle}</p> : null}
    </div>
  );
}

interface LyricsBodyProps {
  lyrics: LyricsPayload;
  positionSeconds: number;
  title: string;
  artist: string;
}

function LyricsBody({ lyrics, positionSeconds, title, artist }: LyricsBodyProps) {
  const syncedLines = useMemo(
    () => (lyrics.synced ? parseLrc(lyrics.synced) : null),
    [lyrics.synced],
  );
  const containerRef = useRef<HTMLDivElement>(null);
  const activeIndex = useMemo(() => {
    if (!syncedLines || syncedLines.length === 0) return -1;
    const ms = positionSeconds * 1000;
    let lo = 0;
    let hi = syncedLines.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (syncedLines[mid]!.timeMs <= ms) {
        found = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return found;
  }, [syncedLines, positionSeconds]);

  useEffect(() => {
    if (!containerRef.current || activeIndex < 0) return;
    const child = containerRef.current.children.item(activeIndex + 2) as HTMLElement | undefined;
    child?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [activeIndex]);

  if (syncedLines && syncedLines.length > 0) {
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <LyricsHeader title={title} artist={artist} />
        <div ref={containerRef} className="min-h-0 flex-1 overflow-y-auto px-4 py-6 text-center">
          {syncedLines.map((line, idx) => (
            <p
              key={`${line.timeMs}-${idx}`}
              className={cn(
                "py-1.5 transition-colors duration-300",
                idx === activeIndex
                  ? "text-base font-semibold text-primary"
                  : idx < activeIndex
                    ? "text-sm text-muted-foreground/50"
                    : "text-sm text-muted-foreground",
              )}
            >
              {line.text || "♪"}
            </p>
          ))}
        </div>
      </div>
    );
  }

  const plain = lyrics.plain ?? "";
  const paragraphs = plain
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <LyricsHeader title={title} artist={artist} />
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-6 text-center">
        {paragraphs.length === 0 ? (
          <p className="text-sm text-muted-foreground">No lyrics available.</p>
        ) : (
          paragraphs.map((p, idx) => (
            <p
              key={idx}
              className="mb-3 whitespace-pre-line text-sm leading-relaxed text-foreground/90"
            >
              {p}
            </p>
          ))
        )}
      </div>
    </div>
  );
}

function LyricsHeader({ title, artist }: { title: string; artist: string }) {
  return (
    <header className="shrink-0 border-b border-border px-3 py-2">
      <p className="truncate text-sm font-semibold text-foreground" title={title}>
        {title}
      </p>
      <p className="truncate text-[11px] text-muted-foreground" title={artist}>
        {artist}
      </p>
    </header>
  );
}
