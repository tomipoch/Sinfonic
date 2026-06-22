import { useEffect, useCallback } from "react";
import { toast } from "sonner";
import {
  next,
  pause,
  previous,
  resume,
  setMuted,
  setVolume,
} from "../lib/tauri";
import { usePlaybackStore } from "../stores/playbackStore";

const VOLUME_STEP = 0.05;

function isEditableTarget(e: KeyboardEvent): boolean {
  const target = e.target as HTMLElement;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
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
              usePlaybackStore.getState().setState({ ...usePlaybackStore.getState(), isPlaying: false });
            } else {
              await resume();
              usePlaybackStore.getState().setState({ ...usePlaybackStore.getState(), isPlaying: true });
            }
          } catch (err) {
            toast.error(`Playback: ${(err as Error).message}`);
          }
          break;
        }

        case "ArrowLeft": {
          e.preventDefault();
          try {
            await previous();
          } catch (err) {
            toast.error(`Previous: ${(err as Error).message}`);
          }
          break;
        }

        case "ArrowRight": {
          e.preventDefault();
          try {
            await next();
          } catch (err) {
            toast.error(`Next: ${(err as Error).message}`);
          }
          break;
        }

        case "ArrowUp": {
          e.preventDefault();
          const nextVol = Math.min(1, volume + VOLUME_STEP);
          try {
            await setVolume(nextVol);
            setVolume(nextVol);
          } catch (err) {
            toast.error(`Volume: ${(err as Error).message}`);
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
            toast.error(`Volume: ${(err as Error).message}`);
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
            toast.error(`Mute: ${(err as Error).message}`);
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
