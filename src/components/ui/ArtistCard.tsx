// ArtistCard — artist tile used by Home (horizontal), ArtistsView
// list rows, FavoritesView artists section, etc. Width is determined
// by the parent's layout (CSS grid or row container).

import { Link } from "react-router-dom";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { MarqueeText } from "@/components/ui/MarqueeText";
import { cn } from "@/lib/cn";
import type { Artist } from "@/types/domain";

export interface ArtistCardProps {
  artist: Artist;

  /** Tailwind classes for the outer <Link>. */
  className?: string;

  /**
   * Layout shape. "tile" centres the cover and labels (square card
   * for grids); "row" lets the consumer compose the row inline
   * (ArtistsView uses a list-row pattern with `<Avatar>` + text).
   */
  variant?: "tile" | "row";

  /** Hide the album-count sub-line (e.g., on Home where space is tight). */
  hideAlbums?: boolean;

  /** Hide the track-count sub-line. */
  hideTracks?: boolean;
}

export function ArtistCard({
  artist,
  className,
  variant = "tile",
  hideAlbums = false,
  hideTracks = false,
}: ArtistCardProps) {
  if (variant === "row") {
    return (
      <Link
        to={`/artists/${encodeURIComponent(artist.id)}`}
        className={cn(
          "group block min-w-0 outline-none focus-visible:ring-2 focus-visible:ring-primary/40",
          className,
        )}
      >
        <div className="truncate text-sm font-medium text-foreground group-hover:text-primary">
          <MarqueeText>{artist.name}</MarqueeText>
        </div>
        {(hideAlbums || hideTracks) === false ? (
          <div className="truncate text-xs text-muted-foreground">
            <MarqueeText>
              {hideTracks
                ? `${artist.albumCount} albums`
                : hideAlbums
                  ? `${artist.trackCount} tracks`
                  : `${artist.trackCount} tracks · ${artist.albumCount} albums`}
            </MarqueeText>
          </div>
        ) : null}
      </Link>
    );
  }

  return (
    <Link
      to={`/artists/${encodeURIComponent(artist.id)}`}
      className={cn(
        "group flex flex-col items-center gap-2 rounded-md p-1 outline-none transition-colors hover:bg-card focus-visible:ring-2 focus-visible:ring-primary/40",
        className,
      )}
    >
      <AlbumCover
        source={artist}
        initial={artist.name.charAt(0).toUpperCase()}
        className="aspect-square w-full rounded-full shadow-sm"
        ariaLabel={`Cover art for ${artist.name}`}
      />
      <div className="min-w-0 text-center">
        <div className="text-sm font-medium text-foreground group-hover:text-primary">
          <MarqueeText>{artist.name}</MarqueeText>
        </div>
        <div className="text-xs text-muted-foreground">
          <MarqueeText>
            {hideTracks
              ? `${artist.albumCount} albums`
              : hideAlbums
                ? `${artist.trackCount} tracks`
                : `${artist.trackCount} tracks`}
          </MarqueeText>
        </div>
      </div>
    </Link>
  );
}
