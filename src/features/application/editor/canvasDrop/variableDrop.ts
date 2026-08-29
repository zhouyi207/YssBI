import { getViewport, editorViewportScope } from '@/features/core/viewport';
import { DEFAULT_VIEWPORT } from '@/shared/config-default';

export interface VariableDropMenu {
  x: number;
  y: number;
  worldX: number;
  worldY: number;
  variableId: string;
  variableName: string;
}

export function clientToWorldInCanvas(
  canvasEl: HTMLElement,
  groupId: string,
  graphPath: string | null,
  clientX: number,
  clientY: number,
): { x: number; y: number } {
  const rect = canvasEl.getBoundingClientRect();
  const viewport = graphPath ? getViewport(editorViewportScope(groupId, graphPath)) : DEFAULT_VIEWPORT;
  return {
    x: (clientX - rect.left - viewport.x) / viewport.scale,
    y: (clientY - rect.top - viewport.y) / viewport.scale,
  };
}

export function isPointInsideCanvas(
  canvasEl: HTMLElement,
  clientX: number,
  clientY: number,
): boolean {
  const rect = canvasEl.getBoundingClientRect();
  return (
    clientX >= rect.left
    && clientX <= rect.right
    && clientY >= rect.top
    && clientY <= rect.bottom
  );
}
