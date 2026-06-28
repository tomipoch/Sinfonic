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

// cleanError — strip the redundant `local scan:`, `login failed:`,
// `save token:` prefixes the Rust commands wrap their inner error
// with. Most are dropped; the credential-storage ones are rewritten
// to a friendlier "Token storage: ..." form so the user understands
// it's a keychain / keyring issue rather than a server problem.
//
// Centralised so `ServerConnectionForm` (which renders errors inline)
// and `ServerManager` (toasts them) share the exact same rewrites.

const CLEAN_RULES: ReadonlyArray<readonly [RegExp, string]> = [
  [/^local scan:\s*/i, ""],
  [/^login failed:\s*/i, ""],
  [/^save token:\s*/i, "Token storage: "],
  [/^build provider:\s*/i, "Provider: "],
  [/^upsert server:\s*/i, "Server record: "],
  [/^provider_set_active:\s*/i, ""],
];

export function cleanError(message: string | null | undefined): string | null {
  if (!message) return null;
  let cleaned = message;
  for (const [pattern, replacement] of CLEAN_RULES) {
    cleaned = cleaned.replace(pattern, replacement);
  }
  return cleaned;
}
