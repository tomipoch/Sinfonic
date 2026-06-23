// AlbumCard — compact album tile for horizontal sections in Home.

import { Link } from "react-router-dom";
import { AlbumCover } from "@/components/ui/AlbumCover";
import type { Album } from "@/types/domain";

type Props = {
  album: Album;
  className?: string;
};

export function AlbumCard({ album, className }: Props) {
  return (
    <Link
      to={`/library/album/${encodeURIComponent(album.id)}`}
      className={`group flex w-40 shrink-0 flex-col gap-2 ${className ?? ""}`}
    >
      <AlbumCover album={album} className="aspect-square w-full rounded-md" />
      <div className="min-w-0">
        <div className="truncate text-sm font-medium text-foreground group-hover:text-primary">
          {album.title}
        </div>
        <div className="truncate text-xs text-muted-foreground">
          {album.artist}
        </div>
      </div>
    </Link>
  );
}
