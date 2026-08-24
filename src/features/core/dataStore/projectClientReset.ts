/**
 * 项目切换时清空前端的 per-project 缓存。
 * 所有被 reset 的 store 须在本文件显式 import（dataStore.audit 校验）。
 */
import { useViewportStore } from '@/features/core/viewport';
import { projectIOApplicationPort } from './projectIOApplicationPort';
import { useGraphInteractionStore } from '@/features/core/graphInteraction';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useDocumentStateStore, useResourceStore } from '@/features/core/resource';
import { useEditStateStore } from './editStateStore';
import { useColumnStatsStore } from './columnStatsStore';
import { useColumnDistributionStore } from './columnDistributionStore';
import { useDatasetOverviewStore } from './datasetOverviewStore';
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import {
  isProjectLifecycleStateCurrent,
  type ProjectLifecycleStateSnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

export function resetProjectScopedRightSidebarState(): void {
  const editor = useEditorStore.getState();
  editor.clearDetailFocus();
  editor.setVariablesGraphScope(null);
}

function runOwnedReset(
  owner: ProjectLifecycleStateSnapshot,
  reset: () => void,
): boolean {
  if (!isProjectLifecycleStateCurrent(owner)) return false;
  reset();
  return true;
}

/** 清空 viewport / history / 数据视图缓存等；变量与 graph 正文由调用方立即覆写。 */
export async function resetClientProjectState(
  previousProjectInstanceId: string | null,
  owner: ProjectLifecycleStateSnapshot,
): Promise<void> {
  if (!isProjectLifecycleStateCurrent(owner)) return;
  const applicationPort = projectIOApplicationPort();
  if (previousProjectInstanceId) {
    await applicationPort.removeProjectScopedWorkbenchPanels(
      previousProjectInstanceId,
      owner,
    );
  }
  if (!runOwnedReset(owner, () => useViewportStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => applicationPort.resetFunctionSignatures())) return;
  if (!runOwnedReset(owner, () => applicationPort.resetHistory())) return;
  if (!runOwnedReset(owner, () => {
    useGraphInteractionStore.setState({ positionOverrides: {} });
  })) return;
  if (!runOwnedReset(owner, () => useEditStateStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useColumnStatsStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useColumnDistributionStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useDatasetOverviewStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useWorksheetStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useResourceStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useDocumentStateStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useGraphMetaStore.getState().clear())) return;
  if (!runOwnedReset(owner, () => useGraphSessionStore.getState().reset())) return;
  runOwnedReset(owner, resetProjectScopedRightSidebarState);
}
