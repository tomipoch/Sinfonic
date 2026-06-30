// `resetSessionState` — clear the library, queue, and playback
// snapshot so the UI doesn't flash entries from a server we're
// about to (or have just) left.
//
// Called by `serverStore` on three lifecycle events:
//   - `logout()`            — user signs out
//   - `deleteServer()`      — active server was deleted
//   - `setActive(otherId)`  — user switched to a different server
//
// The playback snapshot lives inside a React context (see
// `usePlayback`) so we can't `getState()` it the same way we do
// the library / queue Zustand stores. The provider registers a
// reset callback via `registerPlaybackReset` on mount; this module
// just forwards to whatever was last registered.
//
// If the provider hasn't mounted yet (extremely rare: reset is
// only ever triggered by user actions after boot), the playback
// snapshot is left alone — the next poll will overwrite it with
// the new server's state anyway.

import { useLibraryStore } from "@/stores/libraryStore";
import { useQueueStore } from "@/stores/queueStore";

type Resetter = () => void;

let registeredResetter: Resetter | null = null;

export function registerPlaybackReset(fn: Resetter): () => void {
  registeredResetter = fn;
  return () => {
    if (registeredResetter === fn) registeredResetter = null;
  };
}

export function resetSessionState(): void {
  useLibraryStore.getState().reset();
  useQueueStore.getState().clear();
  registeredResetter?.();
}
