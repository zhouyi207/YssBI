import { synchronizeActiveEditorPanel } from "@/features/application/editor/activateEditorPanelAndSyncSession";
import { pruneEditorPanelsForMissingResources } from "@/features/application/editor/pruneEditorPanels";
import { synchronizeVisibleGraphPanels } from "@/features/application/editor/synchronizeVisibleGraphPanel";
import { workbenchLayoutController } from "@/modules/workbench/public";
import { workbenchLayoutRead } from "@/modules/workbench/public";

/** Synchronizes the mounted presentation after one authoritative Project snapshot. */
export function synchronizeProjectPresentation(): void {
  workbenchLayoutController.markProjectResourcesReady(async (context) => {
    if (!context.isCurrent()) return;
    await pruneEditorPanelsForMissingResources();
    if (!context.isCurrent()) return;
    await synchronizeVisibleGraphPanels();
    if (!context.isCurrent()) return;
    const active = workbenchLayoutRead.getActiveEditorPanel();
    if (active?.metadata.role === "editor") {
      synchronizeActiveEditorPanel(active);
    }
  });
}
