import type { DragMoveEvent, DragStartEvent } from '@dnd-kit/core';
import {
  isGraphResourceDragPayload,
  isSidebarSpawnDrag,
  parseCanvasDragPayload,
  readDragModifiers,
  resolveDragClientPoint,
  type GraphResourceDragPayload,
} from '@/features/core/dnd';
import { isSidebarSpawnDropAllowedAtPointer } from './sidebarSpawnDropPolicy';
import { useEditorDropPreviewStore } from './editorDropPreviewStore';
import { useTabBarReorderStore } from './tabBarReorderStore';
import {
  findEditorGroupAtPointer,
  findTabBarTargetFromPointer,
} from '@/features/core/layout/editorDropTarget';
import {
  preferSplitVerticallyFromDirection,
  readEditorPartOptions,
} from '@/features/core/layout/editorPartOptions';
import { resolveEnableSplittingOnDrag } from '@/features/core/layout/editorDragModifiers';
import { resolveEditorDropHitAtClientPoint } from '@/features/core/layout/editorDropPreview';
import { resolveSidebarGraphResourceDropPreview } from './resolveSidebarGraphResourceDropPreview';
import { buildTabBarInsertPreview } from './tabBarInsertPreview';

function splitHitOptions(
  event: DragMoveEvent | DragStartEvent,
) {
  const partOptions = readEditorPartOptions();
  const modifiers = readDragModifiers(event);
  return {
    preferSplitVertically: preferSplitVerticallyFromDirection(partOptions.openSideBySideDirection),
    enableSplitting: resolveEnableSplittingOnDrag(partOptions.splitOnDragAndDrop, modifiers),
    isDraggingGroup: false,
  };
}

export function clearSidebarSpawnDropPreviews(): void {
  useTabBarReorderStore.getState().clearPreview();
  useEditorDropPreviewStore.getState().clearPreview();
}

function updateGraphResourceDropPreviewFromDragMove(
  event: DragMoveEvent | DragStartEvent,
  activeData: GraphResourceDragPayload,
  pointerX: number,
  pointerY: number,
): void {
  const modifiers = readDragModifiers(event);

  const tabBarTarget = findTabBarTargetFromPointer(pointerX, pointerY);
  if (tabBarTarget) {
    useEditorDropPreviewStore.getState().clearPreview();
    useTabBarReorderStore.getState().setPreview(
      buildTabBarInsertPreview(
        tabBarTarget.groupId,
        tabBarTarget.stripElement,
        pointerX,
        { draggedTabId: null, sourceGroupId: null },
      ),
    );
    return;
  }

  useTabBarReorderStore.getState().clearPreview();

  const targetGroupId = findEditorGroupAtPointer(pointerX, pointerY);
  if (!targetGroupId) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  const resolved = resolveEditorDropHitAtClientPoint(
    targetGroupId,
    pointerX,
    pointerY,
    splitHitOptions(event),
  );
  if (!resolved) {
    useEditorDropPreviewStore.getState().clearPreview();
    return;
  }

  useEditorDropPreviewStore.getState().setPreview(
    resolveSidebarGraphResourceDropPreview(
      activeData.sidebarResource,
      targetGroupId,
      resolved,
      modifiers.shiftKey,
    ),
  );
}

/** Sidebar spawn drags: preview only on editor workbench; chrome panels are rejected. */
export function updateSidebarSpawnDropPreviewFromDragMove(
  event: DragMoveEvent | DragStartEvent,
): void {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isSidebarSpawnDrag(activeData)) return;

  const pointer = resolveDragClientPoint(event);
  if (!pointer || !isSidebarSpawnDropAllowedAtPointer(pointer.x, pointer.y)) {
    clearSidebarSpawnDropPreviews();
    return;
  }

  if (isGraphResourceDragPayload(activeData)) {
    updateGraphResourceDropPreviewFromDragMove(event, activeData, pointer.x, pointer.y);
    return;
  }

  clearSidebarSpawnDropPreviews();
}
