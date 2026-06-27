// FavoriteButton — heart toggle for tracks/albums/artists.
//
// Uses React 19's `useOptimistic` to flip the heart instantly while
// the IPC round-trip is in flight; if the call fails the optimistic
// state is reverted automatically once the transition settles.

import { useOptimistic, useTransition } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import {
  setTrackFavorite,
  setAlbumFavorite,
  setArtistFavorite,
} from "@/lib/tauri";
import { extractError } from "@/lib/errors";

type FavoriteKind = "track" | "album" | "artist";

interface FavoriteButtonProps {
  kind: FavoriteKind;
  itemId: string;
  initialFavorite: boolean;
  onToggle?: (newValue: boolean) => void;
}

export function FavoriteButton({ kind, itemId, initialFavorite, onToggle }: FavoriteButtonProps) {
  const [optimisticFavorited, setOptimisticFavorited] = useOptimistic(initialFavorite);
  const [, startTransition] = useTransition();

  const toggle = () => {
    const next = !optimisticFavorited;
    startTransition(async () => {
      setOptimisticFavorited(next);
      try {
        if (kind === "track") await setTrackFavorite(itemId, next);
        else if (kind === "album") await setAlbumFavorite(itemId, next);
        else await setArtistFavorite(itemId, next);
        onToggle?.(next);
      } catch (e) {
        setOptimisticFavorited(!next);
        toast.error(`Couldn't update favorite: ${extractError(e, "unknown error")}`);
      }
    });
  };

  return (
    <button
      type="button"
      onClick={(e) => { e.stopPropagation(); toggle(); }}
      aria-label={optimisticFavorited ? "Remove from favorites" : "Add to favorites"}
      aria-pressed={optimisticFavorited}
      className={cn(
        "rounded-md p-1 transition-colors focus:outline-none disabled:opacity-40",
        optimisticFavorited ? "text-red-500 hover:text-red-400" : "text-fg-muted hover:text-red-400",
      )}
    >
      {optimisticFavorited ? "♥" : "♡"}
    </button>
  );
}
