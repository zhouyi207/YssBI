import type { DragEndEvent } from '@dnd-kit/core';
import { handleGraphResourceDrop } from './handleGraphResourceDrop';

import { clearEditorDragSession } from './useEditorDragPreviewMonitor';
import {
  resolveDropIntoEditorDragState,
  resolveDropPointerFromDragEnd,
  tryDropFunctionIntoEventCanvas,
} from './dropFunctionIntoEventEditor';
import { activateEditorGroup } from './switchEditorTab';
import { canvasDropHandlerStore, useSidebarDragStore } from '@/features/core/sidebarDrag';
import type { SidebarDragPayload } from '@/features/core/dnd';
import { isSidebarSpawnDropAllowed } from './sidebarSpawnDropPolicy';
import {
  isGraphResourceDragPayload,
  isNodeTemplateDragData,
  isNodeTemplateDragState,
  isSidebarSpawnDrag,
  parseCanvasDragPayload,
  readDragModifiers,
} from '@/features/core/dnd';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';

export function readEditorDragModifiers(event: DragEndEvent): {
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
} {
  return readDragModifiers(event);
}

function resolveCanvasDropGroupId(event: DragEndEvent): string | null {
  const overData = event.over?.data.current;
  if (overData && typeof overData === 'object' && 'groupId' in overData) {
    return String((overData as { groupId: string }).groupId);
  }
  return null;
}

async function executeSidebarSpawnDragEnd(
  event: DragEndEvent,
  activeData: SidebarDragPayload,
  options: { finishSidebarDrag: () => void },
): Promise<void> {
  const modifiers = readEditorDragModifiers(event);
  const dropPointer = resolveDropPointerFromDragEnd(event);
  const capturedSidebarDrag = useSidebarDragStore.getState().activeDrag;
  options.finishSidebarDrag();

  if (!isSidebarSpawnDropAllowed(activeData, dropPointer)) {
    clearEditorDragSession();
    return;
  }

  if (isGraphResourceDragPayload(activeData)) {
    const { sidebarResource } = activeData;
    const groupId = resolveCanvasDropGroupId(event);
    const dropState = resolveDropIntoEditorDragState(sidebarResource, dropPointer, capturedSidebarDrag);
    if (groupId && dropState && modifiers.shiftKey) {
      const handled = await tryDropFunctionIntoEventCanvas(groupId, dropState, modifiers);
      if (handled) {
        clearEditorDragSession();
        return;
      }
    }
    if (groupId) await handleGraphResourceDrop(sidebarResource, groupId);
    clearEditorDragSession();
    return;
  }

  if (isNodeTemplateDragData(activeData)) {
    const groupId = resolveCanvasDropGroupId(event);
    if (groupId && capturedSidebarDrag && isNodeTemplateDragState(capturedSidebarDrag)) {
      void activateEditorGroup(groupId);
      const handler = canvasDropHandlerStore.getHandler(groupId);
      if (handler) await handler(capturedSidebarDrag, modifiers);
    }
  }

  clearEditorDragSession();
}

/** Handle only sidebar-to-editor DnD; Dockview owns tab/group drag, order, move, and split. */
export async function executeEditorDragEnd(
  event: DragEndEvent,
  options: { finishSidebarDrag: () => void },
): Promise<void> {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isSidebarSpawnDrag(activeData)) {
    clearEditorDragSession();
    return;
  }

  try {
    await executeSidebarSpawnDragEnd(event, activeData, options);
  } catch (error) {
    logger.notify.error(formatErrorMessage(error), 'UI');
    clearEditorDragSession();
  }
}
