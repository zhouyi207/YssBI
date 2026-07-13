import { useEffect } from 'react';
import { useDndMonitor, type DragMoveEvent, type DragStartEvent } from '@dnd-kit/core';
import {
  CANVAS_DROP_ZONE_ID_PREFIX,
  isCanvasDrop,
  isGraphResourceDragPayload,
  isLayoutRegionDrop,
  isTabbarDrop,
  isTabDragData,
  parseCanvasDragPayload,
  type GraphResourceDragPayload,
} from '@/features/core/dnd';
import {
  readEditorCanvasDropRect,
  readEditorSplitPreviewRect,
} from '@/features/core/layout/editorDropPreview';
import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { clearTabBarDragSession, useTabBarReorderStore } from './tabBarReorderStore';
import { useEditorDropPreviewStore } from './editorDropPreviewStore';
import {
  buildTabBarInsertPreview,
  findTabBarTargetFromPointer,
} from './tabBarInsertPreview';

function pointerFromDragEvent(event: DragMoveEvent | DragStartEvent): { x: number; y: number } | null {
  const activator = event.activatorEvent;
  if (!(activator instanceof MouseEvent) && !(activator instanceof PointerEvent)) {
    return null;
  }
  const delta = 'delta' in event ? event.delta : { x: 0, y: 0 };
  return {
    x: activator.clientX + delta.x,
    y: activator.clientY + delta.y,
  };
}

function findCanvasDropGroupId(
  pointerX: number,
  pointerY: number,
  over: DragMoveEvent['over'] | null,
): string | null {
  const overData = over?.data.current;
  if (overData && isCanvasDrop(overData)) {
    return overData.groupId;
  }

  const elements = document.elementsFromPoint(pointerX, pointerY);
  for (const element of elements) {
    if (!(element instanceof HTMLElement)) continue;
    if (!element.id.startsWith(CANVAS_DROP_ZONE_ID_PREFIX)) continue;
    return element.id.slice(CANVAS_DROP_ZONE_ID_PREFIX.length);
  }
  return null;
}

function findLayoutRegionFromPointer(
  pointerX: number,
  pointerY: number,
): { targetGroupId: string; edge: EditorSplitEdge } | null {
  const elements = document.elementsFromPoint(pointerX, pointerY);
  for (const element of elements) {
    if (!(element instanceof HTMLElement)) continue;
    const match = element.id.match(/^(.+)-(top|bottom|left|right|center)$/);
    if (!match) continue;
    return {
      targetGroupId: match[1],
      edge: match[2] as EditorSplitEdge,
    };
  }
  return null;
}

function resolveTabDragTitle(tabId: string, sourceGroupId: string): string {
  const element = document.querySelector(
    `[data-tab-id="${tabId}"][data-tab-group="${sourceGroupId}"]`,
  ) as HTMLElement | null;
  const label = element?.dataset.tabTitle;
  return label?.trim() || tabId;
}

function beginTabDrag(event: DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData)) return;

  useTabBarReorderStore.getState().setActiveTabDrag({
    tabId: activeData.tabId,
    sourceGroupId: activeData.sourceNodeId,
    title: resolveTabDragTitle(activeData.tabId, activeData.sourceNodeId),
  });
}

export function clearEditorDragSession(): void {
  clearTabBarDragSession();
  useEditorDropPreviewStore.getState().clearPreview();
}

function updateTabBarPreviewFromDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData)) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  const pointer = pointerFromDragEvent(event);
  if (!pointer) return;

  const strip = findTabBarTargetFromPointer(pointer.x, pointer.y);
  if (!strip) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  useTabBarReorderStore.getState().setPreview(
    buildTabBarInsertPreview(strip.groupId, strip.stripElement, pointer.x, {
      draggedTabId: activeData.tabId,
      sourceGroupId: activeData.sourceNodeId,
    }),
  );
}

function updateGraphResourceTabBarPreview(event: DragMoveEvent | DragStartEvent): void {
  const pointer = pointerFromDragEvent(event);
  if (!pointer) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  const strip = findTabBarTargetFromPointer(pointer.x, pointer.y);
  if (!strip) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  useTabBarReorderStore.getState().setPreview(
    buildTabBarInsertPreview(strip.groupId, strip.stripElement, pointer.x, {
      draggedTabId: null,
      sourceGroupId: null,
    }),
  );
}

function updateSplitDropPreviewFromDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData)) return;

  const pointer = pointerFromDragEvent(event);
  if (pointer && findTabBarTargetFromPointer(pointer.x, pointer.y)) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  let targetGroupId: string | null = null;
  let edge: EditorSplitEdge | null = null;

  const over = 'over' in event ? event.over : null;
  const overData = over?.data.current;
  if (overData && isLayoutRegionDrop(overData) && !isTabbarDrop(overData)) {
    targetGroupId = overData.targetNodeId;
    edge = overData.dropPosition as EditorSplitEdge;
  } else if (pointer) {
    const hit = findLayoutRegionFromPointer(pointer.x, pointer.y);
    if (hit) {
      targetGroupId = hit.targetGroupId;
      edge = hit.edge;
    }
  }

  if (!targetGroupId || !edge) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const rect = readEditorSplitPreviewRect(targetGroupId, edge);
  if (!rect) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  useEditorDropPreviewStore.getState().setPreview({
    kind: 'split',
    targetGroupId,
    edge,
    rect,
  });
}

function updateCanvasOpenPreviewFromDragMove(
  event: DragMoveEvent | DragStartEvent,
  activeData: GraphResourceDragPayload,
): void {
  const pointer = pointerFromDragEvent(event);
  if (!pointer) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const over = 'over' in event ? event.over : null;
  const groupId = findCanvasDropGroupId(pointer.x, pointer.y, over);
  if (!groupId) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const rect = readEditorCanvasDropRect(groupId);
  if (!rect) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  useEditorDropPreviewStore.getState().setPreview({
    kind: 'canvas-open',
    targetGroupId: groupId,
    rect,
    resourceName: activeData.sidebarResource.name,
  });
}

function handleDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);

  if (isTabDragData(activeData)) {
    updateTabBarPreviewFromDragMove(event);
    updateSplitDropPreviewFromDragMove(event);
    return;
  }

  if (isGraphResourceDragPayload(activeData)) {
    const pointer = pointerFromDragEvent(event);
    if (pointer && findTabBarTargetFromPointer(pointer.x, pointer.y)) {
      useEditorDropPreviewStore.getState().clearPreview();
      updateGraphResourceTabBarPreview(event);
      return;
    }

    useTabBarReorderStore.getState().clearPreview();
    updateCanvasOpenPreviewFromDragMove(event, activeData);
    return;
  }

  useTabBarReorderStore.getState().clearPreview();
  useEditorDropPreviewStore.getState().clearPreview();
}

/**
 * Mount inside `<DndContext>` — `useDndMonitor` cannot run on the context provider itself.
 */
export function EditorDragPreviewMonitorHost(): null {
  useEditorDragPreviewMonitor();
  return null;
}

/**
 * Unified drag preview monitor: tab reorder, tab split, sidebar graph open on canvas / TabBar.
 */
export function useEditorDragPreviewMonitor(): void {
  useDndMonitor({
    onDragStart: (event) => {
      const activeData = parseCanvasDragPayload(event.active.data.current);
      if (isTabDragData(activeData)) {
        beginTabDrag(event);
      } else {
        useTabBarReorderStore.getState().setActiveTabDrag(null);
        useTabBarReorderStore.getState().clearPreview();
      }
      handleDragMove(event);
    },
    onDragMove: handleDragMove,
    onDragEnd: clearEditorDragSession,
    onDragCancel: clearEditorDragSession,
  });

  useEffect(() => () => clearEditorDragSession(), []);
}
