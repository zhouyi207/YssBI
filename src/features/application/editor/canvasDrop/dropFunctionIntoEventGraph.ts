import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { GraphResourceDragData } from '@/features/core/dnd';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { clientToWorldInCanvas, isPointInsideCanvas } from './variableDrop';
import type { CreateNodeFn } from './createNodeFn';
import { EDITOR_MUTATION_CAPABILITIES } from '../editorMutationAvailability';

export function canDropFunctionIntoEventGraph(
  groupId: string,
  resource: Pick<GraphResourceDragData, 'type' | 'id'>,
  shiftKey: boolean,
): boolean {
  if (!EDITOR_MUTATION_CAPABILITIES.resourceBoundDescriptors) return false;
  if (!shiftKey || resource.type !== 'function') return false;

  const activeTab = getActiveLayoutTab(groupId)?.tab;
  if (!activeTab || (activeTab.type !== 'event' && activeTab.type !== 'function')) return false;
  return activeTab.id !== resource.id;
}

export async function dropFunctionCallIntoEventGraph(
  canvasElement: HTMLElement,
  groupId: string,
  graphPath: string,
  descriptor: NodeCreationDescriptor,
  clientX: number,
  clientY: number,
  createNode: CreateNodeFn,
): Promise<boolean> {
  if (!isPointInsideCanvas(canvasElement, clientX, clientY)) return false;
  if (descriptor.kind !== 'resourceBound' || descriptor.createArgs.kind !== 'function') return false;

  const worldPosition = clientToWorldInCanvas(canvasElement, groupId, graphPath, clientX, clientY);
  return createNode(descriptor, worldPosition);
}
