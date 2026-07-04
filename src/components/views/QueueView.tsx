// QueueView — list of queue entries read from `useQueueStore`,
// kept in sync by the global playback event bridge at the app root.
//
// Per-entry actions:
//   - click row → jump_to (skip playback to that entry)
//   - remove button → queue_remove
//
// Header actions: clear-queue. Shuffle + repeat toggles live in the
// PlayerBar's TransportControls (canonical source of truth via
// `usePlayback().snapshot.repeat/shuffle`); this view just shows
// their state in the header subtitle.

import { useState } from "react";
import { toast } from "sonner";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { repeatLabel } from "@/lib/repeat";
import { queueClear, queueJumpTo, queueRemove } from "@/lib/tauri";
import { useQueueStore } from "@/stores/queueStore";

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
      toast.error(`${label}: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onJumpTo = (entryId: string) => void run(() => queueJumpTo(entryId), "Jump to entry");

  const onRemove = (entryId: string) =>
    void run(async () => {
      const removed = await queueRemove(entryId);
      if (!removed) toast("Entry not found in queue");
    }, "Remove entry");

  const onClear = () => void run(() => queueClear(), "Clear queue");

  const subtitle =
    entries.length === 0
      ? "Empty — play a track or album to start."
      : `${entries.length} ${entries.length === 1 ? "track" : "tracks"}`;

  const modeLine = `${repeatLabel(repeat, "short")} · shuffle ${shuffle ? "on" : "off"}`;

  return (
    <section className="flex flex-col gap-4 p-6">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 className="text-2xl font-semibold">Queue</h1>
          <p className="text-sm text-muted-foreground">{subtitle}</p>
          <p className="text-xs text-muted-foreground/70">{modeLine}</p>
        </div>
        <div className="flex items-center gap-2">
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
          className="divide-y divide-border overflow-hidden rounded-md border border-border"
          aria-label="Up next"
        >
          {entries.map((entry, index) => {
            const isCurrent = index === currentIndex;
            return (
              <li
                key={entry.id}
                className={
                  "grid grid-cols-[2.5rem_1fr_auto_auto] items-center gap-3 px-3 py-2 text-sm " +
                  (isCurrent ? "bg-muted" : "hover:bg-muted/60")
                }
              >
                <div className="text-right font-mono text-xs text-muted">
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
                    className={`truncate font-medium ${isCurrent ? "text-primary" : "text-foreground"}`}
                  >
                    {entry.title}
                  </div>
                  <div className="truncate text-xs text-fg-subtle">
                    {entry.artist}
                    {" · "}
                    {entry.album}
                  </div>
                </button>
                <div className="text-xs text-fg-muted">{formatDuration(entry.durationSeconds)}</div>
                <button
                  type="button"
                  onClick={() => void onRemove(entry.id)}
                  disabled={busy}
                  aria-label={`Remove ${entry.title}`}
                  className="rounded-md p-1 text-muted hover:bg-card hover:text-foreground focus:outline-none disabled:opacity-40"
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
