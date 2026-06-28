// Sort helpers — thin wrappers around `Array.prototype.sort` that
// match the domain-specific quirks:
//
//   - String fields use `localeCompare` with case-insensitive
//     sensitivity so "alice" and "ALICE" sort together.
//   - Numeric fields subtract (ascending).
//   - `null`/`undefined` numeric values (`year`) sort as 0 so the
//     row doesn't disappear from the top of a descending list.
//
// Centralised so `AlbumsView`, `ArtistsView`, and `TrackTable` use
// the exact same comparator semantics.

/**
 * Ascending string comparator (case-insensitive). Returns a negative
 * number if `a` should come first, positive if `b` should.
 */
export function compareString(a: string, b: string): number {
  return a.localeCompare(b, undefined, { sensitivity: "base" });
}

/**
 * Ascending numeric comparator. `null`/`undefined` values sort as
 * `fallback` (default `0`) so albums without a year still appear.
 */
export function compareNumber(
  a: number | null | undefined,
  b: number | null | undefined,
  fallback = 0,
): number {
  const av = a ?? fallback;
  const bv = b ?? fallback;
  return av - bv;
}

/**
 * Descending numeric comparator. Same null-handling as
 * `compareNumber` but reversed.
 */
export function compareNumberDesc(
  a: number | null | undefined,
  b: number | null | undefined,
  fallback = 0,
): number {
  return compareNumber(b, a, fallback);
}
