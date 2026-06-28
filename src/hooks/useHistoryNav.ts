// useHistoryNav — track the router's history stack so back/forward
// reflect both programmatic navigations (`<Link>`, `useNavigate`) and
// OS-level back/forward shortcuts (browser buttons, swipe gestures).
//
// react-router v6 does not expose the history stack directly, so we
// reconstruct it by observing `location.key` changes:
//
//   * If a previously-unseen key arrives, the user pushed (via Link or
//     `useNavigate(n)`). The previous key is added to `back`, and
//     `forward` is cleared.
//   * If a previously-seen key arrives, the user popped (via back/forward
//     button, swipe, or `navigate(-1/+1)`). The current key is pushed
//     onto the opposite stack.

import { useCallback, useEffect, useRef, useState } from "react";
import { useLocation } from "react-router-dom";

export type HistoryNav = {
  goBack: () => void;
  goForward: () => void;
  canGoBack: boolean;
  canGoForward: boolean;
};

export function useHistoryNav(): HistoryNav {
  const location = useLocation();

  const backRef = useRef<string[]>([]);
  const seenRef = useRef<Set<string>>(new Set());
  const forwardRef = useRef<string[]>([]);
  const lastKeyRef = useRef<string>(location.key);

  const [canGoBack, setCanGoBack] = useState(false);
  const [canGoForward, setCanGoForward] = useState(false);

  const sync = useCallback(() => {
    setCanGoBack(backRef.current.length > 0);
    setCanGoForward(forwardRef.current.length > 0);
  }, []);

  useEffect(() => {
    const currentKey = location.key;
    const previousKey = lastKeyRef.current;

    if (currentKey === previousKey) return;

    if (!seenRef.current.has(currentKey)) {
      // New key → push. Record the previous entry on the back stack and
      // discard any forward history.
      if (previousKey !== "default") {
        backRef.current.push(previousKey);
      }
      forwardRef.current = [];
      seenRef.current.add(currentKey);
    } else if (backRef.current[backRef.current.length - 1] === currentKey) {
      // Going back: move the previous (current) entry to forward.
      backRef.current.pop();
      forwardRef.current.push(previousKey);
    } else {
      // Going forward: move the entry from forward back to back.
      const idx = forwardRef.current.indexOf(currentKey);
      if (idx >= 0) forwardRef.current.splice(idx, 1);
      if (previousKey !== "default") {
        backRef.current.push(previousKey);
      }
    }

    lastKeyRef.current = currentKey;
    sync();
  }, [location, sync]);

  const goBack = useCallback(() => {
    if (backRef.current.length === 0) return;
    // Dispatch a popstate-like back by using the History API. We can't
    // call `useNavigate(-1)` here directly because we don't have it,
    // and a popstate would be a no-op. Instead, mutate `window.history`
    // to trigger react-router to pick up the new location. The simplest
    // portable path: use the back method, which fires `popstate` and
    // updates the location.
    window.history.back();
  }, []);

  const goForward = useCallback(() => {
    if (forwardRef.current.length === 0) return;
    window.history.forward();
  }, []);

  return { goBack, goForward, canGoBack, canGoForward };
}
