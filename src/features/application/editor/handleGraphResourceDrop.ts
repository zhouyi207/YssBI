import type { GraphResourceDragData } from "@/features/core/dnd";
import { workbenchDockviewControl } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";

import { openGraphInEditor } from "./openGraphInEditor";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

/** Handle sidebar graph-resource drops without participating in Dockview's native tab DnD. */
export async function handleGraphResourceDrop(
  resource: GraphResourceDragData,
  targetGroupId: string,
  options?: {
    edge?: "right" | "bottom";
    insertIndex?: number;
  },
): Promise<void> {
  const opened = await openGraphInEditor(resource.id, resource.name, resource.type, targetGroupId, {
    pinned: true,
    insertIndex: options?.insertIndex,
  });
  if (!opened || !options?.edge) return;

  const split = await workbenchDockviewControl.split({
    panelInstanceId: opened.panelInstanceId,
    referenceGroupId: opened.groupId,
    direction: options.edge,
  });
  if (!split) return;

  const moved = workbenchDockviewRead.getPanel(opened.panelInstanceId);
  if (moved?.metadata.role !== "editor") return;
  await activateEditorPanelAndSyncSession({ ...moved, metadata: moved.metadata });
}
