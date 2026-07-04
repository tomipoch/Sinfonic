// PlaylistArt — visual representation of a playlist.
//
// Renders a 2x2 mosaic of the first four track covers (when the
// playlist has no explicit image of its own). Falls back to a
// gradient with a music-note glyph when there are no track covers
// to draw from. Used by PlaylistsView (grid) and PlaylistDetailView
// (hero).

import { AlbumCover } from "@/components/ui/AlbumCover";
import { cn } from "@/lib/cn";
import type { ImageRef, Playlist, Track } from "@/types/domain";

export interface PlaylistArtProps {
  playlist: Playlist;
  /** First few tracks used to synthesise the mosaic when there's no playlist cover. */
  previewTracks?: Track[];
  className?: string;
  /** Override the rendered <Playlist.ImageRef>. Falls back to playlist.imageRef. */
  imageRef?: ImageRef | null;
}

export function PlaylistArt({
  playlist,
  previewTracks = [],
  className,
  imageRef,
}: PlaylistArtProps) {
  const cover = imageRef ?? playlist.imageRef ?? null;

  if (cover) {
    return (
      <AlbumCover
        source={{ id: playlist.id, title: playlist.name, imageRef: cover }}
        className={cn("aspect-square w-full rounded-md shadow-sm", className)}
        ariaLabel={`Cover art for ${playlist.name}`}
      />
    );
  }

  const tiles = (previewTracks ?? []).slice(0, 4);

  if (tiles.length === 0) {
    return (
      <div
        className={cn(
          "flex aspect-square w-full items-center justify-center rounded-md bg-gradient-to-br from-secondary to-muted text-5xl text-white/30 shadow-sm",
          className,
        )}
        aria-label={`Cover art for ${playlist.name}`}
      >
        <span aria-hidden>♪</span>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "grid aspect-square w-full grid-cols-2 grid-rows-2 gap-1 overflow-hidden rounded-md bg-muted shadow-sm",
        className,
      )}
      aria-label={`Cover art for ${playlist.name}`}
    >
      {Array.from({ length: 4 }).map((_, idx) => {
        const t = tiles[idx];
        if (!t) {
          return (
            <div
              key={`empty-${idx}`}
              className="bg-gradient-to-br from-secondary to-muted"
              aria-hidden
            />
          );
        }
        return (
          <div key={t.id ?? idx} className="overflow-hidden bg-secondary">
            <AlbumCover
              source={{
                id: t.id ?? t.albumId,
                title: t.album || t.title,
                imageRef: t.imageRef,
              }}
              className="h-full w-full rounded-none shadow-none ring-0"
              ariaLabel={`Cover art for ${t.album}`}
            />
          </div>
        );
      })}
    </div>
  );
}
