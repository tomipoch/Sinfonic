// SyncBanner — slim inline banner under the title bar that appears
// whenever the backend is syncing a library. Mirrors the same
// `library-sync-status` event the full-screen `LoadingView` listens
// for, but is non-blocking: it just shows a chip with the current
// state and a progress bar so the user knows something is happening
// when a sync was kicked off from the Settings window (or any
// future surface that doesn't navigate to `/loading`).
//
// Phase 5 of feature/direct-fetch-providers: once the sync reports
// `complete` we keep the "Library ready ✓" badge visible for a few
// seconds — long enough to confirm the sync landed — then auto-hide.
// Without this the chip stays on screen forever ("luego aparece
// otro debajo del header" in the user's report) because `done=true`
// is sticky until the next sync starts.

import { Loading03Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useState } from "react";
import { type SyncState, useSyncProgress } from "@/hooks/useSyncProgress";

const COMPLETE_VISIBLE_MS = 3_000;

function labelFor(state: SyncState): string {
  switch (state) {
    case "preparing":
      return "Connecting…";
    case "started":
    case "scanning":
      return "Scanning library…";
    case "indexing":
      return "Indexing…";
    case "caching":
      return "Caching album art…";
    case "syncing":
      return "Syncing tracks…";
    case "complete":
      return "Library ready";
    case "error":
      return "Sync failed";
  }
}

export function SyncBanner() {
  const sync = useSyncProgress();
  // `confirmedDone` is a one-shot timer that flips true when the
  // sync reports `complete`. We keep the chip visible for
  // COMPLETE_VISIBLE_MS after that so the user sees the success
  // confirmation, then hide it. The next sync resets `confirmedDone`
  // to false (via the `useEffect`) and the chip reappears.
  const [confirmedDone, setConfirmedDone] = useState(false);

  useEffect(() => {
    if (sync.done) {
      const id = window.setTimeout(() => setConfirmedDone(false), COMPLETE_VISIBLE_MS);
      return () => window.clearTimeout(id);
    }
    // During an in-progress sync, mark "not yet confirmed done" so
    // a new `done` flag re-shows the chip even if we just hid it.
    setConfirmedDone(true);
    return undefined;
  }, [sync.done]);

  const visible = sync.active || (sync.done && confirmedDone);
  if (!visible) return null;

  const percent = Math.round(sync.progress * 100);

  return (
    <div
      role="status"
      aria-live="polite"
      className="flex items-center gap-3 border-b border-border bg-card/60 px-4 py-2 text-xs text-muted-foreground"
    >
      <span
        className={
          "flex size-6 shrink-0 items-center justify-center rounded-full " +
          (sync.done ? "bg-primary text-primary-foreground" : "bg-primary/15 text-primary")
        }
        aria-hidden
      >
        {sync.done ? (
          <span className="text-sm leading-none">✓</span>
        ) : (
          <HugeiconsIcon
            icon={Loading03Icon}
            size={12}
            strokeWidth={2.5}
            className="animate-spin"
          />
        )}
      </span>
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <span className="truncate text-foreground">{labelFor(sync.state)}</span>
        <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full bg-primary transition-[width] duration-300 ease-out"
            style={{ width: `${percent}%` }}
          />
        </div>
        <span className="shrink-0 font-mono tabular-nums text-[11px]">{percent}%</span>
      </div>
    </div>
  );
}
