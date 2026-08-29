import { workbenchDockviewControl } from '@/features/core/dockview/workbenchControl';
import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';

/** Activate a canonical editor panel by resource identity within an editor group. */
export function applyEditorTabSelection(groupId: string, resourceId: string | null): void {
  if (!resourceId) return;
  const panel = workbenchDockviewRead
    .findEditorPanelsByResource(resourceId)
    .find((candidate) => candidate.groupId === groupId);
  if (panel) void workbenchDockviewControl.activate(panel.panelInstanceId);
}
