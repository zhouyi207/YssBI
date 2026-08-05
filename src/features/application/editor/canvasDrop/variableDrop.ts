import { getViewport, editorViewportScope } from '@/features/core/viewport';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import {
  BUILTIN_NODE_TYPE_IDS,
  type VariableNodeTypeId,
} from '@/features/domain/nodeCatalog';

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

export function resolveVariableSpawnType(
  event: Pick<MouseEvent | PointerEvent, 'altKey' | 'ctrlKey'>,
  clientX: number,
  clientY: number,
): VariableNodeTypeId | 'menu' {
  if (event.altKey) return BUILTIN_NODE_TYPE_IDS.setVariable;
  if (event.ctrlKey) return BUILTIN_NODE_TYPE_IDS.getVariable;

  const elements = document.elementsFromPoint(clientX, clientY);
  const pinEl = elements.find((el) => el.closest('[data-pin-id]'))?.closest('[data-pin-id]');
  if (pinEl?.getAttribute('data-pin-id')) {
    return BUILTIN_NODE_TYPE_IDS.getVariable;
  }

  return 'menu';
}

export function buildVariableDropMenu(
  clientX: number,
  clientY: number,
  world: { x: number; y: number },
  variableId: string,
  variableName: string,
): VariableDropMenu {
  return {
    x: clientX,
    y: clientY,
    worldX: world.x,
    worldY: world.y,
    variableId,
    variableName,
  };
}