import { forwardRef, useEffect, useRef } from "react";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { openGraphInEditor } from "@/features/application/editor/openGraphInEditor";
import {
  moveTabBetweenGroups,
  splitEditorWithTab,
} from "@/features/application/editor/editorGroupCommands";
import { resolveTabBarDropIndex } from "@/features/application/editor/tabBarReorderStore";
import { EditorDragPreviewMonitorHost } from "@/features/application/editor/useEditorDragPreviewMonitor";
import { uiStore } from "@/features/core/ui/UIStore";
import { LayoutNodeRenderer } from "../Renderer/LayoutNodeRenderer";
import { DndContext, useSensor, useSensors, PointerSensor, DragEndEvent, DragStartEvent, DragOverlay } from '@dnd-kit/core';
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useSidebarDragStore, canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { useModifierKeyStore } from "@/features/core/keyboard";
import {
  isCanvasDrop,
  isLayoutRegionDrop,
  isTabbarDrop,
  isGraphResourceDragPayload,
  isNodeTemplateDragData,
  isNodeTemplateDragState,
  isSidebarSpawnDrag,
  isTabDragData,
  getSidebarResourceFromDrag,
  buildSidebarDragState,
  parseCanvasDragPayload,
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
      const onMove = (e: PointerEvent) => updatePosition(e.clientX, e.clientY);
      pointerMoveCleanupRef.current?.();
      pointerMoveCleanupRef.current = addGlobalEventListener(document, "pointermove", onMove);
    }
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setDragging(false);
    const { active, over } = event;
    const activeData = parseCanvasDragPayload(active.data.current);
    const overData = over?.data.current;
    const sidebarResource = getSidebarResourceFromDrag(activeData);

    if (sidebarResource && isCanvasDrop(overData)) {
      finishSidebarDrag();

      const targetGroupId = overData.groupId || useLayoutStore.getState().activeEditorGroupId || "default_editor";
      void openGraphInEditor(sidebarResource.id, sidebarResource.name, sidebarResource.type, targetGroupId)
        .catch((error) => uiStore.showToast(formatErrorMessage(error), "error"));
      return;
    }

    if (isGraphResourceDragPayload(activeData)) {
      finishSidebarDrag();
      return;
    }

    if (isNodeTemplateDragData(activeData)) {
      const dragState = useSidebarDragStore.getState().activeDrag;
      finishSidebarDrag();
      const groupId = isCanvasDrop(overData) ? overData.groupId : null;
      if (groupId && dragState && isNodeTemplateDragState(dragState)) {
        // 将目标 canvas 设为 active group（确保 variable drop menu 等 UI 正确显示）
        useLayoutStore.getState().setActiveGroup(groupId);

        const handler = canvasDropHandlerStore.getHandler(groupId);
        if (handler) {
          const modifierKeys = useModifierKeyStore.getState();
          handler(dragState, {
            altKey: modifierKeys.altKey || (event.activatorEvent as PointerEvent)?.altKey || false,
            ctrlKey: modifierKeys.ctrlKey || (event.activatorEvent as PointerEvent)?.ctrlKey || false,
          });
        }
      }
      return;
    }

    if (over && active.id !== over.id) {
      const dropData = over.data.current;
      const dropPosition = isLayoutRegionDrop(dropData) ? dropData.dropPosition : 'center';
      const targetNodeId = isLayoutRegionDrop(dropData) || isTabbarDrop(dropData)
        ? dropData.targetNodeId
        : over.id;

      if (isTabDragData(activeData)) {
        const { sourceNodeId, tabId } = activeData;
        const isTabbarTarget = isTabbarDrop(dropData);

        if (isTabbarTarget) {
          moveTabBetweenGroups(
            sourceNodeId,
            tabId,
            targetNodeId as string,
            resolveTabBarDropIndex(targetNodeId as string, dropData.targetTabIndex),
          );
        } else if (isLayoutRegionDrop(dropData)) {
          void splitEditorWithTab(
            sourceNodeId,
            tabId,
            targetNodeId as string,
            dropPosition,
          );
        }
      }
    }
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

        {/* Drag overlay for sidebar items */}
        <DragOverlay dropAnimation={null}>
          <WorkspaceDragOverlay />
        </DragOverlay>
      </div>
    </DndContext>
  );
});

Workspace.displayName = 'Workspace';
