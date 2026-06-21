// Auto-load library when the active server changes.
//
// The library cache is server-scoped (`active_server_id`), so a
// disconnect must clear the in-memory copy and a connect must
// repopulate it. Centralising the effect in one hook keeps every
// tab consistent — they all subscribe to the same store snapshot.

import { useEffect } from "react";

import { useLibraryStore } from "../stores/libraryStore";
import { useServerStore } from "../stores/serverStore";

export function useLibraryAutoLoad(): void {
  const activeServerId = useServerStore((s) => s.activeServerId);
  const loadAll = useLibraryStore((s) => s.loadAll);
  const reset = useLibraryStore((s) => s.reset);

  useEffect(() => {
    if (activeServerId) {
      void loadAll();
    } else {
      reset();
    }
  }, [activeServerId, loadAll, reset]);
}
