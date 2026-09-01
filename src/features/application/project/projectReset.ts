import type { ProjectLifecycleStateSnapshot } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { isProjectLifecycleStateCurrent } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useViewportStore } from "@/features/core/viewport";
import { useGraphInteractionStore } from "@/features/core/graphInteraction";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { useDocumentStateStore, useResourceStore } from "@/features/core/resource";
import { useColumnStatsStore } from "@/features/core/dataStore/columnStatsStore";
import { useColumnDistributionStore } from "@/features/core/dataStore/columnDistributionStore";
import { useDatasetOverviewStore } from "@/features/core/dataStore/datasetOverviewStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { resetFunctionSignatureCoordinator } from "@/features/application/editorMutation/functionSignatureCoordinator";
import { resetHistoryCoordinator } from "@/features/application/editorMutation/historyCoordinator";

export interface ProjectPresentationResetActions {
  removeProjectScopedWorkbenchPanels(
    previousProjectInstanceId: string,
    owner: ProjectLifecycleStateSnapshot,
  ): Promise<void>;
}

export function resetProjectScopedRightSidebarState(): void {
  const editor = useEditorStore.getState();
  editor.clearDetailFocus();
  editor.setVariablesGraphScope(null);
}

function runOwnedReset(owner: ProjectLifecycleStateSnapshot, reset: () => void): boolean {
  if (!isProjectLifecycleStateCurrent(owner)) return false;
  reset();
  return true;
}

/** Clears all client projections before the next authoritative Project snapshot is applied. */
export async function resetClientProjectState(
  previousProjectInstanceId: string | null,
  owner: ProjectLifecycleStateSnapshot,
  actions: ProjectPresentationResetActions,
): Promise<void> {
  if (!isProjectLifecycleStateCurrent(owner)) return;
  if (previousProjectInstanceId) {
    await actions.removeProjectScopedWorkbenchPanels(previousProjectInstanceId, owner);
  }
  if (!runOwnedReset(owner, () => useViewportStore.getState().clear())) return;
  if (!runOwnedReset(owner, resetFunctionSignatureCoordinator)) return;
  if (!runOwnedReset(owner, resetHistoryCoordinator)) return;
  if (!runOwnedReset(owner, () => useGraphInteractionStore.setState({ positionOverrides: {} })))
    return;
  if (!runOwnedReset(owner, () => useColumnStatsStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useColumnDistributionStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useDatasetOverviewStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useChartDocumentStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useResourceStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useDocumentStateStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useGraphMetaStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useGraphSessionStore.getState().reset())) return;
  runOwnedReset(owner, resetProjectScopedRightSidebarState);
}
