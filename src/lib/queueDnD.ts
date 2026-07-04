// DnD data shape for track drag operations.
import type { Track } from "@/types/domain";

export interface TrackDragData {
  tracks: Track[];
  source:
    | "albums-tab"
    | "album-detail"
    | "tracks-tab"
    | "songs-view"
    | "playlist-detail"
    | "smart-playlist-detail"
    | "favorites"
    | "favorites-tracks"
    | "queue"
    | "genre-detail"
    | "search"
    | "artist-top"
    | "artist-featured";
}

export const DND_MIME_TYPE = "application/json";

export function encodeDragData(data: TrackDragData): string {
  return JSON.stringify(data);
}

export function decodeDragData(raw: string): TrackDragData | null {
  try {
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed.tracks) && typeof parsed.source === "string") {
      return parsed as TrackDragData;
    }
    return null;
  } catch {
    return null;
  }
}
