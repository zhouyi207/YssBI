import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { editorViewportScope, ensureEditorViewport } from "@/features/core/viewport";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";

export interface VisibleGraphPanelScope {
  readonly groupId: string;
  readonly graphPath: string;
}

/** Prepare the visible preview scope before requesting its graph projection. */
export async function synchronizeVisibleGraphPanel(
  scope: VisibleGraphPanelScope,
): Promise<boolean> {
  ensureEditorViewport(editorViewportScope(scope.groupId, scope.graphPath));
  return useProjectIOStore.getState().loadGraph(scope.graphPath);
}

/** Reconcile visible graph panels after Dockview and project resources are ready. */
export async function synchronizeVisibleGraphPanels(): Promise<void> {
  const scopesByGraph = new Map<string, Set<string>>();

  for (const panel of workbenchDockviewRead.listPanels()) {
    if (panel.visible !== true || panel.metadata.role !== "editor") continue;
    if (panel.metadata.resourceKind === "worksheet") continue;

    const groups = scopesByGraph.get(panel.metadata.resourceRef) ?? new Set<string>();
    groups.add(panel.groupId);
    scopesByGraph.set(panel.metadata.resourceRef, groups);
  }

  for (const [graphPath, groupIds] of scopesByGraph) {
    for (const groupId of groupIds) {
      ensureEditorViewport(editorViewportScope(groupId, graphPath));
    }
  }

  await Promise.allSettled(
    [...scopesByGraph.keys()].map((graphPath) => useProjectIOStore.getState().loadGraph(graphPath)),
  );
}
