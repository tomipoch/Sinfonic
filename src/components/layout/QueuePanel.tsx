// QueuePanel — slide-in panel from the right showing the playback
// queue. Opened via the queue button in PlayerBar. Overlaps the
// content area without pushing it (unlike the sidebar).
//
// Mode toggles between `queue` and `lyrics` via a header segmented
// control. Lyrics auto-scrolls following `positionSeconds` when the
// provider returns LRC-shaped synced lines; falls back to a
// vertically-scrollable plain-text view otherwise.

import { Delete03Icon, RepeatIcon, RepeatOne01Icon, ShuffleIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { MaterialSymbol } from "@/components/ui/MaterialSymbol";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { getLyrics, type LyricsPayload, queueClear, queueJumpTo, queueRemove } from "@/lib/tauri";
import { usePlaybackContext } from "@/playback";
import { repeatLabel } from "@/playback/repeat";
import { useQueueStore } from "@/stores/queueStore";

type Mode = "queue" | "lyrics";

interface Props {
  onClose: () => void;
  initialMode?: Mode;
}

export function QueuePanel({ onClose, initialMode = "queue" }: Props) {
  const [mode, setMode] = useState<Mode>(initialMode);

  useEffect(() => {
    setMode(initialMode);
  }, [initialMode]);

  return (
    <div className="absolute inset-y-0 right-0 z-40 flex w-56 flex-col border-l border-border bg-card shadow-xl">
      <PanelHeader mode={mode} onModeChange={setMode} onClose={onClose} />
      {mode === "queue" ? <QueueList /> : <LyricsView />}
    </div>
  );
}

interface PanelHeaderProps {
  mode: Mode;
  onModeChange: (next: Mode) => void;
  onClose: () => void;
}

function PanelHeader({ mode, onModeChange, onClose }: PanelHeaderProps) {
  return (
    <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
      <div>
        <h2 className="text-sm font-semibold text-foreground">
          {mode === "queue" ? "Queue" : "Lyrics"}
        </h2>
        <ModeSubtitle mode={mode} />
      </div>
      <div className="flex items-center gap-1">
        <SegmentedModeToggle mode={mode} onModeChange={onModeChange} />
        {mode === "queue" ? <QueueActions /> : null}
        <button
          type="button"
          onClick={onClose}
          aria-label="Close panel"
          className="ml-1 size-7 rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          ✕
        </button>
      </div>
    </div>
  );
}

function ModeSubtitle({ mode }: { mode: Mode }) {
  // Hooks must run unconditionally — subscribe to the store here and
  // render the right label based on `mode` afterwards.
  const entries = useQueueStore((s) => s.entries);
  if (mode === "queue") {
    return (
      <p className="text-xs text-muted-foreground">
        {entries.length === 0 ? "Empty" : `${entries.length} tracks`}
      </p>
    );
  }
  return <p className="text-xs text-muted-foreground">Now playing</p>;
}

function SegmentedModeToggle({
  mode,
  onModeChange,
}: {
  mode: Mode;
  onModeChange: (next: Mode) => void;
}) {
  return (
    <div
      role="tablist"
      aria-label="Panel mode"
      className="mr-1 inline-flex h-7 items-center rounded-md bg-muted/60 p-0.5 text-[11px]"
    >
      <button
        type="button"
        role="tab"
        aria-selected={mode === "queue"}
        onClick={() => onModeChange("queue")}
        className={cn(
          "inline-flex h-6 items-center gap-1 rounded px-2 transition-colors",
          mode === "queue"
            ? "bg-card text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground",
        )}
      >
        <MaterialSymbol name="queue_music" size={14} />
        Queue
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={mode === "lyrics"}
        onClick={() => onModeChange("lyrics")}
        className={cn(
          "inline-flex h-6 items-center gap-1 rounded px-2 transition-colors",
          mode === "lyrics"
            ? "bg-card text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground",
        )}
      >
        <MaterialSymbol name="lyrics" size={14} />
        Lyrics
      </button>
    </div>
  );
}

function QueueActions() {
  const entries = useQueueStore((s) => s.entries);
  const { snapshot, cycleRepeat, setShuffle } = usePlaybackContext();
  const { repeat, shuffle } = snapshot;
  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<unknown>, label: string) => {
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

  const onToggleRepeat = () => run(cycleRepeat, "Repeat");

  const onToggleShuffle = () =>
    run(async () => {
      await setShuffle(!shuffle);
    }, "Shuffle");

  const onClear = () => run(() => queueClear(), "Clear");

  return (
    <>
      <button
        type="button"
        onClick={onToggleShuffle}
        disabled={busy}
        aria-pressed={shuffle}
        aria-label="Toggle shuffle"
        className={cn(
          "size-7 rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
          shuffle && "bg-primary/20 text-primary",
        )}
      >
        <HugeiconsIcon icon={ShuffleIcon} size={16} strokeWidth={1.75} />
      </button>
      <button
        type="button"
        onClick={onToggleRepeat}
        disabled={busy}
        aria-label={`Repeat: ${repeatLabel(repeat, "short")}`}
        className={cn(
          "size-7 rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
          repeat !== "off" && "bg-primary/20 text-primary",
        )}
      >
        <HugeiconsIcon
          icon={repeat === "one" ? RepeatOne01Icon : RepeatIcon}
          size={16}
          strokeWidth={1.75}
        />
      </button>
      <button
        type="button"
        onClick={onClear}
        disabled={busy || entries.length === 0}
        aria-label="Clear queue"
        className="size-7 rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-30"
      >
        <HugeiconsIcon icon={Delete03Icon} size={16} strokeWidth={1.75} />
      </button>
    </>
  );
}

function QueueList() {
  const entries = useQueueStore((s) => s.entries);
  const currentIndex = useQueueStore((s) => s.currentIndex);
  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<unknown>, label: string) => {
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

  const onJumpTo = (entryId: string) => void run(() => queueJumpTo(entryId), "Jump");
  const onRemove = (entryId: string) =>
    void run(async () => {
      const removed = await queueRemove(entryId);
      if (!removed) toast("Entry not found");
    }, "Remove");

  if (entries.length === 0) {
    return (
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="flex flex-col items-center justify-center p-6 text-center">
          <p className="text-sm text-muted-foreground">Nothing in queue</p>
          <p className="mt-1 text-xs text-muted-foreground">Play a track to add it here</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <ol className="divide-y divide-border">
        {entries.map((entry, index) => {
          const isCurrent = index === currentIndex;
          return (
            <li
              key={entry.id}
              className={cn(
                "flex items-center gap-2 px-3 py-2 text-sm",
                isCurrent ? "bg-muted" : "hover:bg-muted/50",
              )}
            >
              <span className="w-6 shrink-0 text-right font-mono text-xs text-muted-foreground">
                {isCurrent ? "▶" : index + 1}
              </span>
              <button
                type="button"
                onClick={() => void onJumpTo(entry.id)}
                className="min-w-0 flex-1 text-left"
              >
                <div
                  className={cn(
                    "truncate font-medium",
                    isCurrent ? "text-primary" : "text-foreground",
                  )}
                >
                  {entry.title}
                </div>
                <div className="truncate text-xs text-muted-foreground">{entry.artist}</div>
              </button>
              <span className="shrink-0 font-mono text-xs text-muted">
                {formatDuration(entry.durationSeconds)}
              </span>
              <button
                type="button"
                onClick={() => void onRemove(entry.id)}
                disabled={busy}
                aria-label={`Remove ${entry.title}`}
                className="size-5 rounded p-0.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-30"
              >
                ✕
              </button>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

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
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center p-6 text-center">
        <MaterialSymbol name="music_off" size={28} className="text-muted-foreground/70" />
        <p className="mt-2 text-sm text-muted-foreground">Nothing playing</p>
      </div>
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
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1 p-6 text-center">
        <MaterialSymbol name="lyrics" size={28} className="text-muted-foreground/70" />
        <p className="text-sm text-muted-foreground">No lyrics for this track</p>
        <p className="text-xs text-muted-foreground">
          {currentTrack.title} — {currentTrack.artist}
        </p>
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1 p-6 text-center">
        <MaterialSymbol name="error" size={28} className="text-destructive" />
        <p className="text-sm text-foreground">Couldn't load lyrics</p>
        <p className="text-xs text-muted-foreground">{state.message}</p>
      </div>
    );
  }

  return <LyricsBody lyrics={state.lyrics} positionSeconds={positionSeconds} />;
}

function LyricsBody({
  lyrics,
  positionSeconds,
}: {
  lyrics: LyricsPayload;
  positionSeconds: number;
}) {
  const syncedLines = useMemo(
    () => (lyrics.synced ? parseLrc(lyrics.synced) : null),
    [lyrics.synced],
  );
  const containerRef = useRef<HTMLDivElement>(null);
  const activeIndex = useMemo(() => {
    if (!syncedLines) return -1;
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
    const child = containerRef.current.children[activeIndex] as HTMLElement | undefined;
    child?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [activeIndex]);

  if (syncedLines && syncedLines.length > 0) {
    return (
      <div
        ref={containerRef}
        className="min-h-0 flex-1 overflow-y-auto px-6 py-10 text-center text-base leading-relaxed"
      >
        {syncedLines.map((line, idx) => (
          <p
            key={`${line.timeMs}-${idx}`}
            className={cn(
              "transition-colors duration-300",
              idx === activeIndex
                ? "text-foreground"
                : idx < activeIndex
                  ? "text-muted-foreground/60"
                  : "text-muted-foreground/80",
            )}
          >
            {line.text || "♪"}
          </p>
        ))}
      </div>
    );
  }

  const plain = lyrics.plain ?? "";
  const paragraphs = plain
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-6 py-8 text-center">
      {paragraphs.length === 0 ? (
        <p className="text-sm text-muted-foreground">No lyrics available.</p>
      ) : (
        paragraphs.map((p, idx) => (
          <p
            key={idx}
            className="mb-4 whitespace-pre-line text-sm leading-relaxed text-foreground/90"
          >
            {p}
          </p>
        ))
      )}
    </div>
  );
}
