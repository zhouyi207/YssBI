import type { DragEndEvent } from "@dnd-kit/core";
import type { GraphResourceDragData } from "@/features/core/dnd";
import {
  DRAG_TYPES,
  resolveDragClientPoint,
  type GraphResourceDragState,
  type SidebarDragState,
} from "@/features/core/dnd";
import { canCreateFunctionNodeInGraph } from "@/features/application/editor/canvasDrop";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";
import { useSidebarDragStore } from "@/features/core/sidebarDrag";

export function resolveDropPointerFromDragEnd(
  event: Pick<DragEndEvent, "activatorEvent" | "delta">,
): {
  x: number;
  y: number;
} | null {
  return resolveDragClientPoint(event);
}

export function buildFunctionGraphResourceDragState(
  functionPath: string,
  name: string,
  clientX: number,
  clientY: number,
): GraphResourceDragState {
  return {
    type: DRAG_TYPES.GRAPH_RESOURCE,
    sidebarResource: { id: functionPath, name, type: "function" },
    x: clientX,
    y: clientY,
  };
}

export function resolveFunctionDragState(
  resource: GraphResourceDragData,
  clientX: number,
  clientY: number,
): GraphResourceDragState {
  return buildFunctionGraphResourceDragState(resource.id, resource.name, clientX, clientY);
}

export interface CanvasDropTarget {
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: "event" | "function";
}

export async function tryDropFunctionIntoCanvas(
  target: CanvasDropTarget,
  dragState: SidebarDragState,
  modifiers: { shiftKey: boolean; altKey: boolean; ctrlKey: boolean },
): Promise<boolean> {
  if (dragState.type !== DRAG_TYPES.GRAPH_RESOURCE) return false;
  if (!canCreateFunctionNodeInGraph(target.graphKind, target.graphPath, dragState.sidebarResource))
    return false;

  const handler = canvasDropHandlerStore.getHandler(target.panelInstanceId);
  if (!handler) return false;

  if (!(await workbenchDockviewControl.activate(target.panelInstanceId))) return false;
  const handled = await handler(dragState, modifiers);
  return handled === true;
}

export function readSidebarDragPointer(): { x: number; y: number } | null {
  const dragState = useSidebarDragStore.getState().activeDrag;
  if (!dragState) return null;
  return { x: dragState.x, y: dragState.y };
}

export function resolveDropIntoEditorDragState(
  resource: GraphResourceDragData,
  pointer?: { x: number; y: number } | null,
  capturedDrag?: SidebarDragState | null,
): SidebarDragState | null {
  const activeDrag = capturedDrag ?? useSidebarDragStore.getState().activeDrag;
  if (activeDrag?.type === DRAG_TYPES.GRAPH_RESOURCE) {
    if (pointer) {
      return { ...activeDrag, x: pointer.x, y: pointer.y };
    }
    return activeDrag;
  }
  const resolvedPointer = pointer ?? readSidebarDragPointer();
  if (!resolvedPointer) return null;
  return resolveFunctionDragState(resource, resolvedPointer.x, resolvedPointer.y);
}
