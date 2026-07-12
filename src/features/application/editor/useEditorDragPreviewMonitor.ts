import { useEffect } from 'react';
import { useDndMonitor, type DragMoveEvent, type DragStartEvent } from '@dnd-kit/core';
import {
  computeTabGapLeft,
  computeTabInsertIndex,
  measureTabBarMetrics,
  resolveTabGapWidth,
} from '@/features/core/layout/tabBarInsertIndex';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
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

const TAB_BAR_DROP_SELECTOR = '[data-tabbar-drop]';
const TAB_STRIP_SELECTOR = '[data-tab-strip]';

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

function findTabBarTarget(
  pointerX: number,
  pointerY: number,
): { groupId: string; stripElement: HTMLElement } | null {
  const dropTargets = document.querySelectorAll<HTMLElement>(TAB_BAR_DROP_SELECTOR);
  for (const dropElement of dropTargets) {
    const rect = dropElement.getBoundingClientRect();
    if (
      pointerX >= rect.left
      && pointerX <= rect.right
      && pointerY >= rect.top
      && pointerY <= rect.bottom
    ) {
      const groupId = dropElement.dataset.tabbarDrop;
      if (!groupId) continue;
      const stripElement = dropElement.querySelector<HTMLElement>(TAB_STRIP_SELECTOR) ?? dropElement;
      return { groupId, stripElement };
    }
  }
  return null;
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

function tabIdsForGroup(groupId: string): string[] {
  return useLayoutStore.getState().nodes[groupId]?.data?.tabs?.map((tab) => tab.id) ?? [];
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

  const strip = findTabBarTarget(pointer.x, pointer.y);
  if (!strip) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  const tabIds = tabIdsForGroup(strip.groupId);
  const metrics = measureTabBarMetrics(strip.stripElement, tabIds);
  if (metrics.length === 0) {
    useTabBarReorderStore.getState().setPreview({
      targetGroupId: strip.groupId,
      sourceGroupId: activeData.sourceNodeId,
      draggedTabId: activeData.tabId,
      insertIndex: 0,
      draggedIndex: -1,
      gapWidth: resolveTabGapWidth(metrics, activeData.tabId),
      gapLeft: 0,
    });
    return;
  }

  const sameGroup = activeData.sourceNodeId === strip.groupId;
  const draggedIndex = sameGroup
    ? metrics.findIndex((metric) => metric.tabId === activeData.tabId)
    : -1;
  const scrollLeft = (
    strip.stripElement.closest('.overlay-scrollbar-viewport') as HTMLElement | null
  )?.scrollLeft ?? 0;
  const insertIndex = computeTabInsertIndex(
    pointer.x - strip.stripElement.getBoundingClientRect().left + scrollLeft,
    metrics,
  );
  const gapWidth = resolveTabGapWidth(metrics, sameGroup ? activeData.tabId : null);

  useTabBarReorderStore.getState().setPreview({
    targetGroupId: strip.groupId,
    sourceGroupId: activeData.sourceNodeId,
    draggedTabId: activeData.tabId,
    insertIndex,
    draggedIndex,
    gapWidth,
    gapLeft: computeTabGapLeft(
      metrics,
      insertIndex,
      sameGroup ? activeData.tabId : null,
    ),
  });
}

function updateSplitDropPreviewFromDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData)) return;

  const pointer = pointerFromDragEvent(event);
  if (pointer && findTabBarTarget(pointer.x, pointer.y)) {
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

  useTabBarReorderStore.getState().clearPreview();

  if (isGraphResourceDragPayload(activeData)) {
    updateCanvasOpenPreviewFromDragMove(event, activeData);
    return;
  }

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
 * Unified drag preview monitor: tab reorder, tab split, sidebar graph open on canvas.
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
