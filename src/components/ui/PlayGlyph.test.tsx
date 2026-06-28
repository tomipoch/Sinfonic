// PlayGlyph — regression test for the centralised play-triangle SVG.

import { render, screen } from "@testing-library/react";

import { PlayGlyph } from "./PlayGlyph";

describe("PlayGlyph", () => {
  it("renders an SVG with the canonical play path", () => {
    const { container } = render(<PlayGlyph />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg?.getAttribute("aria-hidden")).toBe("true");
    expect(svg?.getAttribute("fill")).toBe("currentColor");
    // The path carries the canonical triangle (M8 5v14l11-7z).
    const path = container.querySelector("path");
    expect(path?.getAttribute("d")).toBe("M8 5v14l11-7z");
  });

  it("defaults to 14px and accepts an explicit size", () => {
    const { container, rerender } = render(<PlayGlyph />);
    expect(container.querySelector("svg")?.getAttribute("width")).toBe("14");
    expect(container.querySelector("svg")?.getAttribute("height")).toBe("14");

    rerender(<PlayGlyph size={20} />);
    expect(container.querySelector("svg")?.getAttribute("width")).toBe("20");
    expect(container.querySelector("svg")?.getAttribute("height")).toBe("20");
  });

  it("forwards an extra className to the SVG", () => {
    const { container } = render(<PlayGlyph className="h-3.5 w-3.5" />);
    expect(container.querySelector("svg")?.getAttribute("class")).toBe("h-3.5 w-3.5");
  });

  it("has no role / label — the parent button provides the semantic", () => {
    render(<PlayGlyph aria-label="ignored" />);
    // Sonner / React Testing Library shouldn't expose the SVG to the
    // accessible name (aria-hidden supersedes the prop).
    expect(screen.queryByRole("img")).toBeNull();
  });
});