import { workbenchDockviewControl } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";

import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

/** Split the active canonical editor right or down; native Dockview DnD owns moves/order. */
export async function splitEditorPanel(
  groupId: string,
  edge: "right" | "bottom",
): Promise<string | null> {
  const panel =
    workbenchDockviewRead.getActiveEditorPanelInGroup(groupId) ??
    workbenchDockviewRead.listEditorPanelsInGroup(groupId)[0];
  if (!panel) return null;

  const split = await workbenchDockviewControl.split({
    panelInstanceId: panel.panelInstanceId,
    referenceGroupId: groupId,
    direction: edge,
  });
  if (!split) return null;

  const moved = workbenchDockviewRead.getPanel(panel.panelInstanceId);
  if (moved?.metadata.role !== "editor") return null;
  await activateEditorPanelAndSyncSession({ ...moved, metadata: moved.metadata });
  return moved.groupId;
}
