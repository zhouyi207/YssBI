import type { GraphResourceDragData } from "@/features/core/dnd";
import { workbenchLayoutControl } from "@/modules/workbench/public";
import { workbenchLayoutRead } from "@/modules/workbench/public";

import { openGraphInEditor } from "./openGraphInEditor";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

/** Handle sidebar graph-resource drops without participating in FlexLayout's native tab DnD. */
export async function handleGraphResourceDrop(
  resource: GraphResourceDragData,
  targetGroupId: string,
  options?: {
    edge?: "right" | "bottom";
    insertIndex?: number;
  },
): Promise<void> {
  const opened = await openGraphInEditor(resource.id, resource.name, resource.type, targetGroupId, {
    insertIndex: options?.insertIndex,
  });
  if (!opened || !options?.edge) return;

  const split = await workbenchLayoutControl.split({
    panelInstanceId: opened.panelInstanceId,
    referenceGroupId: opened.groupId,
    direction: options.edge,
  });
  if (!split) return;

  const moved = workbenchLayoutRead.getPanel(opened.panelInstanceId);
  if (moved?.metadata.role !== "editor") return;
  await activateEditorPanelAndSyncSession({ ...moved, metadata: moved.metadata });
}
