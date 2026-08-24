import type { GraphResourceDragData } from '@/features/core/dnd';
import { layoutTabFromEditorMetadata } from '@/features/core/dockview/workbenchPanelModel';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';

import { openGraphInEditor } from './openGraphInEditor';
import { switchEditorTab } from './switchEditorTab';

/** Handle sidebar graph-resource drops without participating in Dockview's native tab DnD. */
export async function handleGraphResourceDrop(
  resource: GraphResourceDragData,
  targetGroupId: string,
  options?: {
    edge?: 'right' | 'bottom';
    insertIndex?: number;
  },
): Promise<void> {
  const opened = await openGraphInEditor(
    resource.id,
    resource.name,
    resource.type,
    targetGroupId,
    { pinned: true, insertIndex: options?.insertIndex },
  );
  if (!opened || !options?.edge) return;

  const split = await workbenchDockviewPort.split({
    panelInstanceId: opened.panelInstanceId,
    referenceGroupId: opened.groupId,
    direction: options.edge,
  });
  if (!split) return;

  const moved = workbenchDockviewPort.getPanel(opened.panelInstanceId);
  if (moved?.metadata.role !== 'editor') return;
  await switchEditorTab(
    moved.groupId,
    layoutTabFromEditorMetadata(moved.metadata),
  );
}
