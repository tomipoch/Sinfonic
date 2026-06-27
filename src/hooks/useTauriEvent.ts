// Typed wrapper around Tauri's `listen` for use inside React effects.
//
// Why: `listen` returns an unlisten function, has its own setup state,
// and uses a generic for the payload type. Without this hook every
// effect would re-implement the same boilerplate.
//
// The cleanup swallows errors from the unlisten call. Tauri's
// internal `__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener`
// can throw "listeners[eventId].handlerId is undefined" when the
// listener was registered from a different JS context (e.g. a
// navigation that swapped the webview's JS world) or when the
// effect's `listen` promise hadn't fully resolved before the
// component unmounted. Both cases are best-effort cleanup — the
// listener will be GC'd either way. The rejection can be async (the
// underlying `_unlisten` is `async`), so we have to catch the
// returned Promise too.

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

function safelyUnlisten(fn: UnlistenFn | undefined): void {
  if (!fn) return;
  try {
    const result = fn() as unknown;
    if (
      result !== null &&
      typeof result === "object" &&
      typeof (result as { catch?: unknown }).catch === "function"
    ) {
      (result as Promise<unknown>).catch(() => {
        // StrictMode-cleanup race — listener is already gone.
      });
    }
  } catch {
    // Sync throw — same "already gone" territory.
  }
}

export const useTauriEvent = <T>(
  name: string,
  handler: (payload: T) => void,
  deps: ReadonlyArray<unknown> = [name],
) => {
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;

    void listen<T>(name, (event) => {
      handler(event.payload);
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        safelyUnlisten(fn);
      }
    });

    return () => {
      active = false;
      safelyUnlisten(unlisten);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
};
