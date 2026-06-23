// ArtistCard — compact artist tile for horizontal sections in Home.

import { Link } from "react-router-dom";
import type { Artist } from "@/types/domain";

type Props = {
  artist: Artist;
  className?: string;
};

export function ArtistCard({ artist, className }: Props) {
  return (
    <Link
      to={`/library/artist/${encodeURIComponent(artist.id)}`}
      className={`group flex w-36 shrink-0 flex-col items-center gap-2 ${className ?? ""}`}
    >
      <div className="flex h-36 w-36 items-center justify-center rounded-full bg-card text-4xl font-bold text-muted-foreground">
        {artist.name.charAt(0).toUpperCase()}
      </div>
      <div className="min-w-0 text-center">
        <div className="truncate text-sm font-medium text-foreground group-hover:text-primary">
          {artist.name}
        </div>
        <div className="truncate text-xs text-muted-foreground">
          {artist.albumCount} albums
        </div>
      </div>
    </Link>
  );
}
