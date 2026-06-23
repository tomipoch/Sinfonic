// SmartPlaylistDetailView — evaluates a smart playlist and shows matching tracks.
// Fetches the playlist definition + evaluated tracks in one load.

import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { getSmartPlaylists, evaluateSmartPlaylist, type SmartPlaylist } from "@/lib/tauri";
import type { Track } from "@/types/domain";
import { useServerStore } from "@/stores/serverStore";
import { formatDuration } from "@/lib/format";

export function SmartPlaylistDetailView() {
  const { id } = useParams<{ id: string }>();
  const activeServerId = useServerStore((s) => s.activeServerId);

  const [playlist, setPlaylist] = useState<SmartPlaylist | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    if (!id || !activeServerId) return;
    setLoading(true);
    setError(null);
    try {
      const [pls, trks] = await Promise.all([
        getSmartPlaylists(),
        evaluateSmartPlaylist(id),
      ]);
      const found = pls.find((p) => p.id === id) ?? null;
      setPlaylist(found);
      setTracks(trks);
    } catch (e) {
      setError((e as Error).message ?? String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (activeServerId) void load();
  }, [id, activeServerId]);

  if (!activeServerId) {
    return <p className="text-muted-foreground text-sm p-6">No server connected.</p>;
  }

  if (loading) {
    return <p className="text-muted-foreground text-sm p-6" role="status">Loading…</p>;
  }

  if (error) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-red-900 bg-red-950 p-6 m-6">
        <div className="text-base font-medium text-red-400">Failed to load</div>
        <p className="text-sm text-red-300">{error}</p>
        <button type="button" onClick={() => void load()} className="btn-ghost text-sm">Retry</button>
      </div>
    );
  }

  if (!playlist) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6 m-6">
        <div className="text-base font-medium text-foreground">Smart playlist not found</div>
        <Link to="/smart-playlists" className="text-sm text-primary hover:underline">← Back to smart playlists</Link>
      </div>
    );
  }

  return (
    <section className="flex flex-col gap-6 p-6">
      <header className="flex flex-col gap-2">
        <Link to="/smart-playlists" className="text-xs text-muted hover:text-foreground">← Smart Playlists</Link>
        <h1 className="text-2xl font-semibold">{playlist.name}</h1>
        <div className="text-sm text-muted-foreground">
          {tracks.length} matching {tracks.length === 1 ? "track" : "tracks"}
        </div>
      </header>

      {tracks.length === 0 ? (
        <div className="flex flex-col items-start gap-3 rounded-md border border-border bg-muted p-6">
          <div className="text-base font-medium text-foreground">No matching tracks</div>
          <p className="text-sm text-muted-foreground">
            Try adjusting the rule or value to match more tracks.
          </p>
        </div>
      ) : (
        <ol className="divide-y divide-border rounded-md border border-border">
          {tracks.map((track) => (
            <li
              key={track.id}
              className="grid grid-cols-[2.5rem_1fr_auto] items-center gap-3 px-3 py-2 text-sm"
            >
              <div className="text-right font-mono text-xs text-muted">
                {track.trackNumber || "—"}
              </div>
              <div className="min-w-0">
                <div className="truncate font-medium text-foreground">{track.title}</div>
                <div className="truncate text-xs text-muted-foreground">{track.artist}</div>
              </div>
              <div className="text-xs text-muted">
                {formatDuration(track.durationSeconds)}
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
