import { useCallback, useEffect } from "react";
import { toast } from "sonner";
import { extractError } from "@/lib/errors";
import { next, pause, previous, resume, setMuted, setVolume } from "@/lib/tauri";
import { usePlaybackStore } from "@/stores/playbackStore";

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
  const isPlaying = usePlaybackStore((s) => s.isPlaying);
  const volume = usePlaybackStore((s) => s.volume);
  const muted = usePlaybackStore((s) => s.muted);
  const updateVolume = usePlaybackStore((s) => s.setVolume);
  const updateMuted = usePlaybackStore((s) => s.setMuted);

  const handleKey = useCallback(
    async (e: KeyboardEvent) => {
      if (isEditableTarget(e) || isModifierKey(e)) return;

      switch (e.key) {
        case " ": {
          e.preventDefault();
          try {
            if (isPlaying) {
              await pause();
              usePlaybackStore.getState().setIsPlaying(false);
            } else {
              await resume();
              usePlaybackStore.getState().setIsPlaying(true);
            }
          } catch (err) {
            toast.error(`Playback: ${extractError(err, "unknown error")}`);
          }
          break;
        }

        case "ArrowLeft": {
          e.preventDefault();
          try {
            await previous();
          } catch (err) {
            toast.error(`Previous: ${extractError(err, "unknown error")}`);
          }
          break;
        }

        case "ArrowRight": {
          e.preventDefault();
          try {
            await next();
          } catch (err) {
            toast.error(`Next: ${extractError(err, "unknown error")}`);
          }
          break;
        }

        case "ArrowUp": {
          e.preventDefault();
          const nextVol = Math.min(1, volume + VOLUME_STEP);
          try {
            await setVolume(nextVol);
            updateVolume(nextVol);
          } catch (err) {
            toast.error(`Volume: ${extractError(err, "unknown error")}`);
          }
          break;
        }

        case "ArrowDown": {
          e.preventDefault();
          const nextVol = Math.max(0, volume - VOLUME_STEP);
          try {
            await setVolume(nextVol);
            updateVolume(nextVol);
          } catch (err) {
            toast.error(`Volume: ${extractError(err, "unknown error")}`);
          }
          break;
        }

        case "m":
        case "M": {
          e.preventDefault();
          const nextMuted = !muted;
          try {
            await setMuted(nextMuted);
            updateMuted(nextMuted);
          } catch (err) {
            toast.error(`Mute: ${extractError(err, "unknown error")}`);
          }
          break;
        }
      }
    },
    [isPlaying, volume, muted, updateVolume, updateMuted],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [handleKey]);
}
