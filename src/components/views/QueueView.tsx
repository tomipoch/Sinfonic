// QueueView — list of queue entries read from `useQueueStore`,
// kept in sync by the global playback event bridge at the app root.
//
// Per-entry actions:
//   - click row → jump_to (skip playback to that entry)
//   - remove button → queue_remove
//
// Header actions: shuffle + repeat toggles (write-through to backend
// + store), clear-queue.

import { useState } from "react";
import { toast } from "sonner";

import {
  queueClear,
  queueJumpTo,
  queueRemove,
  setRepeat,
  setShuffle,
} from "../../lib/tauri";
import { useQueueStore } from "../../stores/queueStore";
import { formatDuration } from "../../lib/format";
import type { RepeatMode } from "../../types/domain";

const REPEAT_CYCLE: ReadonlyArray<RepeatMode> = ["off", "all", "one"];

function nextRepeat(current: RepeatMode): RepeatMode {
  const idx = REPEAT_CYCLE.indexOf(current);
  return REPEAT_CYCLE[(idx + 1) % REPEAT_CYCLE.length] ?? "off";
}

function repeatLabel(mode: RepeatMode): string {
  switch (mode) {
    case "off":
      return "Repeat off";
    case "all":
      return "Repeat all";
    case "one":
      return "Repeat one";
  }
}

export function QueueView() {
  const entries = useQueueStore((s) => s.entries);
  const currentIndex = useQueueStore((s) => s.currentIndex);
  const repeat = useQueueStore((s) => s.repeat);
  const shuffle = useQueueStore((s) => s.shuffle);

  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<unknown>, label: string) => {
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

  const onJumpTo = (entryId: string) =>
    void run(() => queueJumpTo(entryId), "Jump to entry");

  const onRemove = (entryId: string) =>
    void run(async () => {
      const removed = await queueRemove(entryId);
      if (!removed) toast("Entry not found in queue");
    }, "Remove entry");

  const onClear = () => void run(() => queueClear(), "Clear queue");

  const onToggleRepeat = () =>
    void run(async () => {
      await setRepeat(nextRepeat(repeat));
    }, "Repeat");

  const onToggleShuffle = () =>
    void run(async () => {
      await setShuffle(!shuffle);
    }, "Shuffle");

  return (
    <section className="flex flex-col gap-4 p-6">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 className="text-2xl font-semibold">Queue</h1>
          <p className="text-sm text-fg-subtle">
            {entries.length === 0
              ? "Empty — play a track or album to start."
              : `${entries.length} ${entries.length === 1 ? "track" : "tracks"}`}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onToggleShuffle}
            disabled={busy}
            aria-pressed={shuffle}
            aria-label={shuffle ? "Disable shuffle" : "Enable shuffle"}
            className={
              "btn-ghost text-sm " +
              (shuffle ? "bg-accent/20 text-accent" : "")
            }
          >
            Shuffle {shuffle ? "on" : "off"}
          </button>
          <button
            type="button"
            onClick={onToggleRepeat}
            disabled={busy}
            aria-label={`Cycle repeat mode (currently ${repeatLabel(repeat)})`}
            className={
              "btn-ghost text-sm " +
              (repeat !== "off" ? "bg-accent/20 text-accent" : "")
            }
          >
            {repeatLabel(repeat)}
          </button>
          <button
            type="button"
            onClick={onClear}
            disabled={busy || entries.length === 0}
            className="btn-ghost text-sm"
          >
            Clear
          </button>
        </div>
      </header>

      {entries.length > 0 && (
        <ol
          className="divide-y divide-bg-raised overflow-hidden rounded-md border border-bg-raised"
          aria-label="Up next"
        >
          {entries.map((entry, index) => {
            const isCurrent = index === currentIndex;
            return (
              <li
                key={entry.id}
                className={
                  "grid grid-cols-[2.5rem_1fr_auto_auto] items-center gap-3 px-3 py-2 text-sm " +
                  (isCurrent ? "bg-bg-subtle" : "hover:bg-bg-subtle/60")
                }
              >
                <div className="text-right font-mono text-xs text-fg-muted">
                  {isCurrent ? "▶" : index + 1}
                </div>
                <button
                  type="button"
                  onClick={() => void onJumpTo(entry.id)}
                  disabled={busy}
                  className="min-w-0 text-left focus:outline-none"
                  aria-label={`Jump to ${entry.title}`}
                >
                  <div
                    className={
                      "truncate font-medium " +
                      (isCurrent ? "text-accent" : "text-fg")
                    }
                  >
                    {entry.title}
                  </div>
                  <div className="truncate text-xs text-fg-subtle">
                    {entry.artist}
                    {" · "}
                    {entry.album}
                  </div>
                </button>
                <div className="text-xs text-fg-muted">
                  {formatDuration(entry.durationSeconds)}
                </div>
                <button
                  type="button"
                  onClick={() => void onRemove(entry.id)}
                  disabled={busy}
                  aria-label={`Remove ${entry.title}`}
                  className="rounded-md p-1 text-fg-muted hover:bg-bg-raised hover:text-fg focus:outline-none disabled:opacity-40"
                >
                  ✕
                </button>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}
