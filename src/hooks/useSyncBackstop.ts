// `useSyncBackstop` — silent-sync detector.
//
// Fires `onStale` if no `sync.*` value changes for `timeoutMs`
// milliseconds. Cancels itself the moment `done === true` or
// `error !== null`. Designed to be safe under StrictMode (the
// timer is registered in an effect with a cleanup that clears it,
// so the dev double-invoke does not produce a duplicate timer).
//
// Extracted from `LoadingView` so the timer-reset semantics can be
// unit-tested without rendering the full view (which spins up
// toast, react-router, the server store, and a Tauri listener).
//
// Usage:
//   useSyncBackstop(sync, 30_000, () => navigate("/", { replace: true }));

import { useEffect } from "react";

export interface BackstopSyncState {
  state: string;
  progress: number;
  done: boolean;
  active: boolean;
  error: string | null;
}

export function useSyncBackstop(
  sync: BackstopSyncState,
  timeoutMs: number,
  onStale: () => void,
): void {
  useEffect(() => {
    if (sync.done || sync.error) return;
    const timer = window.setTimeout(onStale, timeoutMs);
    return () => window.clearTimeout(timer);
  }, [onStale, timeoutMs, sync.state, sync.progress, sync.done, sync.active, sync.error]);
}
