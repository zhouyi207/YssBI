import { editorDockviewPort, type DockviewPanelInfo } from '@/features/core/dockview';
import type { LayoutTab } from '@/shared/types/ui';

export function layoutTabFromDockviewPanel(panel: DockviewPanelInfo | undefined): LayoutTab | null {
  const value = panel?.tab?.data?.layoutTab;
  return value && typeof value === 'object' ? value as unknown as LayoutTab : null;
}

export function listDockviewGroupPanels(groupId: string): DockviewPanelInfo[] {
  const group = editorDockviewPort.listGroups().find((candidate) => candidate.groupId === groupId);
  if (!group) return [];
  const panelsById = new Map(
    editorDockviewPort.listPanels().map((panel) => [panel.panelInstanceId, panel]),
  );
  return group.panelInstanceIds
    .map((panelId) => panelsById.get(panelId))
    .filter((panel): panel is DockviewPanelInfo => panel !== undefined);
}

export function listDockviewGroupTabs(groupId: string): LayoutTab[] {
  return listDockviewGroupPanels(groupId)
    .map(layoutTabFromDockviewPanel)
    .filter((tab): tab is LayoutTab => tab !== null);
}

export function findDockviewPanel(resourceId: string, groupId?: string): DockviewPanelInfo | undefined {
  return editorDockviewPort
    .findPanelsByResource(resourceId)
    .find((panel) => !groupId || panel.groupId === groupId);
}
