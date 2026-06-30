// Public surface of the playback module.
//
// `src/playback/` is a self-contained mini-app that owns playback
// state, IPC commands, and the React context that wraps both. The
// `src/lib/repeat.ts` file is kept as a thin re-export for legacy
// callers (`QueueView`, `QueuePanel`) that haven't migrated yet.

export {
  PlaybackProvider,
  usePlaybackContext,
} from "./PlaybackContext";
export { nextRepeat, REPEAT_CYCLE, repeatLabel } from "./repeat";
export type { PlaybackSnapshot } from "./types";
export { DEFAULT_SNAPSHOT } from "./types";
export type { PlaybackControls } from "./usePlayback";
export { usePlayback } from "./usePlayback";
