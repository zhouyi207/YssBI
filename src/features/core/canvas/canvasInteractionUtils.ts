import { CONTEXT_MENU_MOVE_THRESHOLD_PX } from "@/app/appConfig/default";
import { useViewportStore } from "@/features/core/viewport";
import type { EditorGesture } from "@/shared/types/ui";

export interface SelectionHitTarget {
  id: string;
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export function getCanvasWorldPoint(groupId: string, clientX: number, clientY: number) {
  const canvasEl = document.querySelector(`[data-editor-group-id="${groupId}"]`);
  if (!canvasEl) {
    return { x: clientX, y: clientY };
  }

  const rect = canvasEl.getBoundingClientRect();
  const viewport = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
  return {
    x: (clientX - rect.left - viewport.x) / viewport.scale,
    y: (clientY - rect.top - viewport.y) / viewport.scale,
  };
}

export function createSelectionHitTargets(
  groupId: string,
  nodes: Array<{ id?: string }>,
): SelectionHitTarget[] {
  const targets: SelectionHitTarget[] = [];
  const canvasEl = document.querySelector(`[data-editor-group-id="${groupId}"]`);

  for (const node of nodes) {
    const nodeId = node.id;
    if (!nodeId) continue;

    const element = canvasEl
      ? canvasEl.querySelector(`[data-node-id="${nodeId}"]`)
      : document.getElementById(nodeId);
    if (!element) continue;

    const bounds = element.getBoundingClientRect();
    targets.push({
      id: nodeId,
      left: bounds.left,
      right: bounds.right,
      top: bounds.top,
      bottom: bounds.bottom,
    });
  }

  return targets;
}

export function selectNodeIdsFromScreenTargets(
  targets: SelectionHitTarget[],
  rect: { x1: number; y1: number; x2: number; y2: number },
): string[] {
  const selectedIds: string[] = [];

  for (const target of targets) {
    if (!(target.left > rect.x2 || target.right < rect.x1 || target.top > rect.y2 || target.bottom < rect.y1)) {
      selectedIds.push(target.id);
    }
  }

  return selectedIds;
}

export function selectNodeIdsInScreenRect(
  groupId: string,
  nodes: Array<{ id?: string }>,
  rect: { x1: number; y1: number; x2: number; y2: number },
): string[] {
  return selectNodeIdsFromScreenTargets(createSelectionHitTargets(groupId, nodes), rect);
}

export function hasSelectionChanged(previous: string[], next: string[]): boolean {
  if (previous.length !== next.length) return true;
  const previousSet = new Set(previous);
  return next.some((id) => !previousSet.has(id));
}

export function getGestureScreenMovement(gesture: EditorGesture, scale = 1): boolean {
  if (!gesture) return false;

  if (gesture.type === "pan") {
    const dx = gesture.lastX - gesture.startX;
    const dy = gesture.lastY - gesture.startY;
    return Math.sqrt(dx * dx + dy * dy) > CONTEXT_MENU_MOVE_THRESHOLD_PX;
  }

  if (gesture.type === "select") {
    const dx = Math.abs(gesture.currentX - gesture.startX);
    const dy = Math.abs(gesture.currentY - gesture.startY);
    return dx > CONTEXT_MENU_MOVE_THRESHOLD_PX || dy > CONTEXT_MENU_MOVE_THRESHOLD_PX;
  }

  if (gesture.type === "drag" && gesture.dragDelta) {
    const screenDx = Math.abs(gesture.dragDelta.x * scale);
    const screenDy = Math.abs(gesture.dragDelta.y * scale);
    return screenDx > CONTEXT_MENU_MOVE_THRESHOLD_PX || screenDy > CONTEXT_MENU_MOVE_THRESHOLD_PX;
  }

  return false;
}
