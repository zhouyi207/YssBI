import { useVariableStore } from '@/features/core/dataStore';
import { getViewport } from '@/features/core/viewport';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { logger } from '@/utils/appLogger';

export type VariableNodeType = 'Variables:Get Variable' | 'Variables:Set Variable';

export interface VariableDropMenu {
  x: number;
  y: number;
  worldX: number;
  worldY: number;
  variableId: string;
  variableName: string;
}

export type CreateNodeFn = (
  nodeType: string,
  position: { x: number; y: number },
  params?: Record<string, unknown>,
) => Promise<{ nodeId: string; pinIds: string[] } | undefined>;

export function clientToWorldInCanvas(
  canvasEl: HTMLElement,
  graphId: string | null,
  clientX: number,
  clientY: number,
): { x: number; y: number } {
  const rect = canvasEl.getBoundingClientRect();
  const viewport = graphId ? getViewport(graphId) : DEFAULT_VIEWPORT;
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

export function isVariableAvailable(
  variableId: string,
  variables: Record<string, unknown>,
): boolean {
  if (variableId in variables) return true;
  return variableId in useVariableStore.getState().variables;
}

export function resolveVariableSpawnType(
  event: Pick<MouseEvent | PointerEvent, 'altKey' | 'ctrlKey'>,
  clientX: number,
  clientY: number,
): VariableNodeType | 'menu' {
  if (event.altKey) return 'Variables:Set Variable';
  if (event.ctrlKey) return 'Variables:Get Variable';

  const elements = document.elementsFromPoint(clientX, clientY);
  const pinEl = elements.find((el) => el.closest('[data-pin-id]'))?.closest('[data-pin-id]');
  if (pinEl?.getAttribute('data-pin-id')) {
    return 'Variables:Get Variable';
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

export async function spawnVariableNode(
  nodeType: VariableNodeType,
  worldPosition: { x: number; y: number },
  variableId: string,
  createNode: CreateNodeFn,
): Promise<void> {
  await createNode(nodeType, worldPosition, { variableId });
}

export async function spawnVariableFromMenu(
  menu: VariableDropMenu,
  nodeType: VariableNodeType,
  variables: Record<string, unknown>,
  createNode: CreateNodeFn,
): Promise<void> {
  if (!isVariableAvailable(menu.variableId, variables)) {
    logger.graph.warn('Variable no longer exists', 'CanvasVariableDrop');
    return;
  }
  await spawnVariableNode(
    nodeType,
    { x: menu.worldX, y: menu.worldY },
    menu.variableId,
    createNode,
  );
}
