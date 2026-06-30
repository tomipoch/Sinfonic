// Typed wrappers around the playback IPC surface.
//
// These thin re-exports from `@/lib/tauri` exist so the playback
// module owns its command vocabulary. Components in `src/playback/`
// should `import { togglePlay, seekTo, ... }` from here, not from
// `@/lib/tauri`, so the surface can change without rippling through
// every consumer.

import {
  next as ipcNext,
  pause as ipcPause,
  playAlbum as ipcPlayAlbum,
  playTrack as ipcPlayTrack,
  previous as ipcPrevious,
  queueAddMany as ipcQueueAddMany,
  resume as ipcResume,
  seek as ipcSeek,
  setMuted as ipcSetMuted,
  setRepeat as ipcSetRepeat,
  setShuffle as ipcSetShuffle,
  setVolume as ipcSetVolume,
  stop as ipcStop,
} from "@/lib/tauri";

import type { RepeatMode, Track } from "@/types/domain";

export async function pause(): Promise<void> {
  await ipcPause();
}

export async function resume(): Promise<void> {
  await ipcResume();
}

export async function stop(): Promise<void> {
  await ipcStop();
}

export async function next(): Promise<void> {
  await ipcNext();
}

export async function previous(): Promise<void> {
  await ipcPrevious();
}

export async function seek(positionSeconds: number): Promise<void> {
  await ipcSeek(positionSeconds);
}

export async function setVolume(volume: number): Promise<void> {
  await ipcSetVolume(volume);
}

export async function setMuted(muted: boolean): Promise<void> {
  await ipcSetMuted(muted);
}

export async function setRepeat(repeat: RepeatMode): Promise<void> {
  await ipcSetRepeat(repeat);
}

export async function setShuffle(enabled: boolean): Promise<void> {
  await ipcSetShuffle(enabled);
}

export async function playTrack(track: Track): Promise<string> {
  return ipcPlayTrack(track);
}

export async function playAlbum(tracks: Track[]): Promise<void> {
  await ipcPlayAlbum(tracks);
}

export async function queueAddMany(tracks: Track[]): Promise<string[]> {
  return ipcQueueAddMany(tracks);
}
