import { useCallback, useEffect } from "react";
import { toast } from "sonner";
import { extractError } from "@/lib/errors";
import { usePlaybackContext } from "@/playback";

const VOLUME_STEP = 0.05;

function isEditableTarget(e: KeyboardEvent): boolean {
  const target = e.target as HTMLElement;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
}

function isModifierKey(e: KeyboardEvent): boolean {
  return e.ctrlKey || e.metaKey || e.altKey;
}

export function useKeyboardShortcuts() {
  const { snapshot, togglePlay, next, previous, setVolume, setMuted } = usePlaybackContext();

  const handleKey = useCallback(
    async (e: KeyboardEvent) => {
      if (isEditableTarget(e) || isModifierKey(e)) return;

      try {
        switch (e.key) {
          case " ": {
            e.preventDefault();
            await togglePlay();
            return;
          }
          case "ArrowLeft": {
            e.preventDefault();
            await previous();
            return;
          }
          case "ArrowRight": {
            e.preventDefault();
            await next();
            return;
          }
          case "ArrowUp": {
            e.preventDefault();
            await setVolume(Math.min(1, snapshot.volume + VOLUME_STEP));
            return;
          }
          case "ArrowDown": {
            e.preventDefault();
            await setVolume(Math.max(0, snapshot.volume - VOLUME_STEP));
            return;
          }
          case "m":
          case "M": {
            e.preventDefault();
            await setMuted(!snapshot.muted);
            return;
          }
        }
      } catch (err) {
        toast.error(`Playback: ${extractError(err, "unknown error")}`);
      }
    },
    [snapshot.volume, snapshot.muted, togglePlay, previous, next, setVolume, setMuted],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [handleKey]);
}
