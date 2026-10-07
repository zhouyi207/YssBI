import type { GraphConstantDragState } from "@/features/core/dnd";
import { getGraphConstants } from "@/features/core/graph/read";
import { createGraphConstantNode } from "@/features/application/graphEditing/graphConstantActions";
import { captureEditorCommandTarget, isEditorCommandTargetCurrent } from "../editorCommandFocus";
import { clientToWorldInCanvas, isPointInsideCanvas } from "./canvasGeometry";

export async function dropGraphConstantIntoCanvas(
  canvas: HTMLElement,
  panelInstanceId: string,
  groupId: string,
  graphPath: string,
  dragState: GraphConstantDragState,
): Promise<boolean> {
  if (dragState.graphPath !== graphPath || !isPointInsideCanvas(canvas, dragState.x, dragState.y))
    return false;

  const target = captureEditorCommandTarget(panelInstanceId);
  if (
    !target ||
    target.groupId !== groupId ||
    target.resourceRef !== graphPath ||
    (target.resourceKind !== "event_graph" && target.resourceKind !== "function_graph") ||
    !isEditorCommandTargetCurrent(target) ||
    !getGraphConstants(graphPath)?.[dragState.constantId]
  )
    return false;

  const position = clientToWorldInCanvas(canvas, groupId, graphPath, dragState.x, dragState.y);
  const outcome = await createGraphConstantNode(graphPath, dragState.constantId, position);
  return outcome.status === "applied";
}
