import type { GraphResourceDragData } from '@/features/core/dnd';
import type { EditorSplitDirection } from '@/features/core/layout/editorSplitHitTest';
import { getActiveLayoutTab, resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { EditorGroupsService } from '@/features/core/layout/editorGroupsService';
import { openGraphInEditor } from './openGraphInEditor';
import { switchEditorTab } from './switchEditorTab';
/**
 * Drop handler for sidebar Event/Function graph resources.
 * TabBar → open pinned at insert index; editor body → merge or VS Code-style split.
 */
export async function handleGraphResourceDrop(
  resource: GraphResourceDragData,
  targetGroupId: string,
  options?: {
    edge?: EditorSplitDirection;
    insertIndex?: number;
  },
): Promise<void> {
  if (options?.insertIndex != null) {
    await openGraphInEditor(resource.id, resource.name, resource.type, targetGroupId, {
      pinned: true,
      insertIndex: options.insertIndex,
    });
    return;
  }

  if (options?.edge) {
    const created = EditorGroupsService.splitGroupAtEdge(targetGroupId, options.edge, {
      component: 'GraphEditor',
      tabs: [],
    });
    if (!created) return;
    await openGraphInEditor(resource.id, resource.name, resource.type, created, {
      pinned: true,
    });
    const activeTab = getActiveLayoutTab(created)?.tab;
    if (activeTab) await switchEditorTab(created, activeTab);
    return;
  }

  const layoutState = useLayoutStore.getState();
  const resolvedGroupId = targetGroupId
    || resolveEditorTargetGroupId(undefined, layoutState.nodes, layoutState);
  await openGraphInEditor(resource.id, resource.name, resource.type, resolvedGroupId, {
    pinned: true,
  });
}
