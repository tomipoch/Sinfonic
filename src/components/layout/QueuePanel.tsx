// QueuePanel — slide-in panel from the right.
//
// The panel itself owns no header / no title / no mode toggle:
// the mode (queue vs lyrics) is controlled by the PanelToggles in
// the PlayerBar so the header here would duplicate that choice
// against the sidebar's NowPlaying.
//
// When mode === "queue" the panel shows:
//   1. A row of two full-width buttons:
//        - "Seguir reproduciendo" (accent colour) calls next() so
//          the queue advances; visually the primary action of the
//          panel.
//        - "Crossfade" toggles a local crossfade preference (no
//          backend support yet — wired up so the UI lives; the
//          audio engine still crossfades per its own configuration).
//   2. Two sections, each with a clear button next to the title:
//        - "Historial" — entries[0..currentIndex] (already played)
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
import { getLyrics, type LyricsPayload, next, queueClear, queueRemove } from "@/lib/tauri";
import { usePlaybackContext } from "@/playback";
import { useQueueStore } from "@/stores/queueStore";

type Mode = "queue" | "lyrics";

interface Props {
  /** Optional — the panel can be closed via the queue/lyrics
   *  toggles in the PlayerBar; an explicit `onClose` is kept for
   *  future callers that want to drive the close from the panel. */
  onClose?: () => void;
  initialMode?: Mode;
}

export function QueuePanel({ initialMode = "queue" }: Props) {
  const mode = useQueueStore((s) => s.panelMode ?? initialMode);

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
  const [busy, setBusy] = useState(false);
  const [crossfade, setCrossfade] = useState(false);

  const historyEntries = useMemo(
    () => (currentIndex === null ? [] : entries.slice(0, Math.min(currentIndex, entries.length))),
    [entries, currentIndex],
  );
  const upcomingEntries = useMemo(
    () => (currentIndex === null ? entries : entries.slice(currentIndex + 1)),
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

  const onPlayNext = () => run(() => next(), "Skip to next");
  const onClearHistory = () =>
    run(async () => {
      if (historyEntries.length === 0) return;
      await Promise.all(historyEntries.map((entry) => queueRemove(entry.id)));
    }, "Clear history");
  const onClearUpcoming = () =>
    run(async () => {
      if (upcomingEntries.length === 0) return;
      await Promise.all(upcomingEntries.map((entry) => queueRemove(entry.id)));
    }, "Clear play next");
  const onClearAll = () => run(() => queueClear(), "Clear queue");

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Two full-width action buttons */}
      <div className="flex shrink-0 flex-col gap-1.5 border-b border-border p-2">
        <button
          type="button"
          onClick={onPlayNext}
          disabled={busy || upcomingEntries.length === 0}
          className="flex h-9 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-40"
        >
          <MaterialSymbol name="play_arrow" size={18} weight={700} fill />
          Seguir reproduciendo
        </button>
        <button
          type="button"
          onClick={() => setCrossfade((v) => !v)}
          aria-pressed={crossfade}
          className={cn(
            "flex h-8 w-full items-center justify-center gap-2 rounded-md border border-border text-sm font-medium transition-colors",
            crossfade
              ? "bg-primary/15 text-primary border-primary/40"
              : "bg-background text-foreground hover:bg-muted",
          )}
        >
          <MaterialSymbol name="graphic_eq" size={16} />
          Crossfade
        </button>
      </div>

      {/* Scrollable sections */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        <Section
          title="Historial"
          count={historyEntries.length}
          onClear={onClearHistory}
          clearDisabled={busy || historyEntries.length === 0}
        >
          {historyEntries.length === 0 ? (
            <p className="px-3 py-2 text-xs text-muted-foreground">
              Nothing played yet in this session.
            </p>
          ) : (
            <ol className="divide-y divide-border">
              {historyEntries.map((entry, index) => (
                <QueueRow
                  key={entry.id}
                  title={entry.title}
                  artist={entry.artist}
                  duration={formatDuration(entry.durationSeconds)}
                  index={index + 1}
                  isCurrent={false}
                />
              ))}
            </ol>
          )}
        </Section>

        <Section
          title="Seguir reproduciendo"
          count={upcomingEntries.length}
          onClear={onClearUpcoming}
          clearDisabled={busy || upcomingEntries.length === 0}
        >
          {upcomingEntries.length === 0 ? (
            <p className="px-3 py-2 text-xs text-muted-foreground">
              Queue is empty — the next track will start when the current one ends.
            </p>
          ) : (
            <ol className="divide-y divide-border">
              {upcomingEntries.map((entry, index) => {
                const absoluteIndex = currentIndex === null ? index : currentIndex + 1 + index;
                return (
                  <QueueRow
                    key={entry.id}
                    title={entry.title}
                    artist={entry.artist}
                    duration={formatDuration(entry.durationSeconds)}
                    index={absoluteIndex + 1}
                    isCurrent={false}
                  />
                );
              })}
            </ol>
          )}
        </Section>

        {entries.length > 0 && (
          <div className="border-t border-border p-2">
            <button
              type="button"
              onClick={onClearAll}
              disabled={busy}
              className="w-full rounded-md px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"
            >
              Clear entire queue
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

interface SectionProps {
  title: string;
  count: number;
  onClear: () => void;
  clearDisabled: boolean;
  children: React.ReactNode;
}

function Section({ title, count, onClear, clearDisabled, children }: SectionProps) {
  return (
    <section className="border-b border-border py-2 last:border-b-0">
      <header className="flex items-center justify-between px-3 pb-1">
        <h3 className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          {title}
          {count > 0 ? <span className="ml-1 text-muted-foreground/60">· {count}</span> : null}
        </h3>
        <button
          type="button"
          onClick={onClear}
          disabled={clearDisabled}
          aria-label={`Clear ${title}`}
          className="size-6 rounded p-0.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-30"
        >
          <MaterialSymbol name="delete" size={14} />
        </button>
      </header>
      {children}
    </section>
  );
}

interface QueueRowProps {
  title: string;
  artist: string;
  duration: string;
  index: number;
  isCurrent: boolean;
}

function QueueRow({ title, artist, duration, index, isCurrent }: QueueRowProps) {
  return (
    <li className="flex items-center gap-2 px-3 py-2 text-sm">
      <span className="w-5 shrink-0 text-right font-mono text-[11px] text-muted-foreground">
        {index}
      </span>
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
