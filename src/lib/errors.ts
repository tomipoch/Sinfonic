// extractError — defensive error-message extraction for Tauri IPC.
//
// In Tauri v2, `invoke()` rejects with the **string** returned from the
// Rust command's `Result::Err`, not an `Error` instance. Code that
// naively does `(e as Error).message` therefore gets `undefined` and
// silently degrades to a generic fallback message — the user never
// sees what actually went wrong. This helper handles both shapes:
//
//   - Tauri string rejection          → "save token: ..." returned as-is
//   - Native Error (tests, manual)    → message field preserved
//   - Anything else (null, object)    → String() coercion
//
// Always returns a non-empty string. Empty results fall back to
// `fallback` so callers never have to handle the empty-string case.

export function extractError(e: unknown, fallback: string): string {
  if (typeof e === "string") return e.length > 0 ? e : fallback;
  if (e == null) return fallback;
  if (e instanceof Error) return e.message.length > 0 ? e.message : fallback;
  if (typeof e === "object" && "message" in e) {
    const m = (e as { message: unknown }).message;
    if (typeof m === "string" && m.length > 0) return m;
  }
  const s = String(e);
  return s === "[object Object]" || s.length === 0 ? fallback : s;
}
