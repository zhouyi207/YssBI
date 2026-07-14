import { forwardRef, useEffect, useRef } from "react";
import { executeEditorDragEnd } from "@/features/application/editor/editorDragDropActions";
import { EditorDragPreviewMonitorHost } from "@/features/application/editor/useEditorDragPreviewMonitor";
import { LayoutNodeRenderer } from "../Renderer/LayoutNodeRenderer";
import { DndContext, useSensor, useSensors, PointerSensor, DragEndEvent, DragStartEvent, DragOverlay } from '@dnd-kit/core';
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useSidebarDragStore } from "@/features/core/sidebarDrag";
import { useModifierKeyStore } from "@/features/core/keyboard";
import {
  isSidebarSpawnDrag,
  buildSidebarDragState,
  parseCanvasDragPayload,
  snapTopLeftToCursor,
} from "@/features/core/dnd";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { EditorDropPreviewOverlay } from "./EditorDropPreviewOverlay";
import { WorkspaceDragOverlay } from "./WorkspaceDragOverlay";
import "../Renderer/viewRegistry"; // 确保业务组件已注册

export const Workspace = forwardRef<HTMLDivElement, { nodeId: string }>(({ nodeId }, ref) => {
  const setDragging = useLayoutStore(s => s.setDragging);
  const setActiveDrag = useSidebarDragStore(s => s.setActiveDrag);
  const updatePosition = useSidebarDragStore(s => s.updatePosition);

  const pointerMoveCleanupRef = useRef<(() => void) | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    })
  );

  useEffect(() => {
    return () => {
      pointerMoveCleanupRef.current?.();
      pointerMoveCleanupRef.current = null;
    };
  }, []);

  const finishSidebarDrag = () => {
    pointerMoveCleanupRef.current?.();
    pointerMoveCleanupRef.current = null;
    setActiveDrag(null);
  };

  const handleDragStart = (event: DragStartEvent) => {
    setDragging(true);
    const activeData = parseCanvasDragPayload(event.active.data.current);
    if (isSidebarSpawnDrag(activeData)) {
      const activatorEvent = event.activatorEvent as PointerEvent;
      const x = activatorEvent?.clientX ?? 0;
      const y = activatorEvent?.clientY ?? 0;
      setActiveDrag(buildSidebarDragState(activeData, x, y));
      const onMove = (e: PointerEvent) => {
        updatePosition(e.clientX, e.clientY);
        useModifierKeyStore.getState().setModifierKeys({
          altKey: e.altKey,
          ctrlKey: e.ctrlKey,
          shiftKey: e.shiftKey,
        });
      };
      pointerMoveCleanupRef.current?.();
      pointerMoveCleanupRef.current = addGlobalEventListener(document, "pointermove", onMove);
    }
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setDragging(false);
    void executeEditorDragEnd(event, { finishSidebarDrag });
  };

  return (
    <DndContext
      sensors={sensors}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    >
      <EditorDragPreviewMonitorHost />
      <EditorDropPreviewOverlay />
      <div ref={ref} className="flex-1 min-w-0 flex overflow-hidden relative">
        <div className="flex-1 min-w-0">
          <LayoutNodeRenderer nodeId={nodeId} />
        </div>

        <DragOverlay dropAnimation={null} modifiers={[snapTopLeftToCursor]}>
          <WorkspaceDragOverlay />
        </DragOverlay>
      </div>
    </DndContext>
  );
});

Workspace.displayName = 'Workspace';
