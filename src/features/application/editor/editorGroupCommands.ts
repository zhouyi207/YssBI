import { workbenchLayoutControl } from "@/modules/workbench/public";
import { workbenchLayoutRead } from "@/modules/workbench/public";

import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

/** Split the active canonical editor right or down; native FlexLayout DnD owns moves/order. */
export async function splitEditorPanel(
  groupId: string,
  edge: "right" | "bottom",
): Promise<string | null> {
  const panel =
    workbenchLayoutRead.getActiveEditorPanelInGroup(groupId) ??
    workbenchLayoutRead.listEditorPanelsInGroup(groupId)[0];
  if (!panel) return null;

  const split = await workbenchLayoutControl.split({
    panelInstanceId: panel.panelInstanceId,
    referenceGroupId: groupId,
    direction: edge,
  });
  if (!split) return null;

  const moved = workbenchLayoutRead.getPanel(panel.panelInstanceId);
  if (moved?.metadata.role !== "editor") return null;
  await activateEditorPanelAndSyncSession({ ...moved, metadata: moved.metadata });
  return moved.groupId;
}
