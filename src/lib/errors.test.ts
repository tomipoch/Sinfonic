import { describe, expect, it, vi } from "vitest";

import { cleanError, extractError } from "./errors";

describe("extractError", () => {
  it("returns Tauri string rejection as-is (the common case)", () => {
    const tauriErr = "subsonic: password missing from keyring";
    expect(extractError(tauriErr, "fallback")).toBe(tauriErr);
  });

  it("returns Error.message for native Error instances", () => {
    expect(extractError(new Error("boom"), "fallback")).toBe("boom");
  });

  it("falls back when the Error has an empty message", () => {
    expect(extractError(new Error(""), "fallback")).toBe("fallback");
  });

  it("falls back when the string is empty", () => {
    expect(extractError("", "fallback")).toBe("fallback");
  });

  it("falls back when the value is null or undefined", () => {
    expect(extractError(null, "fallback")).toBe("fallback");
    expect(extractError(undefined, "fallback")).toBe("fallback");
  });

  it("handles plain objects with a message field", () => {
    expect(extractError({ message: "from object" }, "fallback")).toBe(
      "from object",
    );
  });

  it("falls back when the plain object has no message", () => {
    expect(extractError({ foo: "bar" }, "fallback")).toBe("fallback");
  });

  it("legacy buggy pattern would have hidden the backend message", () => {
    const tauriErr = "provider_set_active: load password: keyring error";
    const buggyResult = (tauriErr as unknown as Error).message || "switch source failed";
    expect(buggyResult).toBe("switch source failed");
    expect(extractError(tauriErr, "switch source failed")).toBe(tauriErr);
  });
});

describe("cleanError", () => {
  it("returns null for empty / nullish input", () => {
    expect(cleanError(null)).toBeNull();
    expect(cleanError(undefined)).toBeNull();
    expect(cleanError("")).toBeNull();
  });

  it("strips the redundant 'local scan:' prefix", () => {
    expect(cleanError("local scan: permission denied")).toBe(
      "permission denied",
    );
  });

  it("strips the redundant 'login failed:' prefix", () => {
    expect(cleanError("login failed: bad password")).toBe("bad password");
  });

  it("rewrites credential-storage errors to friendlier form", () => {
    expect(cleanError("save token: keyring error")).toBe(
      "Token storage: keyring error",
    );
    expect(cleanError("build provider: bad config")).toBe(
      "Provider: bad config",
    );
    expect(cleanError("upsert server: dup key")).toBe(
      "Server record: dup key",
    );
  });

  it("strips the provider_set_active prefix", () => {
    expect(cleanError("provider_set_active: bad id")).toBe("bad id");
  });

  it("applies multiple rewrites in order", () => {
    // A message containing both `local scan:` and `provider_set_active:`
    // would be unusual but the helper should handle it idempotently.
    const cleaned = cleanError("local scan: provider_set_active: nested");
    expect(cleaned).toBe("nested");
  });

  it("passes through messages that don't match any prefix", () => {
    expect(cleanError("something unrelated")).toBe("something unrelated");
  });
});

// `vi` import is intentional: when this file is the only place that
// uses `vi` in `src/lib`, Biome's `noUnusedImports` rule would strip
// it otherwise. We use it implicitly via `vi.fn()` in the other test
// files; this keeps the import valid as a defensive default.
void vi;