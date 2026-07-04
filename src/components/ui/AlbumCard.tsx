// AlbumCard — album tile used by Home (horizontal), AlbumsView and
// GenreDetailView (grid). Width is determined by the parent's
// layout (CSS grid columns or horizontal flex); the component itself
// takes the full width of its slot.

import { Link } from "react-router-dom";

import { AlbumCover } from "@/components/ui/AlbumCover";
import { MarqueeText } from "@/components/ui/MarqueeText";
import { cn } from "@/lib/cn";
import { formatDuration } from "@/lib/format";
import type { Album } from "@/types/domain";

export interface AlbumCardProps {
  album: Album;

  /** Tailwind classes for the outer <Link>. */
  className?: string;

  /** Show the album year on the artist line. Defaults to true. */
  showYear?: boolean;

  /** Show "{n} tracks · {duration}" line. Defaults to true in grid contexts. */
  showMeta?: boolean;
}

export function AlbumCard({ album, className, showYear = true, showMeta = true }: AlbumCardProps) {
  return (
    <Link
      to={`/albums/${encodeURIComponent(album.id)}`}
      className={cn(
        "group flex flex-col gap-2 rounded-md p-1 outline-none transition-colors hover:bg-card focus-visible:ring-2 focus-visible:ring-primary/40",
        className,
      )}
    >
      <AlbumCover source={album} className="aspect-square w-full rounded-md shadow-sm" />
      <div className="min-w-0">
        <div className="text-sm font-medium text-foreground group-hover:text-primary">
          <MarqueeText>{album.title}</MarqueeText>
        </div>
        <div className="text-xs text-muted-foreground">
          <MarqueeText>
            {album.artist}
            {showYear && album.year ? ` · ${album.year}` : ""}
          </MarqueeText>
        </div>
        {showMeta && (
          <div className="text-xs text-muted-foreground/80">
            {album.trackCount} tracks · {formatDuration(album.durationSeconds)}
          </div>
        )}
      </div>
    </Link>
  );
}
