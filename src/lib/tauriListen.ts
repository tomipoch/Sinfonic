// `safelyUnlisten` — best-effort cleanup of a Tauri event listener.
//
// Tauri's internal `__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener`
// can throw `listeners[eventId].handlerId is undefined` when the
// listener was registered from a different JS context (e.g. a
// navigation that swapped the webview's JS world) or when the
// effect's `listen` promise hadn't fully resolved before the
// component unmounted. Both cases are best-effort cleanup — the
// listener will be GC'd either way. The rejection can be async
// (the underlying `_unlisten` is `async`), so we have to catch the
// returned Promise too.
//
// Centralised because the same wrapper exists in three places
// (`useTauriEvent`, `useLibraryAutoLoad`, `useSyncProgress`).

import type { UnlistenFn } from "@tauri-apps/api/event";

export function safelyUnlisten(fn: UnlistenFn | null | undefined): void {
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
