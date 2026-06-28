// LoginDialog — modal dialog for adding a new source from inside the
// app (sidebar dropdown, Settings window, anywhere the user has a
// reason to connect to a *different* server than the one currently
// active).
//
// The actual connection UI is the shared `ServerConnectionForm`; this
// wrapper just adds the modal chrome.
//
// Two submission modes:
//   * **Default** (no `onSubmit`): queue the values in
//     `pendingConnection` and navigate to `/loading`, which kicks off
//     the login + scan + sync pipeline in LoadingView's handoff. This
//     is the sidebar flow — the user is in the main window and wants
//     to see the loading progress immediately.
//   * **Custom `onSubmit`**: the caller takes full ownership of what
//     happens with the form values. The Settings window uses this
//     because `navigate("/loading")` would target its own (Settings)
//     router and silently no-op; the Settings caller instead calls
//     `serverStore.login()` directly so the new server becomes
//     active in-place and the saved-servers list refreshes.

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  type ConnectionValues,
  ServerConnectionForm,
} from "@/components/dialogs/ServerConnectionForm";
import { pickLocalFolder } from "@/lib/dialogs";
import { useServerStore } from "@/stores/serverStore";

interface Props {
  open: boolean;
  onClose: () => void;
  /**
   * If provided, replaces the default "set pending + navigate to
   * /loading" behaviour. Called with the form values; the caller is
   * responsible for closing the dialog (or not) and any post-login
   * side effects.
   */
  onSubmit?: (values: ConnectionValues) => void;
}

export function LoginDialog({ open, onClose, onSubmit }: Props) {
  const navigate = useNavigate();
  const [dismissed, setDismissed] = useState(false);

  const discovered = useServerStore((s) => s.discovered);
  const storeError = useServerStore((s) => s.error);
  const setPendingConnection = useServerStore((s) => s.setPendingConnection);
  const discover = useServerStore((s) => s.discover);

  // Clear stale errors when the dialog mounts so a failed attempt
  // from a previous open doesn't bleed in.
  useEffect(() => {
    if (open) useServerStore.getState().clearError();
  }, [open]);

  if (!open || dismissed) return null;

  const handleClose = () => {
    setDismissed(true);
    onClose();
  };

  const handleSubmit = (values: ConnectionValues) => {
    if (onSubmit) {
      onSubmit(values);
      return;
    }
    // Default: queue the connection and hand off to /loading. The
    // route mount order matters — we close the dialog and navigate
    // in the same tick so LoadingView mounts before the listener can
    // miss any events.
    setPendingConnection(values);
    handleClose();
    void navigate("/loading", { replace: true });
  };

  const handlePickLocalPath = async () => {
    try {
      return await pickLocalFolder();
    } catch (err) {
      useServerStore.getState().clearError();
      throw err;
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Add a new source"
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 px-4 pt-[15vh] backdrop-blur-sm"
      onClick={(e) => {
        // Click on the backdrop closes; clicks inside the card don't
        // bubble up because the card has its own click boundary.
        if (e.target === e.currentTarget) handleClose();
      }}
    >
      <div className="w-full max-w-lg rounded-xl border border-border bg-card shadow-2xl">
        <div className="flex items-center justify-between border-b border-border px-6 py-4">
          <h2 className="text-base font-semibold text-foreground">Add a new source</h2>
          <button
            type="button"
            onClick={handleClose}
            className="size-7 rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <div className="max-h-[60vh] overflow-y-auto p-6">
          <ServerConnectionForm
            variant="modal"
            discovered={discovered}
            error={storeError}
            onSubmit={handleSubmit}
            onDiscover={() => void discover()}
            onPickLocalPath={handlePickLocalPath}
          />
        </div>
      </div>
    </div>
  );
}
