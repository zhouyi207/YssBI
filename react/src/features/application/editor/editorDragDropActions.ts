import type { DragEndEvent, DragStartEvent } from "@dnd-kit/core";

import { handleGraphResourceDrop } from "./handleGraphResourceDrop";

import {
  resolveDropIntoEditorDragState,
  resolveDropPointerFromDragEnd,
  tryDropFunctionIntoCanvas,
  type CanvasDropTarget,
} from "./dropFunctionIntoEventEditor";
import { canvasDropHandlerStore, useSidebarDragStore } from "@/features/core/sidebarDrag";
import { sidebarDragUi, useSidebarDragUi } from "@/features/core/sidebarDrag/ui";
import { workbenchLayoutControl } from "@/modules/workbench/public";
import type { SidebarDragPayload } from "@/features/core/dnd";
import {
  findSidebarDropCanvasAtPointer,
  isSidebarSpawnDropAllowed,
} from "./sidebarSpawnDropPolicy";
import {
  getSidebarDragOverlayLabel,
  isGraphResourceDragPayload,
  isGraphConstantDragPayload,
  isSidebarSpawnDrag,
  parseCanvasDragPayload,
  buildSidebarDragState,
} from "@/features/core/dnd";
import { formatApplicationIpcError } from "@/features/application/errorReference";
import { logger } from "@/utils/frontendLogger";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

let activeDragProject: ProjectIdentitySnapshot | null = null;

export function useActivityEditorDragOverlayLabel(): string | null {
  const activeDrag = useSidebarDragUi((state) => state.activeDrag);
  return activeDrag ? getSidebarDragOverlayLabel(activeDrag) : null;
}

export function beginActivityEditorDrag(event: DragStartEvent): boolean {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isSidebarSpawnDrag(activeData)) return false;
  try {
    activeDragProject = captureProjectIdentity();
  } catch {
    return false;
  }
  const activatorEvent = event.activatorEvent as PointerEvent;
  sidebarDragUi.setActiveDrag(
    buildSidebarDragState(activeData, activatorEvent?.clientX ?? 0, activatorEvent?.clientY ?? 0),
  );
  return true;
}

export function updateActivityEditorDragPointer(event: PointerEvent): void {
  sidebarDragUi.updatePosition(event.clientX, event.clientY);
}

export function finishActivityEditorDrag(): void {
  activeDragProject = null;
  sidebarDragUi.setActiveDrag(null);
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
    ((overData as { graphKind?: unknown }).graphKind === "event_graph" ||
      (overData as { graphKind?: unknown }).graphKind === "function_graph")
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
  const dropPointer = resolveDropPointerFromDragEnd(event);
  const capturedSidebarDrag = useSidebarDragStore.getState().activeDrag;
  const project = activeDragProject;
  options.finishSidebarDrag();

  if (
    !project ||
    !isCurrentProjectIdentity(project) ||
    !isSidebarSpawnDropAllowed(activeData, dropPointer)
  ) {
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
    if (target && dropState && sidebarResource.type === "function_graph") {
      const handled = await tryDropFunctionIntoCanvas(target, dropState, project);
      if (handled) {
        return;
      }
    }
    if (target && isCurrentProjectIdentity(project))
      await handleGraphResourceDrop(sidebarResource, target.groupId);
    return;
  }

  const target = resolveCanvasDropTarget(event, dropPointer);
  if (!target || !capturedSidebarDrag || capturedSidebarDrag.type !== activeData.type) return;
  if (isGraphConstantDragPayload(activeData) && activeData.graphPath !== target.graphPath) return;
  if (
    !(await workbenchLayoutControl.activate(target.panelInstanceId)) ||
    !isCurrentProjectIdentity(project)
  )
    return;
  const handler = canvasDropHandlerStore.getHandler(target.panelInstanceId);
  if (handler) await handler({ ...capturedSidebarDrag, ...dropPointer });
}

/** Handle only sidebar-to-editor DnD; FlexLayout owns tab/group drag, order, move, and split. */
export async function executeEditorDragEnd(
  event: DragEndEvent,
  options: { finishSidebarDrag: () => void },
): Promise<void> {
  const activeData = parseCanvasDragPayload(event.active.data.current);
  if (!isSidebarSpawnDrag(activeData)) {
    options.finishSidebarDrag();
    return;
  }

  try {
    await executeSidebarSpawnDragEnd(event, activeData, options);
  } catch (error) {
    logger.graph.error(
      `Editor drag/drop failed: ${formatApplicationIpcError(error)}`,
      "EditorDragDrop",
    );
  }
}
