import { layoutTabFromEditorMetadata } from "@/features/core/dockview/workbenchPanelModel";
import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";

import { switchEditorTab } from "./switchEditorTab";

function activeEditorPanelInGroup(groupId: string) {
  const panels = workbenchDockviewRead
    .listGroupPanels(groupId)
    .filter((panel) => panel.metadata.role === "editor");
  const activePanelInstanceId = workbenchDockviewRead
    .listGroups()
    .find((group) => group.groupId === groupId)?.activePanelInstanceId;
  return panels.find((panel) => panel.panelInstanceId === activePanelInstanceId) ?? panels[0];
}

/** Split the active canonical editor right or down; native Dockview DnD owns moves/order. */
export async function splitEditorAtEdge(
  groupId: string,
  edge: "right" | "bottom",
): Promise<string | null> {
  const panel = activeEditorPanelInGroup(groupId);
  if (!panel || panel.metadata.role !== "editor") return null;

  const split = await workbenchDockviewControl.split({
    panelInstanceId: panel.panelInstanceId,
    referenceGroupId: groupId,
    direction: edge,
  });
  if (!split) return null;

  const moved = workbenchDockviewRead.getPanel(panel.panelInstanceId);
  if (moved?.metadata.role !== "editor") return null;
  await switchEditorTab(moved.groupId, layoutTabFromEditorMetadata(moved.metadata));
  return moved.groupId;
}
