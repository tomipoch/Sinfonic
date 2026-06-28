// `resetSessionState` — clear the library, queue, and playback
// stores so the UI doesn't flash entries from a server we're
// about to (or have just) left.
//
// Called by `serverStore` on three lifecycle events:
//   - `logout()`            — user signs out
//   - `deleteServer()`      — active server was deleted
//   - `setActive(otherId)`  — user switched to a different server
//
// The reset is eager so the QueuePanel / PlayerBar / AlbumGrid
// don't briefly render stale entries from the previous server
// before the backend event lands.

import { useLibraryStore } from "@/stores/libraryStore";
import { usePlaybackStore } from "@/stores/playbackStore";
import { useQueueStore } from "@/stores/queueStore";

export function resetSessionState(): void {
  useLibraryStore.getState().reset();
  useQueueStore.getState().clear();
  usePlaybackStore.getState().reset();
}
