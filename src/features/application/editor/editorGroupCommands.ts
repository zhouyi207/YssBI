import { layoutTabFromEditorMetadata } from '@/features/core/dockview/workbenchPanelModel';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';

import { switchEditorTab } from './switchEditorTab';

function activeEditorPanelInGroup(groupId: string) {
  const panels = workbenchDockviewPort
    .listGroupPanels(groupId)
    .filter((panel) => panel.metadata.role === 'editor');
  const activePanelInstanceId = workbenchDockviewPort
    .listGroups()
    .find((group) => group.groupId === groupId)
    ?.activePanelInstanceId;
  return panels.find((panel) => panel.panelInstanceId === activePanelInstanceId)
    ?? panels[0];
}

/** Split the active canonical editor right or down; native Dockview DnD owns moves/order. */
export async function splitEditorAtEdge(
  groupId: string,
  edge: 'right' | 'bottom',
): Promise<string | null> {
  const panel = activeEditorPanelInGroup(groupId);
  if (!panel || panel.metadata.role !== 'editor') return null;

  const split = await workbenchDockviewPort.split({
    panelInstanceId: panel.panelInstanceId,
    referenceGroupId: groupId,
    direction: edge,
  });
  if (!split) return null;

  const moved = workbenchDockviewPort.getPanel(panel.panelInstanceId);
  if (moved?.metadata.role !== 'editor') return null;
  await switchEditorTab(
    moved.groupId,
    layoutTabFromEditorMetadata(moved.metadata),
  );
  return moved.groupId;
}
