import type { DragEndEvent, DragStartEvent } from "@dnd-kit/core";

import { handleGraphResourceDrop } from "./handleGraphResourceDrop";

import { clearEditorDragSession } from "./useEditorDragPreviewMonitor";
import {
  resolveDropIntoEditorDragState,
  resolveDropPointerFromDragEnd,
  tryDropFunctionIntoCanvas,
  type CanvasDropTarget,
} from "./dropFunctionIntoEventEditor";
import { canvasDropHandlerStore, useSidebarDragStore } from "@/features/core/sidebarDrag";
import { sidebarDragUi } from "@/features/core/sidebarDrag/ui";
import { workbenchDockviewControl } from "@/modules/workbench/public";
import type { SidebarDragPayload } from "@/features/core/dnd";
import {
  findSidebarDropCanvasAtPointer,
  isSidebarSpawnDropAllowed,
} from "./sidebarSpawnDropPolicy";
import {
  isGraphResourceDragPayload,
  isNodeTemplateDragData,
  isNodeTemplateDragState,
  isSidebarSpawnDrag,
  parseCanvasDragPayload,
  readDragModifiers,
  buildSidebarDragState,
} from "@/features/core/dnd";
import { keyboardUi } from "@/features/core/keyboard/ui";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { logger } from "@/features/application/observability/appLogger";

export function beginActivityEditorDrag(event: DragStartEvent): boolean {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isSidebarSpawnDrag(activeData)) return false;
  const activatorEvent = event.activatorEvent as PointerEvent;
  sidebarDragUi.setActiveDrag(
    buildSidebarDragState(activeData, activatorEvent?.clientX ?? 0, activatorEvent?.clientY ?? 0),
  );
  return true;
}

export function updateActivityEditorDragPointer(event: PointerEvent): void {
  sidebarDragUi.updatePosition(event.clientX, event.clientY);
  keyboardUi.setModifierKeys(event);
}

export function finishActivityEditorDrag(): void {
  sidebarDragUi.setActiveDrag(null);
}

export function readEditorDragModifiers(event: DragEndEvent): {
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
} {
  return readDragModifiers(event);
}

function resolveCanvasDropTarget(
  event: DragEndEvent,
  dropPointer: { x: number; y: number } | null,
): CanvasDropTarget | null {
  const overData = event.over?.data.current;
  if (
    overData &&
    typeof overData === "object" &&
    "panelInstanceId" in overData &&
    "groupId" in overData &&
    "graphPath" in overData &&
    "graphKind" in overData &&
    typeof (overData as { panelInstanceId?: unknown }).panelInstanceId === "string" &&
    typeof (overData as { groupId?: unknown }).groupId === "string" &&
    typeof (overData as { graphPath?: unknown }).graphPath === "string" &&
    ((overData as { graphKind?: unknown }).graphKind === "event" ||
      (overData as { graphKind?: unknown }).graphKind === "function")
  ) {
    return overData as CanvasDropTarget;
  }
  if (!dropPointer) return null;
  const canvas = findSidebarDropCanvasAtPointer(dropPointer.x, dropPointer.y);
  return canvas
    ? {
        panelInstanceId: canvas.panelInstanceId,
        groupId: canvas.groupId,
        graphPath: canvas.graphPath,
        graphKind: canvas.graphKind,
      }
    : null;
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
    const target = resolveCanvasDropTarget(event, dropPointer);
    const dropState = resolveDropIntoEditorDragState(
      sidebarResource,
      dropPointer,
      capturedSidebarDrag,
    );
    if (target && dropState && sidebarResource.type === "function") {
      const handled = await tryDropFunctionIntoCanvas(target, dropState, modifiers);
      if (handled) {
        clearEditorDragSession();
        return;
      }
    }
    if (target) await handleGraphResourceDrop(sidebarResource, target.groupId);
    clearEditorDragSession();
    return;
  }

  if (isNodeTemplateDragData(activeData)) {
    const target = resolveCanvasDropTarget(event, dropPointer);
    if (target && capturedSidebarDrag && isNodeTemplateDragState(capturedSidebarDrag)) {
      if (!(await workbenchDockviewControl.activate(target.panelInstanceId))) {
        clearEditorDragSession();
        return;
      }
      const handler = canvasDropHandlerStore.getHandler(target.panelInstanceId);
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
    logger.graph.error(`Editor drag/drop failed: ${formatErrorMessage(error)}`, "EditorDragDrop");
    clearEditorDragSession();
  }
}
