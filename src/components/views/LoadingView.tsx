// LoadingView — owns the entire post-form-submit handoff for every
// flow in the app that needs to wait for a sync to finish:
// first-run setup, LoginDialog, Quick Connect, manual Sync library
// button, and server switch.
//
// The caller writes a `pendingConnection` to the store (for new
// sources) OR just navigates here with an existing active server
// (for re-syncs). This view branches on the two cases and either
// runs the login/scan/sync pipeline or just kicks off a fresh
// `provider_sync_library`. On `sync.done` it navigates to `/`; on
// error it bounces back to `/setup` so the user can correct.

import { SparklesIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import {
  buildChecklistSteps,
  SyncChecklist,
  SyncProgressBar,
  stepForState,
} from "@/components/ui/SyncChecklist";
import { useSyncBackstop } from "@/hooks/useSyncBackstop";
import { useSyncProgress } from "@/hooks/useSyncProgress";
import { extractError } from "@/lib/errors";
import { useServerStore } from "@/stores/serverStore";
import type { ServerKind } from "@/types/domain";
import { makeLogger } from "@/utils/log";

const log = makeLogger("LoadingView");

export function LoadingView() {
  const navigate = useNavigate();
  const servers = useServerStore((s) => s.servers);
  const activeServerId = useServerStore((s) => s.activeServerId);
  const pendingConnection = useServerStore((s) => s.pendingConnection);
  const setPendingConnection = useServerStore((s) => s.setPendingConnection);
  const login = useServerStore((s) => s.login);
  const syncLibrary = useServerStore((s) => s.syncLibrary);
  const activeServer = servers.find((s) => s.id === activeServerId);
  const kind: ServerKind = activeServer?.kind ?? pendingConnection?.kind ?? "local";

  const sync = useSyncProgress();

  // One handoff that covers every entry point into this route:
  //   1. `pendingConnection` set  → run the login pipeline + (for
  //      remote sources) a fresh sync.
  //   2. No pending, but a server is active → just re-sync (manual
  //      Sync button, Quick Connect, server switch).
  //   3. Neither → redirect to setup.
  // Guarded by a ref so a re-render doesn't fire a second handoff.
  // We wait for `sync.ready` (the Tauri listener registration) so
  // the first `library-sync-status` event fired by the backend can't
  // race past us and be lost.
  const handoffStartedRef = useRef(false);
  useEffect(() => {
    if (handoffStartedRef.current) return;
    if (!sync.ready) return;
    handoffStartedRef.current = true;

    const pending = pendingConnection;
    if (pending) setPendingConnection(null);

    log.log("handoff starting", {
      pending: pending?.kind ?? null,
      activeServerId,
    });

    void (async () => {
      try {
        if (pending) {
          if (pending.kind === "local") {
            // `local_login` is the scan itself and emits the whole
            // preparing/scanning/indexing/complete stream, so we
            // skip the redundant `syncLibrary` call here.
            await login({ kind: "local", path: pending.path });
          } else if (pending.kind === "jellyfin") {
            await login({
              kind: "jellyfin",
              baseUrl: pending.baseUrl,
              username: pending.username,
              password: pending.password,
            });
            await syncLibrary();
          } else {
            await login({
              kind: "subsonic",
              baseUrl: pending.baseUrl,
              username: pending.username,
              password: pending.password,
            });
            await syncLibrary();
          }
        } else if (activeServerId) {
          // Manual sync / Quick Connect / server switch: the
          // provider is already installed, just fetch a fresh copy
          // of the library.
          await syncLibrary();
        } else {
          // No server, no pending connection — nothing to sync.
          log.log("nothing to sync, back to setup");
          void navigate("/setup", { replace: true });
        }
        log.log("handoff login/syncLibrary complete, waiting for sync.done");
      } catch (err) {
        const msg = extractError(err, "connection failed");
        log.error("handoff failed", msg);
        toast.error(msg);
        void navigate("/setup", { replace: true });
      }
    })();
  }, [
    pendingConnection,
    activeServerId,
    setPendingConnection,
    login,
    syncLibrary,
    navigate,
    sync.ready,
  ]);

  // Navigate home once the backend reports `complete`. The 350 ms
  // delay lets the UI paint the final "ready" state before the
  // route swap. This effect runs only when `sync.done` flips — not
  // on every progress event — so the 350 ms timer isn't reset
  // while progress is still ticking.
  useEffect(() => {
    if (!sync.done) return;
    log.log("sync.done — scheduling navigate to /");
    const timer = window.setTimeout(() => {
      log.log("sync.done — navigating to /");
      void navigate("/", { replace: true });
    }, 350);
    return () => window.clearTimeout(timer);
  }, [navigate, sync.done]);

  // 5 minute safety backstop for hung syncs. The timer resets on
  // every sync progress event so a legitimate long scan (local
  // source over a network share, large Jellyfin/Subsonic library on
  // the first page) never gets force-navigated away. The 5 min
  // ceiling is generous — a NAS scan over AFP/SMB can take that
  // long — but if the backend is silent for that long we still bail
  // out so the user is not stuck on the loading screen forever.
  useSyncBackstop(sync, 5 * 60 * 1000, () => {
    log.log("5m backstop — navigating to / (sync silent for 5 min)");
    void navigate("/", { replace: true });
  });

  // Log every sync state transition so the user can see in
  // DevTools exactly which event arrived (or didn't) before the
  // 30 s backstop fires.
  useEffect(() => {
    log.log("sync state", {
      state: sync.state,
      progress: sync.progress,
      done: sync.done,
      active: sync.active,
      error: sync.error,
    });
    if (sync.error) {
      log.error("sync.error", sync.error);
    }
  }, [sync.state, sync.progress, sync.done, sync.active, sync.error]);

  const currentStep = stepForState(sync.state);
  const steps = buildChecklistSteps({
    kind,
    currentStep,
    skipConnecting: kind !== "local",
  });

  return (
    <div className="flex h-full w-full items-stretch justify-center overflow-auto [overscroll-behavior:contain]">
      <div className="flex w-full max-w-xl flex-col gap-8 p-8 md:p-12">
        <header className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-lg bg-primary text-primary-foreground">
              <HugeiconsIcon icon={SparklesIcon} size={20} strokeWidth={2.25} />
            </div>
            <h1 className="text-3xl font-semibold tracking-tight text-foreground">
              Setting things up
            </h1>
          </div>
          <p className="max-w-prose text-sm text-muted-foreground">
            {activeServer
              ? `Connected to ${activeServer.name}. Caching your library so the home view can render without lag.`
              : "Caching your library so the home view can render without lag."}
          </p>
        </header>

        <SyncProgressBar progress={sync.progress} done={sync.done} />
        <SyncChecklist steps={steps} />

        {sync.error ? (
          <div
            role="alert"
            className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
          >
            {sync.error}
          </div>
        ) : null}

        <p className="text-[11px] text-muted-foreground">
          This screen will move on automatically when the cache is ready.
        </p>
      </div>
    </div>
  );
}
