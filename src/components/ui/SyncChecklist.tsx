// SyncChecklist — shared checklist rendering for any sync progress UI.
//
// Two consumers reuse this:
//   - `LoadingView` (full-screen first-run / re-sync experience)
//   - `SyncOverlay` (non-blocking inline chip on top of an existing view)
//
// The step labels branch on `kind` (jellyfin / subsonic / local) so the
// same component can describe a remote library pull or a local folder
// scan without duplicating the layout.

import {
  CheckmarkCircle01Icon,
  HardDriveIcon,
  Loading03Icon,
  SqlIcon,
  Tick02Icon,
  Wifi01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type { SyncState } from "@/hooks/useSyncProgress";
import { cn } from "@/lib/cn";
import type { ServerKind } from "@/types/domain";

export type StepId = "connecting" | "scanning" | "indexing" | "caching" | "ready";

type StepStatus = "pending" | "active" | "done";

const STEP_ICONS = {
  connecting: Tick02Icon,
  scanning: HardDriveIcon,
  indexing: SqlIcon,
  caching: Wifi01Icon,
  ready: CheckmarkCircle01Icon,
} as const;

export function stepForState(state: SyncState): StepId {
  switch (state) {
    case "preparing":
      return "connecting";
    case "started":
    case "scanning":
      return "scanning";
    case "indexing":
      return "indexing";
    case "caching":
    case "syncing":
      return "caching";
    case "complete":
      return "ready";
    case "error":
      // The full-screen LoadingView shows the error inline; from
      // the checklist's perspective an error is "we stopped before
      // reaching the ready state" — surface it as the last completed
      // step so the user sees everything that DID land plus the error
      // indicator on the LoadingView header.
      return "caching";
  }
}

const CONNECTING_DETAIL: Record<ServerKind, string> = {
  jellyfin: "Signing in to your Jellyfin server",
  subsonic: "Signing in to your Subsonic server",
  local: "Reading folder contents",
};

const READY_DETAIL: Record<ServerKind, string> = {
  jellyfin: "Library cached. Loading your home view…",
  subsonic: "Library cached. Loading your home view…",
  local: "Scan complete. Loading your home view…",
};

export interface ChecklistStep {
  id: StepId;
  label: string;
  detail: string;
  status: StepStatus;
  icon: typeof Tick02Icon;
}

export interface BuildStepsOptions {
  kind: ServerKind;
  currentStep: StepId;
  /**
   * If the connecting step already happened elsewhere (e.g. the user
   * just logged in successfully) skip showing it as `pending` and
   * mark it `done` so the checklist starts at `scanning`.
   */
  skipConnecting?: boolean;
}

export function buildChecklistSteps({
  kind,
  currentStep,
  skipConnecting,
}: BuildStepsOptions): ChecklistStep[] {
  const order: StepId[] = ["connecting", "scanning", "indexing", "caching", "ready"];
  const curIdx = order.indexOf(currentStep);
  return [
    {
      id: "connecting",
      label: "Connecting",
      detail: CONNECTING_DETAIL[kind],
      icon: STEP_ICONS.connecting,
      status:
        skipConnecting || curIdx > order.indexOf("connecting")
          ? "done"
          : curIdx === order.indexOf("connecting")
            ? "active"
            : "pending",
    },
    {
      id: "scanning",
      label: kind === "local" ? "Scanning folder" : "Discovering library",
      detail:
        kind === "local"
          ? "Walking files and reading audio tags"
          : "Fetching artists and albums from the server",
      icon: STEP_ICONS.scanning,
      status: statusFor("scanning", currentStep),
    },
    {
      id: "indexing",
      label: "Indexing",
      detail: "Building the local search cache",
      icon: STEP_ICONS.indexing,
      status: statusFor("indexing", currentStep),
    },
    {
      id: "caching",
      label: kind === "local" ? "Caching artwork" : "Syncing tracks",
      detail:
        kind === "local"
          ? "Resolving cover art for every album"
          : "Pulling tracks and persisting them locally",
      icon: STEP_ICONS.caching,
      status: statusFor("caching", currentStep),
    },
    {
      id: "ready",
      label: "Ready",
      detail: READY_DETAIL[kind],
      icon: STEP_ICONS.ready,
      status: statusFor("ready", currentStep),
    },
  ];
}

function statusFor(stepId: StepId, current: StepId): StepStatus {
  const order: StepId[] = ["connecting", "scanning", "indexing", "caching", "ready"];
  const stepIdx = order.indexOf(stepId);
  const curIdx = order.indexOf(current);
  if (stepIdx < 0 || curIdx < 0) return "pending";
  if (stepIdx < curIdx) return "done";
  if (stepIdx === curIdx) return "active";
  return "pending";
}

export interface ChecklistProps {
  steps: ChecklistStep[];
  className?: string;
}

export function SyncChecklist({ steps, className }: ChecklistProps) {
  return (
    <ol className={cn("flex flex-col gap-2", className)}>
      {steps.map((step) => (
        <li
          key={step.id}
          className={cn(
            "flex items-start gap-3 rounded-lg border border-border bg-card/50 px-4 py-3 transition-colors",
            step.status === "active" && "border-primary/40 bg-primary/5",
            step.status === "done" && "border-primary/20 bg-card/30",
          )}
        >
          <div
            className={cn(
              "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full",
              step.status === "done"
                ? "bg-primary text-primary-foreground"
                : step.status === "active"
                  ? "bg-primary/20 text-primary"
                  : "bg-muted text-muted-foreground",
            )}
            aria-hidden
          >
            {step.status === "active" ? (
              <HugeiconsIcon
                icon={Loading03Icon}
                size={14}
                strokeWidth={2.5}
                className="animate-spin"
              />
            ) : (
              <HugeiconsIcon icon={step.icon} size={14} strokeWidth={2} />
            )}
          </div>
          <div className="flex min-w-0 flex-col">
            <div
              className={cn(
                "text-sm font-medium",
                step.status === "pending" ? "text-muted-foreground" : "text-foreground",
              )}
            >
              {step.label}
            </div>
            <div className="text-xs text-muted-foreground">{step.detail}</div>
          </div>
        </li>
      ))}
    </ol>
  );
}

export interface ProgressBarProps {
  progress: number;
  done: boolean;
  className?: string;
}

export function SyncProgressBar({ progress, done, className }: ProgressBarProps) {
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
        <div
          className="h-full bg-primary transition-[width] duration-300 ease-out"
          style={{ width: `${Math.round(progress * 100)}%` }}
        />
      </div>
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>{Math.round(progress * 100)}%</span>
        <span>{done ? "Done" : "Hang tight — first run only takes a moment"}</span>
      </div>
    </div>
  );
}
