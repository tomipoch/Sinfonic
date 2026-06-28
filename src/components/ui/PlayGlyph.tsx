// PlayGlyph — small play-triangle SVG used in the "Play all" buttons
// across `AlbumsView`, `SongsView`, and the per-row overlay in
// `TrackTable`. Centralised so the icon's path and accessibility
// attributes stay in sync.
//
// The component takes an optional `size` (in px) and applies it as
// the SVG's `width` and `height`. Callers that previously used
// `className="h-3.5 w-3.5"` should pass `size={14}` instead (the
// default), which renders to the same pixel size.

interface PlayGlyphProps {
  /** SVG width / height in px. Defaults to 14 (= `h-3.5 w-3.5`). */
  size?: number;
  /** Extra className on the inner `<svg>`. */
  className?: string;
}

export function PlayGlyph({ size = 14, className }: PlayGlyphProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="currentColor"
      aria-hidden
      className={className}
    >
      <path d="M8 5v14l11-7z" />
    </svg>
  );
}
