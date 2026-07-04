// PlaylistCard — playlist tile for the All Playlists grid.
// Mirrors AlbumCard's layout so the two screens feel visually
// parallel.

import { Link } from "react-router-dom";

import { MarqueeText } from "@/components/ui/MarqueeText";
import { PlaylistArt } from "@/components/ui/PlaylistArt";
import { cn } from "@/lib/cn";
import { formatDuration } from "@/lib/format";
import type { Playlist, Track } from "@/types/domain";

export interface PlaylistCardProps {
  playlist: Playlist;
  previewTracks?: Track[];
  className?: string;
}

export function PlaylistCard({ playlist, previewTracks, className }: PlaylistCardProps) {
  return (
    <Link
      to={`/playlists/${encodeURIComponent(playlist.id)}`}
      className={cn(
        "group flex flex-col gap-2 rounded-md p-1 outline-none transition-colors hover:bg-card focus-visible:ring-2 focus-visible:ring-primary/40",
        className,
      )}
    >
      <PlaylistArt playlist={playlist} previewTracks={previewTracks} />
      <div className="min-w-0">
        <div className="text-sm font-medium text-foreground group-hover:text-primary">
          <MarqueeText>{playlist.name}</MarqueeText>
        </div>
        <div className="text-xs text-muted-foreground">
          {playlist.trackCount} tracks · {formatDuration(playlist.durationSeconds)}
        </div>
      </div>
    </Link>
  );
}
