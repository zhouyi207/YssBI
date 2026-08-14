import type { GraphResourceDragData } from '@/features/core/dnd';
import { editorDockviewPort } from '@/features/core/dockview';
import type { EditorSplitDirection } from '@/features/core/layout/editorSplitHitTest';
import { getActiveLayoutTab, resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { openGraphInEditor } from './openGraphInEditor';
import { switchEditorTab } from './switchEditorTab';

/** Handle sidebar graph-resource drops without participating in Dockview's native tab DnD. */
export async function handleGraphResourceDrop(
  resource: GraphResourceDragData,
  targetGroupId: string,
  options?: {
    edge?: EditorSplitDirection;
    insertIndex?: number;
  },
): Promise<void> {
  const resolvedGroupId = resolveEditorTargetGroupId(targetGroupId);
  await openGraphInEditor(resource.id, resource.name, resource.type, resolvedGroupId, {
    pinned: true,
    insertIndex: options?.insertIndex,
  });

  if (!options?.edge) return;
  const panel = editorDockviewPort
    .findPanelsByResource(resource.id)
    .find((candidate) => candidate.groupId === resolvedGroupId);
  if (!panel) return;
  const split = await editorDockviewPort.split({
    panelInstanceId: panel.panelInstanceId,
    referenceGroupId: resolvedGroupId,
    direction: options.edge,
  });
  if (!split) return;
  const createdGroupId = editorDockviewPort
    .listPanels()
    .find((candidate) => candidate.panelInstanceId === panel.panelInstanceId)?.groupId;
  const activeTab = createdGroupId ? getActiveLayoutTab(createdGroupId)?.tab : null;
  if (createdGroupId && activeTab) await switchEditorTab(createdGroupId, activeTab);
}
