import { useEffect } from 'react';
import { useDndMonitor, type DragMoveEvent, type DragStartEvent } from '@dnd-kit/core';
import {
  isEditorGroupDragData,
  isSidebarSpawnDrag,
  isTabDragData,
  parseCanvasDragPayload,
  readDragModifiers,
  resolveDragClientPoint,
} from '@/features/core/dnd';
import { resolveEditorDropHitAtClientPoint } from '@/features/core/layout/editorDropPreview';
import {
  findEditorGroupAtPointer,
  findTabBarTargetFromPointer,
  type TabBarInsertPreviewContext,
} from '@/features/core/layout/editorDropTarget';
import {
  preferSplitVerticallyFromDirection,
  readEditorPartOptions,
} from '@/features/core/layout/editorPartOptions';
import { resolveEnableSplittingOnDrag } from '@/features/core/layout/editorDragModifiers';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { clearTabBarDragSession, useTabBarReorderStore } from './tabBarReorderStore';
import { useEditorDropPreviewStore } from './editorDropPreviewStore';
import { buildTabBarInsertPreview } from './tabBarInsertPreview';
import { clearTabDragHoverOpen, scheduleTabDragHoverOpen } from './tabBarDragHoverOpen';
import {
  cancelDropPreviewStaleGuard,
  refreshDropPreviewStaleGuard,
} from './editorDropPreviewStaleGuard';
import { resolveTabDragTransferIds } from '@/features/core/layout/tabSelection';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { resolveTabDisplayName } from './resolveTabDisplayName';
import { layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import {
  updateSidebarSpawnDropPreviewFromDragMove,
} from './sidebarSpawnDropPreview';

function splitHitOptions(
  event: DragMoveEvent | DragStartEvent,
  isDraggingGroup: boolean,
) {
  const partOptions = readEditorPartOptions();
  const modifiers = readDragModifiers(event);
  return {
    preferSplitVertically: preferSplitVerticallyFromDirection(partOptions.openSideBySideDirection),
    enableSplitting: resolveEnableSplittingOnDrag(partOptions.splitOnDragAndDrop, modifiers),
    isDraggingGroup,
  };
}

function resolveTabDragTitle(tabId: string, sourceGroupId: string): string {
  const tab = useEditorTabStore.getState().resolveTab(tabId);
  if (tab) {
    const resourceRef = layoutTabResourceRef(tab);
    return resolveTabDisplayName(resourceRef, tab.id);
  }
  const element = document.querySelector(
    `[data-tab-id="${tabId}"][data-tab-group="${sourceGroupId}"]`,
  ) as HTMLElement | null;
  const label = element?.dataset.tabTitle;
  return label?.trim() || tabId;
}

function resolveEditorGroupDragTitle(sourceGroupId: string): string {
  const activeTab = getActiveLayoutTab(sourceGroupId)?.tab;
  if (!activeTab) return sourceGroupId;
  const resourceRef = layoutTabResourceRef(activeTab);
  return resolveTabDisplayName(resourceRef, activeTab.id);
}

function shouldSuppressDropPreview(
  activeData: ReturnType<typeof parseCanvasDragPayload>,
  targetGroupId: string,
): boolean {
  if (!activeData) return false;

  if (isEditorGroupDragData(activeData)) {
    return activeData.sourceNodeId === targetGroupId;
  }

  if (isTabDragData(activeData) && activeData.sourceNodeId === targetGroupId) {
    const tabCount = useEditorTabStore.getState().getPlacement(targetGroupId).tabIds.length;
    return tabCount < 2;
  }

  return false;
}

function beginTabDrag(event: DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData)) return;

  const transferIds = activeData.draggedTabIds
    ?? resolveTabDragTransferIds(activeData.sourceNodeId, activeData.tabId);
  const title = resolveTabDragTitle(activeData.tabId, activeData.sourceNodeId);
  const dragTitle = transferIds.length > 1
    ? `${title} (+${transferIds.length - 1})`
    : title;

  useTabBarReorderStore.getState().setActiveGroupDrag(null);
  useTabBarReorderStore.getState().setActiveTabDrag({
    tabId: activeData.tabId,
    sourceGroupId: activeData.sourceNodeId,
    title: dragTitle,
  });
}

function beginEditorGroupDrag(event: DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isEditorGroupDragData(activeData)) return;

  const tabCount = useEditorTabStore.getState().getPlacement(activeData.sourceNodeId).tabIds.length;
  const title = resolveEditorGroupDragTitle(activeData.sourceNodeId);

  useTabBarReorderStore.getState().setActiveTabDrag(null);
  useTabBarReorderStore.getState().setActiveGroupDrag({
    sourceGroupId: activeData.sourceNodeId,
    title,
    tabCount,
  });
}

export function clearEditorDragSession(): void {
  cancelDropPreviewStaleGuard();
  clearTabBarDragSession();
  clearTabDragHoverOpen();
  useEditorDropPreviewStore.getState().clearPreview();
}

function updateTabBarInsertPreviewFromPointer(
  pointerX: number,
  pointerY: number,
  context: TabBarInsertPreviewContext,
): void {
  const strip = findTabBarTargetFromPointer(pointerX, pointerY);
  if (!strip) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  useTabBarReorderStore.getState().setPreview(
    buildTabBarInsertPreview(strip.groupId, strip.stripElement, pointerX, context),
  );
}

function updateTabBarPreviewFromDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData) && !isEditorGroupDragData(activeData)) {
    useTabBarReorderStore.getState().clearPreview();
    return;
  }

  const pointer = resolveDragClientPoint(event);
  if (!pointer) return;

  updateTabBarInsertPreviewFromPointer(pointer.x, pointer.y, {
    draggedTabId: isTabDragData(activeData) ? activeData.tabId : null,
    sourceGroupId: activeData?.sourceNodeId ?? null,
  });
}

function updateEditorDropPreviewFromPointer(
  event: DragMoveEvent | DragStartEvent,
  pointerX: number,
  pointerY: number,
  resourceName?: string,
): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  const targetGroupId = findEditorGroupAtPointer(pointerX, pointerY);
  if (!targetGroupId) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  if (shouldSuppressDropPreview(activeData, targetGroupId)) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const isDraggingGroup = isEditorGroupDragData(activeData);
  const resolved = resolveEditorDropHitAtClientPoint(
    targetGroupId,
    pointerX,
    pointerY,
    splitHitOptions(event, isDraggingGroup),
  );
  if (!resolved) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  if (resolved.hit.mode === 'split') {
    useEditorDropPreviewStore.getState().setPreview({
      kind: 'split',
      targetGroupId,
      edge: resolved.hit.edge,
      rect: resolved.rect,
    });
    return;
  }

  useEditorDropPreviewStore.getState().setPreview({
    kind: 'merge',
    targetGroupId,
    rect: resolved.rect,
    resourceName,
  });
}

function updateSplitDropPreviewFromDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isTabDragData(activeData) && !isEditorGroupDragData(activeData)) return;

  const pointer = resolveDragClientPoint(event);
  if (!pointer) return;

  if (findTabBarTargetFromPointer(pointer.x, pointer.y)) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  updateEditorDropPreviewFromPointer(event, pointer.x, pointer.y);
}

function handleDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  const pointer = resolveDragClientPoint(event);

  if (pointer) {
    refreshDropPreviewStaleGuard(pointer.x, pointer.y);
  }

  if (isSidebarSpawnDrag(activeData)) {
    updateSidebarSpawnDropPreviewFromDragMove(event);
    return;
  }

  if (isTabDragData(activeData) || isEditorGroupDragData(activeData)) {
    updateTabBarPreviewFromDragMove(event);
    updateSplitDropPreviewFromDragMove(event);
    if (pointer && isTabDragData(activeData)) {
      scheduleTabDragHoverOpen(pointer.x, pointer.y);
    } else {
      clearTabDragHoverOpen();
    }
    return;
  }

  useTabBarReorderStore.getState().clearPreview();
  clearTabDragHoverOpen();
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
 * Unified drag preview monitor: tab/group reorder, VS Code-style split zones, sidebar graph open/split.
 */
export function useEditorDragPreviewMonitor(): void {
  useDndMonitor({
    onDragStart: (event) => {
      const activeData = parseCanvasDragPayload(event.active.data.current);
      if (isTabDragData(activeData)) {
        beginTabDrag(event);
      } else if (isEditorGroupDragData(activeData)) {
        beginEditorGroupDrag(event);
      } else {
        useTabBarReorderStore.getState().setActiveTabDrag(null);
        useTabBarReorderStore.getState().setActiveGroupDrag(null);
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
