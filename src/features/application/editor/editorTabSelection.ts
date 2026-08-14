import { editorDockviewPort } from '@/features/core/dockview';

/** Activate a Dockview panel by resource identity within an editor group. */
export function applyEditorTabSelection(groupId: string, resourceId: string | null): void {
  if (!resourceId) return;
  const panel = editorDockviewPort
    .findPanelsByResource(resourceId)
    .find((candidate) => candidate.groupId === groupId);
  if (panel) void editorDockviewPort.activate(panel.panelInstanceId);
}
