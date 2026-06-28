// EmptyState — empty-state card for library views.
//
// Used by SongsView, AlbumsView, ArtistsView, GenresView when the
// library cache is loaded but has no rows yet (typically before the
// first sync). Includes the primary "Sync library" CTA that
// navigates to `/loading` where `LoadingView` is the single source
// of truth for kicking off a sync — `EmptyState` does not call
// `onSync` itself, otherwise the same `provider_sync_library` IPC
// would fire twice (once here, once on mount of the LoadingView).
//
// Mounts `<SyncOverlay />` underneath so if a sync is already in
// progress (e.g. a background one kicked off by switching sources)
// the user still sees the steps below the CTA.

import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";

import { SyncOverlay } from "@/components/ui/SyncOverlay";

interface Props {
  title: string;
  description: ReactNode;
  syncLabel: string;
  syncing: boolean;
  /** @deprecated Callers should not pass this; sync is triggered by LoadingView on mount. Kept for back-compat and ignored. */
  onSync?: () => Promise<void> | void;
}

export function EmptyState({ title, description, syncLabel, syncing, onSync: _onSync }: Props) {
  const navigate = useNavigate();
  return (
    <div className="flex flex-col gap-4 p-6">
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">{title}</div>
        <div className="text-sm text-muted-foreground">{description}</div>
        <button
          type="button"
          onClick={() => {
            void navigate("/loading", { replace: true });
          }}
          disabled={syncing}
          className="btn-primary"
        >
          {syncing ? "Syncing…" : syncLabel}
        </button>
      </div>
      <SyncOverlay />
    </div>
  );
}
