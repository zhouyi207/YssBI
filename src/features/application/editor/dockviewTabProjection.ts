import { layoutTabFromEditorMetadata } from '@/features/core/dockview/workbenchPanelModel';
import {
  workbenchDockviewRead,
  type WorkbenchPanelInfo,
} from '@/features/core/dockview/workbenchRead';
import type { LayoutTab } from '@/shared/types/ui';

export function layoutTabFromDockviewPanel(
  panel: WorkbenchPanelInfo | undefined,
): LayoutTab | null {
  return panel?.metadata.role === 'editor'
    ? layoutTabFromEditorMetadata(panel.metadata)
    : null;
}

export function listDockviewGroupPanels(groupId: string): WorkbenchPanelInfo[] {
  return workbenchDockviewRead
    .listGroupPanels(groupId)
    .filter((panel) => panel.metadata.role === 'editor');
}

export function listDockviewGroupTabs(groupId: string): LayoutTab[] {
  return listDockviewGroupPanels(groupId)
    .map(layoutTabFromDockviewPanel)
    .filter((tab): tab is LayoutTab => tab !== null);
}

export function findDockviewPanel(
  resourceId: string,
  groupId?: string,
): WorkbenchPanelInfo | undefined {
  return workbenchDockviewRead
    .findEditorPanelsByResource(resourceId)
    .find((panel) => groupId === undefined || panel.groupId === groupId);
}
