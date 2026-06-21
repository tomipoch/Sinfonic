// Album cover placeholder.
//
// Real images land in Phase 7 (filesystem cache +
// `provider_image_bytes`). Until then we render a deterministic
// gradient seeded by the album id so every album looks distinct
// without external assets. The first letter of the title sits on
// top in white.

import type { CSSProperties } from "react";

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
  album: Pick<Album, "id" | "title">;
  className?: string;
}

export function AlbumCover({ album, className }: AlbumCoverProps) {
  const seed = hash(album.id);
  const slot = PALETTE[seed % PALETTE.length] ?? PALETTE[0]!;
  const baseHue = slot[0];
  const sat = slot[1];
  const accentHue = (baseHue + 40 + (seed % 30)) % 360;
  const style: CSSProperties = {
    background: `linear-gradient(135deg, hsl(${baseHue} ${sat}% 22%) 0%, hsl(${accentHue} ${sat}% 35%) 100%)`,
  };
  const initial = (album.title.trim().charAt(0) || "?").toUpperCase();
  return (
    <div
      style={style}
      className={
        "flex aspect-square w-full items-center justify-center rounded-md text-4xl font-bold text-white/90 shadow-sm " +
        (className ?? "")
      }
      aria-hidden
    >
      {initial}
    </div>
  );
}
