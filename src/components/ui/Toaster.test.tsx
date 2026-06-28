// Toaster — single Sonner instance mounted at app root.

import { render } from "@testing-library/react";

import { Toaster } from "./Toaster";

describe("Toaster", () => {
  it("renders without crashing", () => {
    const { container } = render(<Toaster />);
    expect(container.firstChild).not.toBeNull();
  });
});