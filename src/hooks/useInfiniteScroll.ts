// useInfiniteScroll — IntersectionObserver-driven "load more" trigger.
//
// Mount `<div ref={ref} />` as a sentinel at the end of the list. When
// the sentinel enters the viewport (with a small `rootMargin` buffer)
// the callback fires once. The hook re-arms on every intersection,
// so a sentinel that's permanently visible (e.g. short list) keeps
// firing — `loadMore` callers must guard against re-entry themselves
// (the `loadingMoreX` flags on `libraryStore` already do this).
//
// Returns `null` on the server / non-DOM environments (the ref will
// simply never observe, so callers don't have to render the sentinel
// at all in those cases).

import { useEffect, useRef } from "react";

export interface UseInfiniteScrollOptions {
  /** Called when the sentinel intersects the viewport. */
  onIntersect: () => void;
  /** Disable the observer (e.g. when there's nothing more to load). */
  enabled: boolean;
  /** Px margin around the root used to start loading before the sentinel is on-screen. Default: "200px". */
  rootMargin?: string;
  /** Visibility ratio required to fire. Default: 0 (any pixel). */
  threshold?: number | number[];
}

export function useInfiniteScroll<T extends HTMLElement>({
  onIntersect,
  enabled,
  rootMargin = "200px",
  threshold = 0,
}: UseInfiniteScrollOptions): React.RefObject<T | null> {
  const ref = useRef<T | null>(null);
  const callbackRef = useRef(onIntersect);
  useEffect(() => {
    callbackRef.current = onIntersect;
  }, [onIntersect]);

  useEffect(() => {
    if (!enabled) return undefined;
    const node = ref.current;
    if (!node || typeof IntersectionObserver === "undefined") return undefined;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            callbackRef.current();
            break;
          }
        }
      },
      { rootMargin, threshold },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [enabled, rootMargin, threshold]);

  return ref;
}
