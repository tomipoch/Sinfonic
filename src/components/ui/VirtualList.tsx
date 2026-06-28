// VirtualList — thin wrapper around `react-window` v2 for uniform-height
// rows.
//
// Why a wrapper instead of `useVirtualizer` directly:
//
// 1. Encapsulates the `react-window@2` API (which is component-style,
//    not hook-style) behind a single component that matches the
//    shape our existing `<ul>`-based lists already use. Consumers
//    don't need to learn the `rowComponent` / `rowProps` prop
//    pattern.
// 2. Centralises the scrollbar / overscan / container sizing
//    defaults so every virtualised list behaves the same way.
// 3. Provides a thin test seam — the wrapper itself can be unit
//    tested without rendering real rows.
//
// We assume uniform row height because every list that opts into
// this wrapper is a single-type list (all rows are TrackRow or all
// AlbumTile). Variable-height rows would require
// `useDynamicRowHeight` from react-window, which is a larger lift.

import { forwardRef, useImperativeHandle, useRef } from "react";
import { List, type ListImperativeAPI } from "react-window";

export interface VirtualListHandle {
  scrollToIndex: (index: number, align?: "auto" | "smart" | "center" | "end" | "start") => void;
}

interface VirtualListProps {
  rowCount: number;
  rowHeight: number;
  /**
   * Render a single row at the given index. The wrapper handles the
   * outer `<div style={...}>` so `rowRenderer` returns the *content*
   * of a row, not the row container.
   */
  rowRenderer: (index: number) => React.ReactNode;
  /** Outer container height in px or "100%". Defaults to "100%". */
  height?: number | string;
  className?: string;
  /** Number of rows to render outside the viewport. Default: 5. */
  overscanCount?: number;
}

// react-window infers the `RowProps` from `rowComponent`'s props. We
// declare the shape explicitly so the generic on `<List>` can be
// inferred.
type RowReserved = { index: number; style: React.CSSProperties };

export const VirtualList = forwardRef<VirtualListHandle, VirtualListProps>(function VirtualList(
  { rowCount, rowHeight, rowRenderer, height = "100%", className, overscanCount = 5 },
  ref,
) {
  const listRef = useRef<ListImperativeAPI>(null);

  useImperativeHandle(
    ref,
    () => ({
      scrollToIndex: (index, align) => {
        listRef.current?.scrollToRow({ index, align: align ?? "auto" });
      },
    }),
    [],
  );

  const Row = (props: RowReserved) => <div style={props.style}>{rowRenderer(props.index)}</div>;

  return (
    <List<RowReserved>
      listRef={listRef}
      rowCount={rowCount}
      rowHeight={rowHeight}
      rowComponent={Row}
      // `rowProps` must be the *additional* props beyond the
      // reserved `index`/`style`/`aria`. We pass none — react-
      // window's type narrows the rowProps shape from the row
      // component's prop signature, so we cast to satisfy it.
      rowProps={{} as unknown as never}
      overscanCount={overscanCount}
      className={className}
      style={{ height }}
    />
  );
});
