import { bootstrapEditorGraphSession } from "@/features/application/editor/bootstrapEditorGraphSession";
import { reconcileOpenEditorPanelsWithResources } from "@/features/application/editor/reconcileOpenEditorPanels";
import { synchronizeVisibleGraphPanels } from "@/features/application/editor/synchronizeVisibleGraphPanel";
import { workbenchLayoutController } from "@/features/application/layout/workbenchLayoutController";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";

/** Reconciles the mounted presentation after one authoritative Project snapshot. */
export function reconcileProjectPresentation(): void {
  workbenchLayoutController.markProjectResourcesReady(async (context) => {
    if (!context.isCurrent()) return;
    await reconcileOpenEditorPanelsWithResources();
    if (!context.isCurrent()) return;
    await synchronizeVisibleGraphPanels();
    if (!context.isCurrent()) return;
    const active = workbenchDockviewRead.getActiveEditorPanel();
    if (active?.metadata.role === "editor") {
      await bootstrapEditorGraphSession(active.groupId);
    }
  });
}
