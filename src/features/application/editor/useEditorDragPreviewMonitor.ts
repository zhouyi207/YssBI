import { useEffect } from 'react';
import { useDndMonitor, type DragMoveEvent, type DragStartEvent } from '@dnd-kit/core';
import {
  isEditorGroupDragData,
  isGraphResourceDragPayload,
  isSidebarSpawnDrag,
  isTabDragData,
  parseCanvasDragPayload,
  type GraphResourceDragPayload,
} from '@/features/core/dnd';
import { resolveEditorDropHitAtClientPoint } from '@/features/core/layout/editorDropPreview';
import {
  findCanvasDropGroupId,
  findEditorGroupAtPointer,
  findTabBarTargetFromPointer,
  type TabBarInsertPreviewContext,
} from '@/features/core/layout/editorDropTarget';
import { isSidebarItemDropAllowedAtPointer } from '@/features/core/layout/workbenchSidebarDropSurface';
import {
  preferSplitVerticallyFromDirection,
  readEditorPartOptions,
} from '@/features/core/layout/editorPartOptions';
import { resolveEnableSplittingOnDrag } from '@/features/core/layout/editorDragModifiers';
import { useModifierKeyStore } from '@/features/core/keyboard';
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
import { canDropFunctionIntoEventGraph } from '@/features/application/editor/canvasDrop';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { resolveTabDisplayName } from './resolveTabDisplayName';
import { layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';

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

function dragModifiersFromEvent(event: DragMoveEvent | DragStartEvent): {
  altKey: boolean;
  shiftKey: boolean;
  ctrlKey: boolean;
} {
  const activator = event.activatorEvent;
  const modifierStore = useModifierKeyStore.getState();
  if (activator instanceof MouseEvent || activator instanceof PointerEvent) {
    return {
      altKey: activator.altKey || modifierStore.altKey,
      shiftKey: activator.shiftKey || modifierStore.shiftKey,
      ctrlKey: activator.ctrlKey || modifierStore.ctrlKey,
    };
  }
  return {
    altKey: modifierStore.altKey,
    shiftKey: modifierStore.shiftKey,
    ctrlKey: modifierStore.ctrlKey,
  };
}

function splitHitOptions(
  event: DragMoveEvent | DragStartEvent,
  isDraggingGroup: boolean,
) {
  const partOptions = readEditorPartOptions();
  const modifiers = dragModifiersFromEvent(event);
  return {
    preferSplitVertically: preferSplitVerticallyFromDirection(partOptions.openSideBySideDirection),
    enableSplitting: resolveEnableSplittingOnDrag(partOptions.splitOnDragAndDrop, modifiers),
    isDraggingGroup,
  };
}

function readCanvasDropGroupIdFromOver(over: DragMoveEvent['over'] | null): string | null {
  const overData = over?.data.current;
  if (overData && typeof overData === 'object' && overData !== null && 'groupId' in overData) {
    return String((overData as { groupId: string }).groupId);
  }
  return null;
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

function clearSidebarSpawnDropPreviews(): void {
  useTabBarReorderStore.getState().clearPreview();
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

  const pointer = pointerFromDragEvent(event);
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

  const pointer = pointerFromDragEvent(event);
  if (!pointer) return;

  if (findTabBarTargetFromPointer(pointer.x, pointer.y)) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  updateEditorDropPreviewFromPointer(event, pointer.x, pointer.y);
}

function updateGraphResourceDropPreviewFromDragMove(
  event: DragMoveEvent | DragStartEvent,
  activeData: GraphResourceDragPayload,
): void {
  const pointer = pointerFromDragEvent(event);
  if (!pointer) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const modifiers = dragModifiersFromEvent(event);

  if (findTabBarTargetFromPointer(pointer.x, pointer.y)) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const targetGroupId = findEditorGroupAtPointer(pointer.x, pointer.y);
  if (
    activeData.sidebarResource.type === 'function'
    && targetGroupId
    && canDropFunctionIntoEventGraph(targetGroupId, activeData.sidebarResource, true)
  ) {
    const resolved = resolveEditorDropHitAtClientPoint(
      targetGroupId,
      pointer.x,
      pointer.y,
      splitHitOptions(event, false),
    );
    if (resolved) {
      useEditorDropPreviewStore.getState().setPreview({
        kind: 'function-into-event',
        targetGroupId,
        rect: resolved.rect,
        shiftHeld: modifiers.shiftKey,
      });
      return;
    }
  }

  const groupFromCanvas = findCanvasDropGroupId(
    pointer.x,
    pointer.y,
    readCanvasDropGroupIdFromOver('over' in event ? event.over : null),
  );
  if (!groupFromCanvas && !findEditorGroupAtPointer(pointer.x, pointer.y)) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  updateEditorDropPreviewFromPointer(
    event,
    pointer.x,
    pointer.y,
    activeData.sidebarResource.name,
  );
}

function handleDragMove(event: DragMoveEvent | DragStartEvent): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  const pointer = pointerFromDragEvent(event);

  if (pointer) {
    refreshDropPreviewStaleGuard(pointer.x, pointer.y);
  }

  if (isSidebarSpawnDrag(activeData)) {
    if (!pointer || !isSidebarItemDropAllowedAtPointer(pointer.x, pointer.y)) {
      clearSidebarSpawnDropPreviews();
      return;
    }
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

  if (isGraphResourceDragPayload(activeData)) {
    if (pointer && findTabBarTargetFromPointer(pointer.x, pointer.y)) {
      useEditorDropPreviewStore.getState().clearPreview();
      updateTabBarInsertPreviewFromPointer(pointer.x, pointer.y, {
        draggedTabId: null,
        sourceGroupId: null,
      });
      return;
    }

    useTabBarReorderStore.getState().clearPreview();
    updateGraphResourceDropPreviewFromDragMove(event, activeData);
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
