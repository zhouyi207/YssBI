import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectIOStore, getGraphById } from '@/features/core/dataStore';
import { selectFirstGraphResource, useResourceStore } from '@/features/core/resource';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { markResourceDirty } from '@/features/core/resource';
import { ProjectService } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';
import { uiStore } from '@/features/core/ui/UIStore';
import { useExecutionStore } from '@/features/core/execution';
import { createPersistedWindow } from '@/features/application/window';
import type { ExecutionEvent, RecordedEvent } from '@/shared/types/ui/execution';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';

/**
 * Project Operations Hook
 * Handles flush, load, and execute operations
 */
export function useProjectOperations(openGraph: (id: string, name: string, type: any, data?: any) => void | Promise<void>) {
  const { t } = useTranslation();
  const currentPath = useProjectIOStore((s) => s.currentPath);

  // 注意：新架构中不需要 syncActiveToCollection，后端事件会自动同步
  const syncActiveToCollection = useCallback(() => {
    // TODO: 如果需要，实现新的同步逻辑
    logger.app.debug('syncActiveToCollection called (no-op in new architecture)', 'ProjectOperations');
  }, []);

  const saveGraphAs = useCallback(async () => {
    if (!currentPath) {
      uiStore.showToast("项目尚未加载", "warning", 2000);
      return;
    }
    try {
      const dirtySaved = await saveAllDirtyGraphs();
      if (!dirtySaved) return;

      const record = await ProjectService.saveProjectAs();
      if (!record) return;

      await useProjectIOStore.getState().loadProject();
      uiStore.showToast(`项目已另存为：${record.name}`, "success", 3000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast(`另存为失败：${formatErrorMessage(e)}`, "error", 3000);
    }
  }, [currentPath]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) {
      uiStore.showToast("项目尚未加载", "warning", 2000);
      return;
    }
    syncActiveToCollection();
    try {
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId;
      const editorNode = editorGroupId ? layoutStore.nodes[editorGroupId] : null;
      const activeTabId = editorNode?.data?.activeTabId;
      if (!activeTabId) {
        uiStore.showToast("请先打开一个图或工作表", "warning", 2000);
        return;
      }

      const activeTab = editorNode?.data?.tabs?.find((tab) => tab.id === activeTabId);
      if (activeTab?.type === 'worksheet') {
        await useWorksheetStore.getState().saveDocument(activeTabId);
        uiStore.showToast(t('worksheet.saved'), 'success', 2000);
        return;
      }

      if (activeTab?.type !== 'event' && activeTab?.type !== 'function') {
        uiStore.showToast("请先打开一个图或工作表", "warning", 2000);
        return;
      }

      await GraphService.saveProjectGraph(activeTabId);
      markResourceDirty({ id: activeTabId, kind: activeTab.type }, false);
      uiStore.showToast("图已保存", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast(`保存失败：${formatErrorMessage(e)}`, "error", 2000);
    }
  }, [currentPath, syncActiveToCollection, t]);

  const importGraph = useCallback(async () => {
    try {
      const path = await ProjectService.pickProjectMetadataFile();
      if (!path) return;

      await ProjectService.loadProjectToState(path);

      const projectData = await useProjectIOStore.getState().loadProject();
      if (!projectData) {
        uiStore.showToast("加载项目失败", "error", 3000);
        return;
      }

      // 清空当前 tabs，再打开新项目的第一个 graph
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = layoutStore.activeEditorGroupId || 'default_editor';
      const editorNode = layoutStore.nodes[editorGroupId];
      if (editorNode?.data?.tabs) {
        layoutStore.updateNode(editorGroupId, {
          data: { ...editorNode.data, tabs: [], activeTabId: undefined }
        });
      }

      const first = selectFirstGraphResource(
        useResourceStore.getState().resources,
        useResourceStore.getState().graphOrder,
      );
      if (first && (first.kind === 'event' || first.kind === 'function')) {
        void openGraph(first.id, first.name, first.kind);
      }

      uiStore.showToast("项目已加载", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast("加载项目失败", "error", 3000);
    }
  }, [openGraph]);

  const handleOpenWindow = useCallback(async (windowType: string, dataKey: string) => {
    try {
      const label = `${windowType}-${Math.random().toString(36).substring(2, 10)}`;
      const isPlot = windowType === 'scatter' || windowType === 'line' || windowType === 'plot' || windowType === 'ecdf' || windowType === 'kde' || windowType === 'histogram' || windowType === 'correlation' || windowType === 'correlogram';
      const url = isPlot ? `index.html#/plot?key=${dataKey}&type=${windowType}` : `index.html#/info?key=${dataKey}`;
      const plotTitles: Record<string, string> = {
        ecdf: 'ECDF Plot',
        scatter: 'Scatter Plot',
        line: 'Line Plot',
        kde: 'KDE Plot',
        histogram: 'Histogram',
        correlation: 'Correlation Plot',
        correlogram: 'Correlogram',
      };
      const title = isPlot
        ? plotTitles[windowType] ?? 'Plot'
        : 'Regression Results';
      await createPersistedWindow({
        kind: isPlot ? 'plot' : 'info',
        label,
        url,
        title,
      });
    } catch (e) {
      logger.exec.error(`Failed to open window: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, []);

  const executeGraph = useCallback(async (targetGraphId?: string) => {
    try {
      syncActiveToCollection();

      const graphId = targetGraphId ?? (() => {
        const layoutStore = useLayoutStore.getState();
        const editorGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId;
        const editorNode = editorGroupId ? layoutStore.nodes[editorGroupId] : null;
        return editorNode?.data?.activeTabId as string | undefined;
      })();

      if (!graphId) {
        uiStore.showToast("请先打开一个 Event 才能执行", "warning", 3000);
        return;
      }

      const currentGraph = getGraphById(graphId);
      
      if (!currentGraph || currentGraph.type !== 'event') {
        uiStore.showToast("只能执行 Event，当前打开的不是 Event", "warning", 3000);
        return;
      }

      logger.exec.info(`执行当前 Event: ${currentGraph.name} (${graphId})`);

      const recording: RecordedEvent[] = [];
      const { applyEvent, completeExecution, setRecording, startExecution } = useExecutionStore.getState();
      startExecution(graphId);

      const res = await ProjectService.executeProject((event: ExecutionEvent) => {
        if (event.event === 'openWindow') {
          handleOpenWindow(event.data.windowType, event.data.dataKey);
          return;
        }
        applyEvent(graphId, event);
        recording.push({ event, timestamp: Date.now() });
      }, graphId);

      setRecording(graphId, recording);
      if (useExecutionStore.getState().graphs[graphId]?.status === 'running') {
        completeExecution(graphId);
      }

      if (res.logs.length > 0) {
        logger.exec.debug(`执行日志:\n${res.logs.map((line: string) => `  ${line}`).join('\n')}`);
      }
      
      uiStore.showToast(`执行完成: ${currentGraph.name}`, "success", 2000);
    } catch (e) {
      logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
      uiStore.showToast(`执行失败: ${formatErrorMessage(e)}`, "error", 5000);
      const graphId = targetGraphId ?? (() => {
        const layoutStore = useLayoutStore.getState();
        const editorGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId;
        const editorNode = editorGroupId ? layoutStore.nodes[editorGroupId] : null;
        return editorNode?.data?.activeTabId as string | undefined;
      })();
      if (graphId) useExecutionStore.getState().failExecution(graphId);
    }
  }, [handleOpenWindow, syncActiveToCollection]);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
  };
}
