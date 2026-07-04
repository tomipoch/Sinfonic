// SubsonicSyncIndicator — slim chip that appears in the sidebar
// while the Subsonic album-tracks background sync is running.
//
// Phase 3 of feature/direct-fetch-providers: Subsonic fans out
// `getAlbum` for every album on the server to populate the
// SQLite tracks cache; the backend emits per-batch `sync-progress`
// events with phase="tracks" so the UI can show progress. The
// chip disappears on `complete` / `error` and on server switch.

import { Loading03Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useSubsonicTrackSync } from "@/hooks/useSubsonicTrackSync";

export function SubsonicSyncIndicator() {
  const sync = useSubsonicTrackSync();
  if (!sync.active || sync.total === 0) return null;
  const percent = Math.min(100, Math.round((sync.done / sync.total) * 100));
  return (
    <div
      role="status"
      aria-live="polite"
      className="mx-1 flex items-center gap-2 rounded-md border border-border/60 bg-card/80 px-2.5 py-2 text-xs text-muted-foreground"
    >
      <span
        className="flex size-5 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary"
        aria-hidden
      >
        <HugeiconsIcon icon={Loading03Icon} size={11} strokeWidth={2.5} className="animate-spin" />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="truncate text-foreground">Sincronizando canciones…</span>
        <div className="flex items-center gap-2">
          <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full bg-primary transition-[width] duration-300 ease-out"
              style={{ width: `${percent}%` }}
            />
          </div>
          <span className="shrink-0 font-mono tabular-nums text-[10px]">
            {sync.done}/{sync.total}
          </span>
        </div>
      </div>
    </div>
  );
}
