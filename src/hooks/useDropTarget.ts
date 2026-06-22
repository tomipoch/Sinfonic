import { useCallback, useRef, useState } from "react";
import { decodeDragData } from "../lib/queueDnD";
import type { Track } from "../types/domain";

interface UseDropTargetOptions {
  onDrop: (tracks: Track[], insertIndex: number) => void | Promise<void>;
}

export function useDropTarget({ onDrop }: UseDropTargetOptions) {
  const [dragOver, setDragOver] = useState(false);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const droppableRef = useRef<HTMLElement | null>(null);

  const handleDragOver = useCallback(
    (e: React.DragEvent<HTMLElement>) => {
      const raw = e.dataTransfer.types.includes("application/json")
        ? e.dataTransfer.getData("application/json")
        : null;
      if (!raw) return;

      const data = decodeDragData(raw);
      if (!data) return;

      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";

      setDragOver(true);

      if (droppableRef.current) {
        const rect = droppableRef.current.getBoundingClientRect();
        const y = e.clientY - rect.top;
        const height = rect.height;
        const itemHeight = height / Math.max(1, data.tracks.length);
        const hoveredIndex = Math.floor(y / itemHeight);
        setDropIndex(hoveredIndex);
      }
    },
    [],
  );

  const handleDragLeave = useCallback(() => {
    setDragOver(false);
    setDropIndex(null);
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent<HTMLElement>) => {
      e.preventDefault();
      const raw = e.dataTransfer.getData("application/json");
      if (!raw) return;

      const data = decodeDragData(raw);
      if (!data) return;

      setDragOver(false);
      setDropIndex(null);

      const insertIndex = droppableRef.current
        ? (() => {
            const rect = droppableRef.current!.getBoundingClientRect();
            const y = e.clientY - rect.top;
            const height = rect.height;
            const itemHeight = height / Math.max(1, data.tracks.length);
            return Math.floor(y / itemHeight);
          })()
        : undefined;

      await onDrop(data.tracks, insertIndex ?? data.tracks.length);
    },
    [onDrop],
  );

  const setRef = useCallback((el: HTMLElement | null) => {
    droppableRef.current = el;
  }, []);

  return {
    dragOver,
    dropIndex,
    droppableProps: {
      ref: setRef,
      onDragOver: handleDragOver,
      onDragLeave: handleDragLeave,
      onDrop: handleDrop,
    },
  };
}
