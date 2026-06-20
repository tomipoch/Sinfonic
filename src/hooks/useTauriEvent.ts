// Typed wrapper around Tauri's `listen` for use inside React effects.
//
// Why: `listen` returns an unlisten function, has its own setup state,
// and uses a generic for the payload type. Without this hook every
// effect would re-implement the same boilerplate.

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
        fn();
      }
    });

    return () => {
      active = false;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
};
