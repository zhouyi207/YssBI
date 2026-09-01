import { workbenchDockviewControl } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";

/** Activate a canonical editor panel by resource identity within an editor group. */
export function applyEditorPanelSelection(groupId: string, resourceId: string | null): void {
  if (!resourceId) return;
  const panel = workbenchDockviewRead
    .findEditorPanelsByResource(resourceId)
    .find((candidate) => candidate.groupId === groupId);
  if (panel) void workbenchDockviewControl.activate(panel.panelInstanceId);
}
