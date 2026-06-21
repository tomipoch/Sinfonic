// Album cover renderer.
//
// Tries to fetch real cover bytes via `provider_image_bytes` (which
// is read-through cached on disk in the album_art cache). On any
// failure — no active provider, no imageRef, network error — falls
// back to a deterministic gradient seeded by the album id so the
// grid still looks distinct without external assets.
//
// The blob URL is created from a Uint8Array (the IPC payload comes
// back as a plain number[]) and revoked on unmount to free the
// browser-side blob handle.

import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { providerImageBytes } from "../../lib/tauri";
import type { Album } from "../../types/domain";

const PALETTE: ReadonlyArray<readonly [number, number]> = [
  [260, 25],
  [220, 30],
  [180, 28],
  [140, 32],
  [40, 30],
  [340, 30],
  [10, 28],
  [80, 28],
];

function hash(input: string): number {
  let h = 5381;
  for (let i = 0; i < input.length; i += 1) {
    h = ((h << 5) + h + input.charCodeAt(i)) | 0;
  }
  return Math.abs(h);
}

interface AlbumCoverProps {
  album: Pick<Album, "id" | "title" | "imageRef">;
  className?: string;
}

export function AlbumCover({ album, className }: AlbumCoverProps) {
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const itemId = album.imageRef?.itemId;
    if (!itemId) {
      setBlobUrl(null);
      setFailed(false);
      return;
    }
    let cancelled = false;
    setFailed(false);

    (async () => {
      try {
        const res = await providerImageBytes(itemId, album.imageRef?.tag ?? null);
        if (cancelled) return;
        const bytes = new Uint8Array(res.bytes);
        const blob = new Blob([bytes], { type: res.contentType });
        const url = URL.createObjectURL(blob);
        setBlobUrl(url);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();

    return () => {
      cancelled = true;
      setBlobUrl((current) => {
        if (current) URL.revokeObjectURL(current);
        return null;
      });
    };
  }, [album.id, album.imageRef?.itemId, album.imageRef?.tag]);

  const seed = hash(album.id);
  const slot = PALETTE[seed % PALETTE.length] ?? PALETTE[0]!;
  const baseHue = slot[0];
  const sat = slot[1];
  const accentHue = (baseHue + 40 + (seed % 30)) % 360;
  const gradientStyle: CSSProperties = {
    background: `linear-gradient(135deg, hsl(${baseHue} ${sat}% 22%) 0%, hsl(${accentHue} ${sat}% 35%) 100%)`,
  };
  const initial = (album.title.trim().charAt(0) || "?").toUpperCase();

  return (
    <div
      className={
        "relative aspect-square w-full overflow-hidden rounded-md shadow-sm " +
        (className ?? "")
      }
      aria-label={`Cover art for ${album.title}`}
    >
      <div
        style={gradientStyle}
        className="absolute inset-0 flex items-center justify-center text-4xl font-bold text-white/90"
        aria-hidden
      >
        {initial}
      </div>
      {blobUrl && !failed && (
        <img
          src={blobUrl}
          alt=""
          className="absolute inset-0 h-full w-full object-cover"
          draggable={false}
          loading="lazy"
          onError={() => setFailed(true)}
        />
      )}
    </div>
  );
}
