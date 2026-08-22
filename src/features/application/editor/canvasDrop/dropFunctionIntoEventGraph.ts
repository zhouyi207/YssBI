import type { GraphResourceDragData } from '@/features/core/dnd';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { EDITOR_MUTATION_CAPABILITIES } from '../editorMutationAvailability';

export function canCreateFunctionNodeInGraph(
  groupId: string,
  resource: Pick<GraphResourceDragData, 'type' | 'id'>,
): boolean {
  if (!EDITOR_MUTATION_CAPABILITIES.resourceBoundDescriptors) return false;
  if (resource.type !== 'function') return false;

  const activeTab = getActiveLayoutTab(groupId)?.tab;
  if (!activeTab || (activeTab.type !== 'event' && activeTab.type !== 'function')) return false;
  return activeTab.id !== resource.id;
}
