// PlayerBar — fixed bottom bar. Phase 0 placeholder; real implementation
// (transport controls, seek bar, volume) lands in Phase 5.

import { usePlaybackStore } from "../../stores/playbackStore";

export function PlayerBar() {
  const track = usePlaybackStore((s) => s.currentTrack);
  const isPlaying = usePlaybackStore((s) => s.isPlaying);

  return (
    <footer className="flex h-16 items-center justify-between border-t border-bg-raised bg-bg-subtle px-4">
      <div className="flex min-w-0 items-center gap-3">
        <div className="h-10 w-10 rounded bg-bg-raised" aria-hidden />
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-fg">
            {track?.title ?? "Nothing playing"}
          </div>
          <div className="truncate text-xs text-fg-subtle">
            {track?.artist ?? "—"}
          </div>
        </div>
      </div>
      <div className="flex items-center gap-2 text-xs text-fg-muted">
        {isPlaying ? "Playing" : "Paused"}
      </div>
    </footer>
  );
}
