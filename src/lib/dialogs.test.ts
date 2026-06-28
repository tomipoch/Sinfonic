import { describe, expect, it, vi } from "vitest";

import { pickLocalFolder } from "./dialogs";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => "/music/library"),
}));

describe("pickLocalFolder", () => {
  it("returns the selected absolute path", async () => {
    const path = await pickLocalFolder();
    expect(path).toBe("/music/library");
  });

  it("returns undefined when the user cancels", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValueOnce(null);
    const path = await pickLocalFolder();
    expect(path).toBeUndefined();
  });
});