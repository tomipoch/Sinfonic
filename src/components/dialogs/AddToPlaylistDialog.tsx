// AddToPlaylistDialog — pick an existing playlist or create a new one
// to add tracks to. Renders as a centered modal with a backdrop.
//
// The list of existing playlists is fetched on mount. The input at
// the bottom lets the caller type a name and create a brand-new
// playlist containing the supplied `trackIds` in one call.

import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, MusicNoteSquare01Icon } from "@hugeicons/core-free-icons";

import {
  addPlaylistTracks,
  createPlaylist,
  playlistsGet,
} from "@/lib/tauri";
import { usePlaylistsStore } from "@/stores/playlistsStore";
import { extractError } from "@/lib/errors";
import type { Playlist } from "@/types/domain";

interface AddToPlaylistDialogProps {
  trackIds: string[];
  onClose: () => void;
}

export function AddToPlaylistDialog({ trackIds, onClose }: AddToPlaylistDialogProps) {
  const [playlists, setPlaylists] = useState<Playlist[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Touch the store so the host view's list refreshes after we add.
  const loadPlaylistsStore = usePlaylistsStore((s) => s.loadPlaylists);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    playlistsGet()
      .then((list) => {
        if (cancelled) return;
        setPlaylists(list);
        setLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(extractError(err, "unknown error"));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Close on Esc.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const onPick = async (playlist: Playlist) => {
    if (busy) return;
    setBusy(true);
    try {
      await addPlaylistTracks(playlist.id, trackIds);
      toast.success(
        `Added ${trackIds.length} track${trackIds.length !== 1 ? "s" : ""} to "${playlist.name}"`,
      );
      void loadPlaylistsStore();
      onClose();
    } catch (err) {
      toast.error(`Couldn't add: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  const onCreate = async () => {
    const name = newName.trim();
    if (!name || busy) return;
    setBusy(true);
    try {
      await createPlaylist(name, trackIds);
      toast.success(`Created "${name}" with ${trackIds.length} track${trackIds.length !== 1 ? "s" : ""}`);
      void loadPlaylistsStore();
      onClose();
    } catch (err) {
      toast.error(`Couldn't create: ${extractError(err, "unknown error")}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Add to playlist"
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md overflow-hidden rounded-lg border border-border bg-card shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center justify-between gap-2 border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <HugeiconsIcon
              icon={MusicNoteSquare01Icon}
              size={18}
              strokeWidth={1.75}
              className="text-primary"
            />
            <h2 className="text-base font-semibold text-foreground">Add to playlist</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground focus:outline-none"
          >
            ✕
          </button>
        </header>

        <div className="max-h-80 overflow-y-auto p-2">
          {loading && (
            <p className="px-3 py-6 text-center text-sm text-muted-foreground" role="status">
              Loading playlists…
            </p>
          )}
          {error && (
            <p className="px-3 py-6 text-center text-sm text-red-400">{error}</p>
          )}
          {playlists && playlists.length === 0 && (
            <p className="px-3 py-6 text-center text-sm text-muted-foreground">
              No playlists yet — create one below.
            </p>
          )}
          {playlists && playlists.length > 0 && (
            <ul className="flex flex-col gap-0.5">
              {playlists.map((pl) => (
                <li key={pl.id}>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void onPick(pl)}
                    className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition-colors hover:bg-muted focus:outline-none focus-visible:bg-muted disabled:opacity-50"
                  >
                    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded bg-primary/15 text-primary">
                      <HugeiconsIcon icon={MusicNoteSquare01Icon} size={14} strokeWidth={1.75} />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate font-medium text-foreground">
                        {pl.name}
                      </span>
                      <span className="block text-xs text-muted-foreground">
                        {pl.trackCount} track{pl.trackCount !== 1 ? "s" : ""}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="flex items-center gap-2 border-t border-border bg-background/40 px-3 py-3">
          <HugeiconsIcon
            icon={Add01Icon}
            size={16}
            strokeWidth={1.75}
            className="shrink-0 text-muted-foreground"
          />
          <input
            ref={inputRef}
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void onCreate();
            }}
            placeholder="New playlist name"
            disabled={busy}
            className="flex-1 rounded-md border border-border bg-muted px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none disabled:opacity-50"
          />
          <button
            type="button"
            onClick={() => void onCreate()}
            disabled={busy || !newName.trim()}
            className="btn-primary text-sm disabled:opacity-50"
          >
            Create
          </button>
        </div>
      </div>
    </div>
  );
}