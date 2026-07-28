import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
import type { GraphResourceDragData } from '@/features/core/dnd';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { clientToWorldInCanvas, isPointInsideCanvas } from './variableDrop';
import { isFunctionAvailable } from './editorResources';
import type { CreateNodeFn } from './createNodeFn';
import type { EditorFunctions } from '@/features/core/editor';
import {
  EDITOR_MUTATION_CAPABILITIES,
  notifyNodeCreationUnavailable,
} from '../editorMutationAvailability';

export function canDropFunctionIntoEventGraph(
  groupId: string,
  resource: Pick<GraphResourceDragData, 'type' | 'id'>,
  shiftKey: boolean,
): boolean {
  if (!EDITOR_MUTATION_CAPABILITIES.createNodes) return false;
  if (!shiftKey || resource.type !== 'function') return false;

  const activeTab = getActiveLayoutTab(groupId)?.tab;
  if (!activeTab || (activeTab.type !== 'event' && activeTab.type !== 'function')) {
    return false;
  }

  return activeTab.id !== resource.id;
}

export async function dropFunctionCallIntoEventGraph(
  canvasElement: HTMLElement,
  groupId: string,
  graphPath: string,
  functionPath: string,
  clientX: number,
  clientY: number,
  functions: EditorFunctions,
  createNode: CreateNodeFn,
): Promise<boolean> {
  if (!EDITOR_MUTATION_CAPABILITIES.createNodes) {
    notifyNodeCreationUnavailable();
    return false;
  }
  if (!isPointInsideCanvas(canvasElement, clientX, clientY)) return false;
  if (!isFunctionAvailable(functionPath, functions)) return false;

  const worldPosition = clientToWorldInCanvas(canvasElement, groupId, graphPath, clientX, clientY);
  await createNode(CALL_FUNCTION_NODE_TYPE, worldPosition, { subGraphPath: functionPath });
  return true;
}
