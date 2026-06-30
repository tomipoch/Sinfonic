// Now-playing section of the PlayerBar.
//
// Cover art + title + artist. Pulls the current track identity from
// the playback context, and the cover art from the library cache so
// we don't re-fetch what's already in memory.

import { LeftToRightListBulletIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useMemo } from "react";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { cn } from "@/lib/cn";
import { usePlaybackContext } from "@/playback";
import { useLibraryStore } from "@/stores/libraryStore";
import type { Album, Track } from "@/types/domain";

interface Cover {
  id: string;
  title: string;
  imageRef: NonNullable<Track["imageRef"]>;
}

function findCover(
  albums: Album[],
  tracks: Track[],
  currentTrack: { trackId: string } | null,
): Cover | null {
  if (!currentTrack) return null;
  const albumById = new Map(albums.map((a) => [a.id, a]));
  const trackById = new Map(tracks.map((t) => [t.id, t]));
  const full = trackById.get(currentTrack.trackId as Track["id"]);
  if (full?.imageRef) {
    return { id: full.id, title: full.album || full.title, imageRef: full.imageRef };
  }
  if (full) {
    const album = albumById.get(full.albumId);
    if (album?.imageRef) {
      return { id: full.id, title: album.title, imageRef: album.imageRef };
    }
  }
  return null;
}

export function NowPlaying() {
  const { snapshot } = usePlaybackContext();
  const { currentTrack } = snapshot;
  const albums = useLibraryStore((s) => s.albums);
  const tracks = useLibraryStore((s) => s.tracks);
  const cover = useMemo(
    () => findCover(albums, tracks, currentTrack),
    [albums, tracks, currentTrack],
  );
  const hasTrack = currentTrack !== null;

  return (
    <div className="flex min-w-0 flex-1 items-center gap-3.5">
      <div className="h-14 w-14 shrink-0" aria-hidden={!cover}>
        {cover ? (
          <AlbumCover
            source={cover}
            ariaLabel={`Cover art for ${cover.title}`}
            className="h-14 w-14 rounded-md shadow-sm ring-1 ring-inset ring-border/40"
          />
        ) : (
          <div className="flex h-14 w-14 items-center justify-center rounded-md bg-gradient-to-br from-secondary to-muted ring-1 ring-inset ring-border/60">
            <HugeiconsIcon
              icon={LeftToRightListBulletIcon}
              size={20}
              strokeWidth={1.5}
              className="text-muted-foreground/70"
            />
          </div>
        )}
      </div>
      <div className="flex min-w-0 flex-col gap-0.5">
        <div
          className={cn(
            "truncate text-sm font-semibold tracking-tight",
            hasTrack ? "text-foreground" : "text-muted-foreground",
          )}
          title={currentTrack?.title}
        >
          {currentTrack?.title ?? "Nothing playing"}
        </div>
        <div className="truncate text-xs text-muted-foreground" title={currentTrack?.artist}>
          {currentTrack?.artist ?? "—"}
        </div>
      </div>
    </div>
  );
}
