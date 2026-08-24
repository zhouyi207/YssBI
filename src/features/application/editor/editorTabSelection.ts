import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';

/** Activate a canonical editor panel by resource identity within an editor group. */
export function applyEditorTabSelection(groupId: string, resourceId: string | null): void {
  if (!resourceId) return;
  const panel = workbenchDockviewPort
    .findEditorPanelsByResource(resourceId)
    .find((candidate) => candidate.groupId === groupId);
  if (panel) void workbenchDockviewPort.activate(panel.panelInstanceId);
}
