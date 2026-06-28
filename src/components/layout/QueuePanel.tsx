// QueuePanel — slide-in panel from the right showing the playback queue.
// Opened via the queue button in PlayerBar. Overlaps the content area
// without pushing it (unlike the sidebar).

import { Delete03Icon, RepeatIcon, RepeatOne01Icon, ShuffleIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { toast } from "sonner";
import { cn } from "@/lib/cn";
import { extractError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { nextRepeat, repeatLabel } from "@/lib/repeat";
import { queueClear, queueJumpTo, queueRemove, setRepeat, setShuffle } from "@/lib/tauri";
import { useQueueStore } from "@/stores/queueStore";

interface Props {
  onClose: () => void;
}

export function QueuePanel({ onClose }: Props) {
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

  const onJumpTo = (entryId: string) => void run(() => queueJumpTo(entryId), "Jump");
  const onRemove = (entryId: string) =>
    void run(async () => {
      const removed = await queueRemove(entryId);
      if (!removed) toast("Entry not found");
    }, "Remove");
  const onClear = () => void run(() => queueClear(), "Clear");
  const onToggleRepeat = () =>
    void run(async () => {
      await setRepeat(nextRepeat(repeat));
    }, "Repeat");
  const onToggleShuffle = () =>
    void run(async () => {
      await setShuffle(!shuffle);
    }, "Shuffle");

  return (
    <div className="absolute inset-y-0 right-0 z-40 flex w-80 flex-col border-l border-border bg-card shadow-xl">
      {/* Header */}
      <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold text-foreground">Queue</h2>
          <p className="text-xs text-muted-foreground">
            {entries.length === 0 ? "Empty" : `${entries.length} tracks`}
          </p>
        </div>
        <div className="flex items-center gap-1">
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
          <button
            type="button"
            onClick={onClose}
            aria-label="Close queue"
            className="ml-1 size-7 rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            ✕
          </button>
        </div>
      </div>

      {/* Queue list */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        {entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-6 text-center">
            <p className="text-sm text-muted-foreground">Nothing in queue</p>
            <p className="mt-1 text-xs text-muted-foreground">Play a track to add it here</p>
          </div>
        ) : (
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
        )}
      </div>
    </div>
  );
}
