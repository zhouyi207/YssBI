import type { DragEndEvent } from '@dnd-kit/core';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { handleGraphResourceDrop } from '@/features/application/editor/handleGraphResourceDrop';
import {
  copyEditorGroupInto,
  copyEditorGroupWithSplit,
  copyTabsBetweenGroups,
  mergeEditorGroupInto,
  moveTabsBetweenGroups,
  splitEditorGroupWithGroup,
  splitEditorWithTab,
  splitOrMoveSingleTabGroup,
} from '@/features/application/editor/editorGroupCommands';
import { resolveTabBarDropIndex } from '@/features/application/editor/tabBarReorderStore';
import { useEditorDropPreviewStore } from '@/features/application/editor/editorDropPreviewStore';
import { clearEditorDragSession } from '@/features/application/editor/useEditorDragPreviewMonitor';
import {
  resolveDropIntoEditorDragState,
  resolveDropIntoEditorDragStateFromTab,
  resolveDropPointerFromDragEnd,
  resolveFunctionTabForDrop,
  tryDropFunctionIntoEventCanvas,
} from '@/features/application/editor/dropFunctionIntoEventEditor';
import { uiStore } from '@/features/core/ui/UIStore';
import { activateEditorGroup } from '@/features/application/editor/switchEditorTab';
import { useSidebarDragStore, canvasDropHandlerStore } from '@/features/core/sidebarDrag';
import { useModifierKeyStore } from '@/features/core/keyboard';
import { isEditorDragCopyOperation } from '@/features/core/layout/editorDragModifiers';
import { findEditorGroupAtPointer } from '@/features/core/layout/editorDropTarget';
import { isSidebarItemDropAllowedAtPointer } from '@/features/core/layout/workbenchSidebarDropSurface';
import {
  isTabbarDrop,
  isEditorGroupDragData,
  isNodeTemplateDragData,
  isNodeTemplateDragState,
  isTabDragData,
  getSidebarResourceFromDrag,
  parseCanvasDragPayload,
  isSidebarSpawnDrag,
} from '@/features/core/dnd';

export function readEditorDragModifiers(event: DragEndEvent): {
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
} {
  const activator = event.activatorEvent;
  const modifierStore = useModifierKeyStore.getState();
  if (activator instanceof MouseEvent || activator instanceof PointerEvent) {
    return {
      altKey: activator.altKey || modifierStore.altKey,
      ctrlKey: activator.ctrlKey || modifierStore.ctrlKey,
      shiftKey: activator.shiftKey || modifierStore.shiftKey,
    };
  }
  return {
    altKey: modifierStore.altKey,
    ctrlKey: modifierStore.ctrlKey,
    shiftKey: modifierStore.shiftKey,
  };
}

function resolveCanvasDropGroupId(
  event: DragEndEvent,
  preview: ReturnType<typeof useEditorDropPreviewStore.getState>['preview'],
): string | null {
  const overData = event.over?.data.current;
  if (preview?.targetGroupId) return preview.targetGroupId;
  if (overData && typeof overData === 'object' && overData !== null && 'groupId' in overData) {
    return String((overData as { groupId: string }).groupId);
  }
  const activator = event.activatorEvent;
  if (activator instanceof MouseEvent || activator instanceof PointerEvent) {
    const delta = event.delta;
    return findEditorGroupAtPointer(
      activator.clientX + delta.x,
      activator.clientY + delta.y,
    );
  }
  return null;
}

function isSidebarSpawnDropAllowed(
  activeData: ReturnType<typeof parseCanvasDragPayload>,
  pointer: { x: number; y: number } | null,
): pointer is { x: number; y: number } {
  if (!isSidebarSpawnDrag(activeData) || !pointer) return false;
  return isSidebarItemDropAllowedAtPointer(pointer.x, pointer.y);
}

export async function executeEditorDragEnd(
  event: DragEndEvent,
  options: { finishSidebarDrag: () => void },
): Promise<void> {
  const { active, over } = event;
  const activeData = parseCanvasDragPayload(active.data.current);
  const overData = over?.data.current;
  const sidebarResource = getSidebarResourceFromDrag(activeData);
  const preview = useEditorDropPreviewStore.getState().preview;
  const modifiers = readEditorDragModifiers(event);
  const isCopy = isEditorDragCopyOperation(modifiers);
  const dropPointer = resolveDropPointerFromDragEnd(event);
  const capturedSidebarDrag = useSidebarDragStore.getState().activeDrag;

  if (sidebarResource) {
    options.finishSidebarDrag();
    if (!isSidebarSpawnDropAllowed(activeData, dropPointer)) {
      clearEditorDragSession();
      return;
    }
    if (isTabbarDrop(overData)) {
      void handleGraphResourceDrop(
        sidebarResource,
        overData.targetNodeId,
        { insertIndex: resolveTabBarDropIndex(overData.targetNodeId, overData.targetTabIndex) },
      ).catch((error) => uiStore.showToast(formatErrorMessage(error), 'error'));
    } else {
      const groupId = resolveCanvasDropGroupId(event, preview);
      const dropState = resolveDropIntoEditorDragState(
        sidebarResource,
        dropPointer,
        capturedSidebarDrag,
      );
      if (groupId && dropState) {
        const handled = await tryDropFunctionIntoEventCanvas(groupId, dropState, modifiers);
        if (handled) {
          clearEditorDragSession();
          return;
        }
      }
      if (preview) {
        void handleGraphResourceDrop(
          sidebarResource,
          preview.targetGroupId,
          preview.kind === 'split' ? { edge: preview.edge } : undefined,
        ).catch((error) => uiStore.showToast(formatErrorMessage(error), 'error'));
      }
    }
    clearEditorDragSession();
    return;
  }

  if (isNodeTemplateDragData(activeData)) {
    const dragState = useSidebarDragStore.getState().activeDrag;
    options.finishSidebarDrag();
    if (!isSidebarSpawnDropAllowed(activeData, dropPointer)) {
      clearEditorDragSession();
      return;
    }
    const groupId = resolveCanvasDropGroupId(event, preview);
    if (groupId && dragState && isNodeTemplateDragState(dragState)) {
      void activateEditorGroup(groupId);
      const handler = canvasDropHandlerStore.getHandler(groupId);
      if (handler) {
        await handler(dragState, modifiers);
      }
    }
    clearEditorDragSession();
    return;
  }

  if (isEditorGroupDragData(activeData)) {
    const { sourceNodeId } = activeData;

    if (isTabbarDrop(overData) && overData.targetNodeId !== sourceNodeId) {
      const insertIndex = resolveTabBarDropIndex(overData.targetNodeId, overData.targetTabIndex);
      if (isCopy) {
        copyEditorGroupInto(sourceNodeId, overData.targetNodeId, insertIndex);
      } else {
        mergeEditorGroupInto(sourceNodeId, overData.targetNodeId, insertIndex);
      }
    } else if (preview?.kind === 'split' && preview.targetGroupId !== sourceNodeId) {
      if (isCopy) {
        void copyEditorGroupWithSplit(sourceNodeId, preview.targetGroupId, preview.edge);
      } else {
        void splitEditorGroupWithGroup(sourceNodeId, preview.targetGroupId, preview.edge);
      }
    } else if (preview?.kind === 'merge' && preview.targetGroupId !== sourceNodeId) {
      if (isCopy) {
        copyEditorGroupInto(sourceNodeId, preview.targetGroupId);
      } else {
        mergeEditorGroupInto(sourceNodeId, preview.targetGroupId);
      }
    }

    clearEditorDragSession();
    return;
  }

  if (isTabDragData(activeData)) {
    const { sourceNodeId, tabId, draggedTabIds } = activeData;
    const transferTabIds = draggedTabIds?.length ? draggedTabIds : [tabId];

    if (!isTabbarDrop(overData) && preview?.kind === 'merge' && modifiers.shiftKey) {
      const tab = resolveFunctionTabForDrop(tabId);
      const dropState = tab ? resolveDropIntoEditorDragStateFromTab(tab, dropPointer) : null;
      if (tab && dropState) {
        const handled = await tryDropFunctionIntoEventCanvas(preview.targetGroupId, dropState, modifiers);
        if (handled) {
          clearEditorDragSession();
          return;
        }
      }
    }

    if (isTabbarDrop(overData)) {
      if (isCopy) {
        copyTabsBetweenGroups(
          sourceNodeId,
          transferTabIds,
          overData.targetNodeId,
          resolveTabBarDropIndex(overData.targetNodeId, overData.targetTabIndex),
        );
      } else {
        moveTabsBetweenGroups(
          sourceNodeId,
          transferTabIds,
          overData.targetNodeId,
          resolveTabBarDropIndex(overData.targetNodeId, overData.targetTabIndex),
        );
      }
    } else if (preview?.kind === 'split') {
      if (transferTabIds.length > 1) {
        for (const [index, transferTabId] of transferTabIds.entries()) {
          if (isCopy) {
            void splitEditorWithTab(sourceNodeId, transferTabId, preview.targetGroupId, preview.edge, { copy: true });
          } else if (index === 0) {
            void splitOrMoveSingleTabGroup(sourceNodeId, transferTabId, preview.targetGroupId, preview.edge);
          } else {
            moveTabsBetweenGroups(sourceNodeId, [transferTabId], preview.targetGroupId);
          }
        }
      } else if (isCopy) {
        void splitEditorWithTab(sourceNodeId, tabId, preview.targetGroupId, preview.edge, { copy: true });
      } else {
        void splitOrMoveSingleTabGroup(sourceNodeId, tabId, preview.targetGroupId, preview.edge);
      }
    } else if (preview?.kind === 'merge') {
      if (isCopy) {
        copyTabsBetweenGroups(sourceNodeId, transferTabIds, preview.targetGroupId);
      } else {
        moveTabsBetweenGroups(sourceNodeId, transferTabIds, preview.targetGroupId);
      }
    }
  }

  clearEditorDragSession();
}
