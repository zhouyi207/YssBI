import { useCallback, useEffect, useMemo, useRef } from "react";

import {
  beginActivityEditorDrag,
  executeEditorDragEnd,
  finishActivityEditorDrag,
  updateActivityEditorDragPointer,
} from "@/features/application/editor/editorDragDropActions";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import type { RootDockviewDndCoordinator } from "@/views/EditorView/Layout/RootDockviewHost";

export function useActivityEditorDndCoordinator(): RootDockviewDndCoordinator {
  const pointerMoveCleanupRef = useRef<(() => void) | null>(null);

  const finishDrag = useCallback(() => {
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = null;
    finishActivityEditorDrag();
  }, []);

  useEffect(() => finishDrag, [finishDrag]);

  const onDragStart = useCallback<RootDockviewDndCoordinator["onDragStart"]>((event) => {
    if (!beginActivityEditorDrag(event)) return;
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = addGlobalEventListener(
      document,
      "pointermove",
      updateActivityEditorDragPointer,
    );
  }, []);

  const onDragEnd = useCallback<RootDockviewDndCoordinator["onDragEnd"]>(
    (event) => {
      void executeEditorDragEnd(event, { finishSidebarDrag: finishDrag });
    },
    [finishDrag],
  );

  return useMemo(() => ({ onDragStart, onDragEnd }), [onDragEnd, onDragStart]);
}
