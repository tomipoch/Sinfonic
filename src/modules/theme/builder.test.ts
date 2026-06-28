// `defineTheme()` builder — round-trip test that the flat-key shape
// produces the same `Theme` the consumers see.

import { describe, expect, it } from "vitest";

import { defineTheme } from "./builder";

describe("defineTheme", () => {
  it("promotes flat dark/light keys to the nested variants shape", () => {
    const theme = defineTheme({
      id: "test",
      name: "Test",
      dark: { colors: { background: "#000" } },
      light: { colors: { background: "#fff" } },
    });
    expect(theme.id).toBe("test");
    expect(theme.name).toBe("Test");
    expect(theme.variants.dark?.colors?.background).toBe("#000");
    expect(theme.variants.light?.colors?.background).toBe("#fff");
  });

  it("passes through editorTheme, description, author", () => {
    const theme = defineTheme({
      id: "t",
      name: "T",
      description: "desc",
      author: "me",
      editorTheme: { dark: "a", light: "b" },
    });
    expect(theme.description).toBe("desc");
    expect(theme.author).toBe("me");
    expect(theme.editorTheme).toEqual({ dark: "a", light: "b" });
  });

  it("allows dark/light to be undefined (sinfonic-default pattern)", () => {
    const theme = defineTheme({ id: "t", name: "T" });
    expect(theme.variants.dark).toBeUndefined();
    expect(theme.variants.light).toBeUndefined();
  });
});