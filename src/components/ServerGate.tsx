// ServerGate — global bridge that waits for the backend's startup
// restore to finish, then funnels the user into the setup view.
//
//   * On mount: polls `bootstrap_state` until the backend signals
//     ready. The bundled response carries the active server id plus
//     the full list of saved servers so the Quick Connect list in
//     SetupView can render straight away.
//   * While not ready, render a thin loading shell so the user
//     never sees a flash of the wrong screen.
//   * Once ready, redirect to `/setup` for users who have no server
//     configured AND are not already in the middle of a connect.
//     `/setup` is the welcome screen; users with a previously-
//     configured source use the Quick Connect section to re-attach,
//     and users without one fill in credentials / pick a folder.
//
// Idempotency: the redirect is guarded by a `useRef` flag so it
// fires at most once per ServerGate lifetime. Without this guard,
// a navigation chain like /setup → Quick Connect → /loading → /
// would re-run the redirect effect when the path hits `/` and trap
// the user on /setup forever — defeating the whole Quick Connect flow.
// The `activeServerId` guard is defence-in-depth: even if the ref is
// cleared (or never trips), a user with a connected server is never
// bounced back to /setup.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { bootstrapState } from "@/lib/tauri";
import { useServerStore } from "@/stores/serverStore";
import { makeLogger } from "@/utils/log";

const log = makeLogger("ServerGate");

type Props = {
  children: ReactNode;
};

const EXEMPT_PATHS = new Set(["/setup", "/loading"]);

export function ServerGate({ children }: Props) {
  const navigate = useNavigate();
  const location = useLocation();
  const setServers = useServerStore((s) => s.setServers);
  const setActiveServerId = useServerStore((s) => s.setActiveServerId);

  const [ready, setReady] = useState(false);
  const hasRedirectedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      // Light backoff. The restore is sub-second on a warm filesystem
      // so we don't want a long delay between the first read and the
      // ready state landing, but we also don't want to spin if the
      // IPC channel is briefly slow during startup.
      const delays = [0, 50, 100, 200, 400, 400, 400];
      let attempt = 0;
      while (!cancelled) {
        try {
          const state = await bootstrapState();
          if (cancelled) return;
          // Hydrate the server store from the same snapshot the route
          // guard reads. This avoids a second pair of
          // `refreshServers`/`refreshActive` calls racing the restore
          // task and missing the restored provider.
          log.log("bootstrapState", {
            ready: state.ready,
            activeServerId: state.activeServerId,
            savedServersCount: state.savedServers.length,
          });
          setServers(state.savedServers);
          setActiveServerId(state.activeServerId);
          if (state.ready) {
            log.log("bootstrap complete, setting ready=true");
            setReady(true);
            return;
          }
        } catch (err) {
          if (cancelled) return;
          log.warn("bootstrap_state failed", err);
        }
        const wait = delays[Math.min(attempt, delays.length - 1)];
        attempt += 1;
        await new Promise((r) => setTimeout(r, wait));
      }
    };
    void poll();
    return () => {
      cancelled = true;
    };
  }, [setServers, setActiveServerId]);

  useEffect(() => {
    if (!ready || hasRedirectedRef.current) return;
    const path = location.pathname;
    if (EXEMPT_PATHS.has(path)) return;
    if (useServerStore.getState().activeServerId) return;
    hasRedirectedRef.current = true;
    log.log("bootstrap done, redirecting to /setup (was:", path, ")");
    void navigate("/setup", { replace: true });
  }, [ready, navigate, location.pathname]);

  if (!ready) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-background text-sm text-muted-foreground">
        Loading…
      </div>
    );
  }

  return <>{children}</>;
}