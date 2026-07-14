import type { DragMoveEvent, DragStartEvent } from '@dnd-kit/core';
import {
  isGraphResourceDragPayload,
  isSidebarSpawnDrag,
  parseCanvasDragPayload,
  type GraphResourceDragPayload,
} from '@/features/core/dnd';
import { isSidebarSpawnDropAllowedAtPointer } from './sidebarSpawnDropPolicy';
import { useEditorDropPreviewStore } from './editorDropPreviewStore';
import { useTabBarReorderStore } from './tabBarReorderStore';
import {
  findEditorGroupAtPointer,
  findTabBarTargetFromPointer,
} from '@/features/core/layout/editorDropTarget';
import { useModifierKeyStore } from '@/features/core/keyboard';
import {
  preferSplitVerticallyFromDirection,
  readEditorPartOptions,
} from '@/features/core/layout/editorPartOptions';
import { resolveEnableSplittingOnDrag } from '@/features/core/layout/editorDragModifiers';
import { resolveEditorDropHitAtClientPoint } from '@/features/core/layout/editorDropPreview';
import { resolveSidebarGraphResourceDropPreview } from './resolveSidebarGraphResourceDropPreview';
import { buildTabBarInsertPreview } from './tabBarInsertPreview';

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
) {
  const partOptions = readEditorPartOptions();
  const modifiers = dragModifiersFromEvent(event);
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
  const modifiers = dragModifiersFromEvent(event);

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

  const pointer = pointerFromDragEvent(event);
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
