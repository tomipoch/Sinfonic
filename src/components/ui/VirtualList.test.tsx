// VirtualList — basic smoke tests. The wrapper exists to host
// future virtualised list migrations (P3). For now we just verify
// the props contract and the imperative `scrollToIndex` handle
// forwarding is correctly wired.

import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";

import { VirtualList, type VirtualListHandle } from "./VirtualList";

describe("VirtualList", () => {
  it("renders without crashing and exposes a scrollToIndex handle", () => {
    const ref = createRef<VirtualListHandle>();
    const { container } = render(
      <div style={{ height: 400 }}>
        <VirtualList
          ref={ref}
          rowCount={100}
          rowHeight={40}
          rowRenderer={(i) => <div data-testid={`row-${i}`}>row {i}</div>}
          height={400}
        />
      </div>,
    );

    // react-window mounts the outer scroll container.
    expect(container.firstChild).not.toBeNull();

    // The handle is wired. We can't easily verify scroll behaviour
    // in happy-dom (no layout), but we can confirm the function
    // exists and doesn't throw on call.
    expect(ref.current).not.toBeNull();
    expect(() => ref.current?.scrollToIndex(50, "start")).not.toThrow();
  });

  it("renders only the visible window plus overscan", () => {
    const renderSpy = vi.fn((index: number) => (
      <div data-testid={`row-${index}`}>row {index}</div>
    ));
    const { container } = render(
      <div style={{ height: 400 }}>
        <VirtualList
          rowCount={1000}
          rowHeight={40}
          rowRenderer={renderSpy}
          height={400}
          overscanCount={2}
        />
      </div>,
    );

    // 400px / 40px = 10 visible rows + 2 overscan top + 2 overscan
    // bottom = 14 total.
    expect(container.querySelectorAll("[data-testid^='row-']").length).toBeLessThanOrEqual(20);
    expect(renderSpy.mock.calls.length).toBeLessThanOrEqual(20);
  });
});