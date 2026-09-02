import { bootstrapEditorGraphSession } from "@/features/application/editor/bootstrapEditorGraphSession";
import { pruneEditorPanelsForMissingResources } from "@/features/application/editor/pruneEditorPanels";
import { synchronizeVisibleGraphPanels } from "@/features/application/editor/synchronizeVisibleGraphPanel";
import { workbenchLayoutController } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";

/** Synchronizes the mounted presentation after one authoritative Project snapshot. */
export function synchronizeProjectPresentation(): void {
  workbenchLayoutController.markProjectResourcesReady(async (context) => {
    if (!context.isCurrent()) return;
    await pruneEditorPanelsForMissingResources();
    if (!context.isCurrent()) return;
    await synchronizeVisibleGraphPanels();
    if (!context.isCurrent()) return;
    const active = workbenchDockviewRead.getActiveEditorPanel();
    if (active?.metadata.role === "editor") {
      await bootstrapEditorGraphSession(active.groupId);
    }
  });
}
