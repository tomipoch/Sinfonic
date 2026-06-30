// `PlaybackProvider` — wraps the app so any component can read or
// command playback via `usePlaybackContext()`.
//
// The provider owns a single `usePlayback()` instance. Without it
// every consumer would mount its own subscription, which means
// four listeners for `track-changed` and four polling timers per
// page. Centralising the hook behind a context keeps the IPC
// fan-out to one.
//
// On mount, the provider registers a reset callback with
// `lifecycle/resetSession` so the server store can wipe the
// playback snapshot on logout / server-switch without needing a
// direct context reference.

import { createContext, type ReactNode, useContext, useEffect } from "react";

import { registerPlaybackReset } from "@/lifecycle/resetSession";

import { type PlaybackControls, usePlayback } from "./usePlayback";

const PlaybackContext = createContext<PlaybackControls | null>(null);

export function PlaybackProvider({ children }: { children: ReactNode }) {
  const controls = usePlayback();
  useEffect(() => registerPlaybackReset(controls.reset), [controls.reset]);
  return <PlaybackContext.Provider value={controls}>{children}</PlaybackContext.Provider>;
}

export function usePlaybackContext(): PlaybackControls {
  const ctx = useContext(PlaybackContext);
  if (!ctx) {
    throw new Error("usePlaybackContext must be used inside <PlaybackProvider>");
  }
  return ctx;
}
