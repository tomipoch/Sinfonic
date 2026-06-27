// useDropTarget — regression tests for the HTML5 dragover contract.
//
// The original bug: handleDragOver called `dataTransfer.getData()`
// which returns "" during dragover per the HTML5 spec. The hook
// always bailed to the early return and `dragOver` was never set,
// so the PlayerBar's drop highlight never appeared.

import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useDropTarget } from "./useDropTarget";
import { encodeDragData } from "@/lib/queueDnD";
import type { Track } from "@/types/domain";

const track: Track = {
  id: "track-1",
  albumId: { as_str: () => "album-1" } as never,
  title: "T1",
  artist: "A",
  artistId: null,
  album: "Al",
  durationSeconds: 180,
  trackNumber: 1,
  discNumber: 1,
  favorite: false,
  imageRef: null,
};

function fakeRect() {
  return { top: 0, height: 200, bottom: 200, left: 0, right: 0, width: 0, x: 0, y: 0, toJSON: () => {} };
}

function makeDragEvent(overrides: Partial<{
  types: readonly string[];
  getData: (k: string) => string;
  setData: (k: string, v: string) => void;
  dropEffect: string;
  clientY: number;
}> = {}) {
  return {
    preventDefault: vi.fn(),
    clientY: 50,
    dataTransfer: {
      types: ["application/json"] as readonly string[],
      getData: vi.fn(() => ""),
      setData: vi.fn(),
      dropEffect: "",
      ...overrides,
    },
  };
}

describe("useDropTarget", () => {
  it("flips `dragOver` to true on dragover when types include application/json", () => {
    const onDrop = vi.fn();
    const { result } = renderHook(() => useDropTarget({ onDrop }));

    const el = document.createElement("footer");
    Object.defineProperty(el, "getBoundingClientRect", { value: fakeRect });
    result.current.droppableProps.ref(el);

    const event = makeDragEvent();
    act(() => {
      result.current.droppableProps.onDragOver(event as unknown as React.DragEvent<HTMLElement>);
    });

    expect(result.current.dragOver).toBe(true);
    // Critical regression: handleDragOver must NEVER call getData.
    // If getData was called here (the original bug), the spec
    // guarantee that getData returns "" during dragover means the
    // hook would always bail to the early return.
    expect(event.dataTransfer.getData).not.toHaveBeenCalled();
  });

  it("parses payload on drop and forwards tracks via onDrop", async () => {
    const onDrop = vi.fn();
    const { result } = renderHook(() => useDropTarget({ onDrop }));

    const el = document.createElement("footer");
    Object.defineProperty(el, "getBoundingClientRect", { value: fakeRect });
    result.current.droppableProps.ref(el);

    const payload = encodeDragData({ tracks: [track], source: "songs-view" });
    const event = makeDragEvent({ getData: vi.fn(() => payload) });

    await act(async () => {
      await result.current.droppableProps.onDrop(event as unknown as React.DragEvent<HTMLElement>);
    });

    expect(onDrop).toHaveBeenCalledTimes(1);
    const call = onDrop.mock.calls[0];
    expect(call).toBeDefined();
    const [tracks] = call!;
    expect(tracks).toHaveLength(1);
    expect(tracks[0].id).toBe("track-1");
    expect(result.current.dragOver).toBe(false);
  });

  it("clears `dragOver` on dragleave", () => {
    const onDrop = vi.fn();
    const { result } = renderHook(() => useDropTarget({ onDrop }));

    const el = document.createElement("footer");
    result.current.droppableProps.ref(el);

    const over = makeDragEvent();
    act(() => {
      result.current.droppableProps.onDragOver(over as unknown as React.DragEvent<HTMLElement>);
    });
    expect(result.current.dragOver).toBe(true);

    act(() => {
      result.current.droppableProps.onDragLeave();
    });
    expect(result.current.dragOver).toBe(false);
  });
});
