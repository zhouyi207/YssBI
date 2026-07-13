import type { GraphResourceDragData } from '@/features/core/dnd';
import { isCanvasDrop, isTabbarDrop } from '@/features/core/dnd';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { openGraphInEditor } from './openGraphInEditor';
import { resolveTabBarDropIndex } from './tabBarReorderStore';

/**
 * Drop handler for sidebar Event/Function graph resources.
 * Canvas → open in target editor group; TabBar → open pinned at insert index.
 */
export async function handleGraphResourceDrop(
  resource: GraphResourceDragData,
  overData: unknown,
): Promise<void> {
  if (isTabbarDrop(overData)) {
    const targetGroupId = overData.targetNodeId;
    const insertIndex = resolveTabBarDropIndex(targetGroupId, overData.targetTabIndex);
    await openGraphInEditor(resource.id, resource.name, resource.type, targetGroupId, {
      pinned: true,
      insertIndex,
    });
    return;
  }

  if (isCanvasDrop(overData)) {
    const layoutState = useLayoutStore.getState();
    const targetGroupId =
      overData.groupId
      || resolveEditorTargetGroupId(undefined, layoutState.nodes, layoutState);
    await openGraphInEditor(resource.id, resource.name, resource.type, targetGroupId, {
      pinned: true,
    });
  }
}
