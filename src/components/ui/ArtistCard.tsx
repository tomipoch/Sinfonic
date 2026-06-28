// ArtistCard — compact artist tile for horizontal sections in Home.

import { Link } from "react-router-dom";

import { AlbumCover } from "@/components/ui/AlbumCover";
import type { Artist } from "@/types/domain";

type Props = {
  artist: Artist;
  className?: string;
};

export function ArtistCard({ artist, className }: Props) {
  return (
    <Link
      to={`/artists/${encodeURIComponent(artist.id)}`}
      className={`group flex w-36 shrink-0 flex-col items-center gap-2 ${className ?? ""}`}
    >
      <AlbumCover
        source={artist}
        initial={artist.name.charAt(0).toUpperCase()}
        className="h-36 w-36 rounded-full"
        ariaLabel={`Cover art for ${artist.name}`}
      />
      <div className="min-w-0 text-center">
        <div className="truncate text-sm font-medium text-foreground group-hover:text-primary">
          {artist.name}
        </div>
        <div className="truncate text-xs text-muted-foreground">{artist.albumCount} albums</div>
      </div>
    </Link>
  );
}
