import { useCallback } from 'react';
import { useProjectIOStore, getGraphById } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/core/ui/UIStore';
import { useExecutionStore } from '@/features/core/execution';
import type { ExecutionEvent, RecordedEvent } from '@/shared/types/ui/execution';
import { logger } from '@/utils/appLogger';

/**
 * Project Operations Hook
 * Handles save, load, and execute operations
 */
export function useProjectOperations(openGraph: (id: string, name: string, type: any, data?: any) => void) {
  const currentPath = useProjectIOStore((s) => s.currentPath);
  const setCurrentPath = useProjectIOStore((s) => s.setCurrentPath);

  // 注意：新架构中不需要 syncActiveToCollection，后端事件会自动同步
  const syncActiveToCollection = useCallback(() => {
    // TODO: 如果需要，实现新的同步逻辑
    logger.app.debug('syncActiveToCollection called (no-op in new architecture)', 'ProjectOperations');
  }, []);

  const saveGraphAs = useCallback(async () => {
    try {
      syncActiveToCollection();
      const path = await ProjectService.saveProjectFromState();
      if (path) {
        setCurrentPath(path);
        uiStore.showToast("项目已保存", "success", 2000);
      }
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
    }
  }, [syncActiveToCollection, setCurrentPath]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) return saveGraphAs();
    syncActiveToCollection();
    try {
      await ProjectService.saveProjectFromState(currentPath);
      uiStore.showToast("项目已保存", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast("保存失败", "error", 2000);
    }
  }, [currentPath, saveGraphAs, syncActiveToCollection]);

  const importGraph = useCallback(async () => {
    try {
      const result = await ProjectService.loadProjectToState();
      if (!result) return; // 用户取消选择文件

      const projectData = await useProjectIOStore.getState().syncFromBackend();
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

      const first = Object.values(projectData.graphs)[0] as any;
      if (first) openGraph(first.id, first.name, first.type as any, first);

      uiStore.showToast("项目已加载", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast("加载项目失败", "error", 3000);
    }
  }, [openGraph]);

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
      const { applyEvent, setRecording, startExecution } = useExecutionStore.getState();
      startExecution(graphId);

      const res = await ProjectService.executeProject((event: ExecutionEvent) => {
        applyEvent(graphId, event);
        recording.push({ event, timestamp: Date.now() });
      }, graphId);

      setRecording(graphId, recording);

      if (res.logs.length > 0) {
        logger.exec.debug(`执行日志:\n${res.logs.map((line: string) => `  ${line}`).join('\n')}`);
      }
      
      uiStore.showToast(`执行完成: ${currentGraph.name}`, "success", 2000);
    } catch (e) {
      logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
      uiStore.showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [syncActiveToCollection]);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
  };
}
