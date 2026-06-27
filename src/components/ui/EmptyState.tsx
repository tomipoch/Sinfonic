// EmptyState — empty-state card for library views.
//
// Used by SongsView, AlbumsView, ArtistsView, GenresView when the
// library cache is loaded but has no rows yet (typically before the
// first sync). Includes the primary "Sync library" CTA, which:
//   1. Fires `onSync` immediately, then
//   2. Navigates to `/loading` so the user sees the full sync
//      progress UI instead of staying on the empty view.
//
// Mounts `<SyncOverlay />` underneath so if a sync is already in
// progress (e.g. a background one kicked off by switching sources)
// the user still sees the steps below the CTA.

import { useNavigate } from "react-router-dom";
import type { ReactNode } from "react";

import { SyncOverlay } from "@/components/ui/SyncOverlay";

interface Props {
  title: string;
  description: ReactNode;
  syncLabel: string;
  syncing: boolean;
  onSync: () => Promise<void> | void;
}

export function EmptyState({
  title,
  description,
  syncLabel,
  syncing,
  onSync,
}: Props) {
  const navigate = useNavigate();
  return (
    <div className="flex flex-col gap-4 p-6">
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
        <div className="text-base font-medium text-foreground">{title}</div>
        <div className="text-sm text-muted-foreground">{description}</div>
        <button
          type="button"
          onClick={() => {
            void onSync();
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
