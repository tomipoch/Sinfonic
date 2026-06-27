// SyncOverlay — non-blocking progress chip for inline empty states.
//
// Drop into any view (e.g. the "No songs yet" empty state of
// `SongsView`) so the user sees the same step-by-step progress as
// `LoadingView` without being kicked off the current route. Renders
// nothing while a sync isn't running, so it's safe to mount
// unconditionally.

import { useSyncProgress } from "@/hooks/useSyncProgress";
import {
  SyncChecklist,
  SyncProgressBar,
  buildChecklistSteps,
  stepForState,
} from "@/components/ui/SyncChecklist";
import { useServerStore, type ServerKind } from "@/stores/serverStore";

interface Props {
  /**
   * Optional override for the displayed server kind. Falls back to
   * the active server's kind from the store, which is what 99 % of
   * callers want.
   */
  kind?: ServerKind;
  /** Render the bar above the checklist. Defaults to `true`. */
  showProgressBar?: boolean;
  /** Render the steps list. Defaults to `true`. */
  showSteps?: boolean;
  className?: string;
}

export function SyncOverlay({
  kind,
  showProgressBar = true,
  showSteps = true,
  className,
}: Props) {
  const activeKind = useServerStore(
    (s) => s.servers.find((sv) => sv.id === s.activeServerId)?.kind,
  );
  const effectiveKind: ServerKind = kind ?? activeKind ?? "local";

  const sync = useSyncProgress();

  if (!sync.active && !sync.done && !sync.error) return null;

  const currentStep = stepForState(sync.state);
  const steps = buildChecklistSteps({
    kind: effectiveKind,
    currentStep,
    // The connecting step has already happened by the time we show
    // this overlay — skip it so the user sees the active step first.
    skipConnecting: true,
  });

  return (
    <div
      className={
        "flex flex-col gap-4 rounded-lg border border-border bg-card/60 p-5 shadow-sm " +
        (className ?? "")
      }
      role="status"
      aria-live="polite"
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex flex-col gap-0.5">
          <div className="text-sm font-semibold text-foreground">
            {sync.done ? "Library ready" : "Syncing your library"}
          </div>
          <div className="text-xs text-muted-foreground">
            {sync.done
              ? "All set — the view below is up to date."
              : "Hang tight, this only happens once per source."}
          </div>
        </div>
      </div>
      {showProgressBar && (
        <SyncProgressBar progress={sync.progress} done={sync.done} />
      )}
      {showSteps && <SyncChecklist steps={steps} />}
      {sync.error && (
        <div
          role="alert"
          className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
        >
          {sync.error}
        </div>
      )}
    </div>
  );
}
