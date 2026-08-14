/**
 * 项目切换时清空前端的 per-project 缓存。
 * 所有被 reset 的 store 须在本文件显式 import（dataStore.audit 校验）。
 */
import { collapseEditorGroupsForProjectSwitch } from '@/features/core/layout/workbenchLayoutService';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
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

/** 清空 tab / viewport / history / 数据视图缓存等；变量与 graph 正文由调用方立即覆写。 */
export function resetClientProjectState(): void {
  projectIOApplicationPort().cancelPublication();
  useLayoutStore.getState().closeAllGraphTabs();
  collapseEditorGroupsForProjectSwitch();
  useViewportStore.getState().clear();
  projectIOApplicationPort().resetFunctionSignatures();
  projectIOApplicationPort().resetHistory();
  useGraphInteractionStore.setState({ positionOverrides: {} });
  useEditStateStore.getState().clear();
  useColumnStatsStore.getState().clear();
  useColumnDistributionStore.getState().clear();
  useDatasetOverviewStore.getState().clear();
  useWorksheetStore.getState().clear();
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useGraphMetaStore.getState().clear();
  useGraphSessionStore.getState().reset();
  useEditorStore.getState().clearInspectorResult();
}
