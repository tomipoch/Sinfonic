// Drag-then-commit slider hook shared between seek and volume.
//
// The native range input reports every pixel of movement via onChange,
// which would otherwise spam the IPC. This hook shadows the latest
// drag position locally; the consumer commits the value once on
// pointerup / keyup / blur.

import { useCallback, useState } from "react";

export interface DragCommit {
  /** The current display value (drag shadow if dragging, else upstream value). */
  value: number;
  /** Drag handler — pass to the input's onChange. */
  onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  /** Commit handler — call once when the user releases the control. */
  finish: () => void;
}

export function useDragCommit({ value }: { value: number }): DragCommit {
  const [drag, setDrag] = useState<number | null>(null);

  const onChange = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    setDrag(Number(event.currentTarget.value));
  }, []);

  return {
    value: drag ?? value,
    onChange,
    finish: () => setDrag(null),
  };
}
