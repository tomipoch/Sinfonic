// MaterialSymbol — Google Material Symbols (rounded) glyph via the
// ligature-based CSS font shipped by the `material-symbols` package.
//
// The package only exposes a CSS file (no per-icon React component),
// so this thin wrapper applies the `material-symbols-rounded` class
// to a span and sets `font-variation-settings` for fill / weight /
// grade / opsz. Icon names are camelCase-free ligatures — see
// https://fonts.google.com/icons for the full list.
//
// The font-size can be overridden via the `--mat-symbol-size` CSS
// variable so callers can drive a responsive size from CSS without
// having to ref the React tree. `size` is the fallback used when
// no CSS var is set.

import { type CSSProperties, memo } from "react";
import { cn } from "@/lib/cn";

interface MaterialSymbolProps {
  name: string;
  size?: number;
  fill?: boolean;
  weight?: number;
  className?: string;
  style?: CSSProperties;
}

function MaterialSymbolImpl({
  name,
  size = 20,
  fill = false,
  weight = 500,
  className,
  style,
}: MaterialSymbolProps) {
  return (
    <span
      role="img"
      aria-hidden="true"
      className={cn("material-symbols-rounded", className)}
      style={{
        fontSize: "var(--mat-symbol-size, ".concat(String(size), "px)"),
        fontVariationSettings: `'FILL' ${fill ? 1 : 0}, 'wght' ${weight}, 'GRAD' 0, 'opsz' ${size}`,
        lineHeight: 1,
        userSelect: "none",
        ...style,
      }}
    >
      {name}
    </span>
  );
}

export const MaterialSymbol = memo(MaterialSymbolImpl);
