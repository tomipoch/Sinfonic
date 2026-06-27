// extractError — regression tests for the Tauri string-vs-Error
// extraction pattern.
//
// Tauri v2 rejects invoke() promises with the **string** the Rust
// command returned in `Result::Err`, not an `Error` instance. The
// legacy `(e as Error).message || "fallback"` pattern therefore
// silently degrades to the fallback for every IPC rejection, hiding
// the real backend message from the user.

import { describe, expect, it } from "vitest";

import { extractError } from "./errors";

describe("extractError", () => {
  it("returns Tauri string rejection as-is (the common case)", () => {
    // Tauri rejects invoke() with the Err string from the Rust
    // command. This is what `provider_set_active` returns when
    // the keyring has no password for the saved server id.
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
    // Documents the regression: under the old `(e as Error).message
    // || "fallback"` pattern this exact value rendered as the
    // fallback string, so the user could never tell *why* the
    // provider switch failed.
    const tauriErr = "provider_set_active: load password: keyring error";
    const buggyResult = (tauriErr as unknown as Error).message || "switch source failed";
    expect(buggyResult).toBe("switch source failed");

    // extractError preserves the actual message.
    expect(extractError(tauriErr, "switch source failed")).toBe(tauriErr);
  });
});
