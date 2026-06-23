// FavoriteButton — heart toggle for tracks/albums/artists.
// Fires the set_favorite IPC call and updates locally.

import { useState } from "react";
import { toast } from "sonner";

import {
  setTrackFavorite,
  setAlbumFavorite,
  setArtistFavorite,
} from "@/lib/tauri";

type FavoriteKind = "track" | "album" | "artist";

interface FavoriteButtonProps {
  kind: FavoriteKind;
  itemId: string;
  initialFavorite: boolean;
  onToggle?: (newValue: boolean) => void;
}

export function FavoriteButton({ kind, itemId, initialFavorite, onToggle }: FavoriteButtonProps) {
  const [favorited, setFavorited] = useState(initialFavorite);
  const [busy, setBusy] = useState(false);

  const toggle = async () => {
    if (busy) return;
    const next = !favorited;
    setFavorited(next);
    setBusy(true);
    try {
      if (kind === "track") await setTrackFavorite(itemId, next);
      else if (kind === "album") await setAlbumFavorite(itemId, next);
      else await setArtistFavorite(itemId, next);
      onToggle?.(next);
    } catch (e) {
      setFavorited(!next);
      toast.error(`Couldn't update favorite: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <button
      type="button"
      onClick={(e) => { e.stopPropagation(); void toggle(); }}
      disabled={busy}
      aria-label={favorited ? "Remove from favorites" : "Add to favorites"}
      aria-pressed={favorited}
      className="rounded-md p-1 text-fg-muted transition-colors hover:text-red-400 focus:outline-none disabled:opacity-40"
    >
      {favorited ? "♥" : "♡"}
    </button>
  );
}
