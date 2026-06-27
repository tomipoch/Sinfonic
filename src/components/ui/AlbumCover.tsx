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
//
// `source` is intentionally generic (`{ id, title, imageRef }`) so
// tracks (which carry their own `imageRef` in the cache) can be
// rendered the same way as albums.

import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { providerImageBytes } from "@/lib/tauri";
import { buildBlobUrl, getCached, setCached } from "@/lib/albumArtCache";
import type { ImageRef } from "@/types/domain";

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

export interface AlbumCoverSource {
  id: string;
  // Optional — callers that have a "title" (albums) pass it; entities
  // like artists without a title field still get an aria-label via
  // the `ariaLabel` prop and a fallback initial via the `initial` prop.
  title?: string;
  imageRef?: ImageRef | null;
}

interface AlbumCoverProps {
  source: AlbumCoverSource;
  className?: string;
  ariaLabel?: string;
  initial?: string;
}

export function AlbumCover({
  source,
  className,
  ariaLabel,
  initial,
}: AlbumCoverProps) {
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const itemId = source.imageRef?.itemId;
    if (!itemId) {
      setBlobUrl(null);
      setFailed(false);
      return;
    }
    const tag = source.imageRef?.tag ?? null;

    // Synchronous cache hit — render the blob URL immediately
    // without an IPC roundtrip or a re-render.
    const cached = getCached(itemId, tag);
    if (cached) {
      setBlobUrl(cached);
      setFailed(false);
      return;
    }

    setBlobUrl(null);
    setFailed(false);
    let cancelled = false;

    (async () => {
      try {
        const res = await providerImageBytes(itemId, tag);
        if (cancelled) return;
        const url = buildBlobUrl(res.bytes, res.contentType);
        setCached(itemId, tag, url);
        setBlobUrl(url);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [source.id, source.imageRef?.itemId, source.imageRef?.tag]);

  const seed = hash(source.id);
  const slot = PALETTE[seed % PALETTE.length] ?? PALETTE[0]!;
  const baseHue = slot[0];
  const sat = slot[1];
  const accentHue = (baseHue + 40 + (seed % 30)) % 360;
  const gradientStyle: CSSProperties = {
    background: `linear-gradient(135deg, hsl(${baseHue} ${sat}% 22%) 0%, hsl(${accentHue} ${sat}% 35%) 100%)`,
  };
  const fallbackInitial =
    initial ?? (source.title?.trim().charAt(0) || "?").toUpperCase();

  return (
    <div
      className={
        "relative aspect-square w-full overflow-hidden rounded-md shadow-sm " +
        (className ?? "")
      }
      aria-label={ariaLabel ?? `Cover art for ${source.title}`}
    >
      <div
        style={gradientStyle}
        className="absolute inset-0 flex items-center justify-center text-4xl font-bold text-white/90"
        aria-hidden
      >
        {fallbackInitial}
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
