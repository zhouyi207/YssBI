import { useCallback } from 'react';
import type { Variable, VariableScope } from '@/shared/types/domain';
import { DEFAULT_VARIABLE_NAME } from '@/shared/constants/defaultResourceNames';
import { dataTypeFromKey, getDefaultValue, dataTypeDisplay } from '@/shared/types/domain/dataType';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { useVariableStore, useGraphMetaStore, useGraphDataStore } from '@/features/core/dataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { VariableService } from '@/services/variable/variableService';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';

/** 根据 activeTabId 和 graph 类型构建 scope */
function buildScope(
  isGlobal: boolean,
  activeTabId: string | null,
  graphType: 'event' | 'function' | undefined
): VariableScope {
  // 明确要求全局变量，或无打开图 → 全局
  if (isGlobal || !activeTabId) return { type: 'global' };
  // 有打开图 → 创建该图的局部变量（优先用 graphType，未知时默认 event）
  const scopeType = graphType ?? 'event';
  switch (scopeType) {
    case 'event':
      return { type: 'event', eventId: activeTabId };
    case 'function':
      return { type: 'function', functionId: activeTabId };
    default:
      return { type: 'event', eventId: activeTabId };
  }
}

/**
 * Variable Management Hook
 * Handles creation, update, deletion, and promotion/demotion of variables
 */
export function useVariableManagement() {
  const switchSidebarTab = useSidebarTab();
  const activeEditorNode = useLayoutStore((s: LayoutState) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;
  // 优先从当前 tab 取 type，否则从 graphMetaStore 取（确保打开图时能正确识别为局部变量）
  const graphTypeFromTab = activeTabId && activeEditorNode?.data?.tabs
    ? activeEditorNode.data.tabs.find((t: { id: string; type?: string }) => t.id === activeTabId)?.type
    : undefined;
  const graphTypeFromMeta = useGraphMetaStore((s) =>
    activeTabId ? s.graphs[activeTabId]?.type : undefined
  );
  const rawType = graphTypeFromTab || graphTypeFromMeta;
  const graphType = (rawType === 'event' || rawType === 'function' ? rawType : undefined) as 'event' | 'function' | undefined;

  const addVariable = useCallback(async (name?: string, type: string = 'Int32', isGlobal: boolean = false) => {
    try {
      const baseName = name || DEFAULT_VARIABLE_NAME;
      const dataType = dataTypeFromKey(type);
      const variable: Omit<Variable, 'id'> = {
        name: baseName,
        dataType,
        dataValue: dataValueFromRaw(getDefaultValue(dataType), dataType),
        description: '',
        scope: buildScope(isGlobal, activeTabId, graphType),
        tags: [],
      };

      const newVarId = await VariableService.createVariable(variable);
      const newVar = await VariableService.getVariable(newVarId);
      useVariableStore.getState().addVariable(newVarId, newVar);

      switchSidebarTab('graphs');
    } catch (e) {
      logger.data.error('Failed to create variable: ' + String(e), 'VariableManagement');
    }
  }, [activeTabId, graphType, switchSidebarTab]);

  const updateVariableReferences = useCallback((id: string, variable: Variable) => {
    // 同步更新所有引用该变量的 get_variable/set_variable 节点的 title 和 variableName
    if (variable) {
      const graphStore = useGraphDataStore.getState();
      for (const [nodeId, node] of Object.entries(graphStore.nodes)) {
        if (node.variableId === id) {
          const prefix = node.nodeType === 'Variables:Set Variable' ? 'Set ' : 'Get ';
          graphStore.updateNode(nodeId, {
            title: prefix + variable.name,
            variableName: variable.name,
            variableType: dataTypeDisplay(variable.dataType),
          });
        }
      }
    }
  }, []);

  const updateVariable = useCallback(async (id: string, data: Partial<Variable>) => {
    const store = useVariableStore.getState();
    const previous = store.variables[id];
    if (!previous) return;

    try {
      await VariableService.updateVariable(id, data);
      const next = { ...previous, ...data };
      useVariableStore.getState().updateVariable(id, data);
      updateVariableReferences(id, next);
    } catch (e) {
      logger.data.error('Failed to update variable in backend: ' + String(e), 'VariableManagement');
      uiStore.showToast(`变量更新失败: ${e}`, 'error');
    }
  }, [updateVariableReferences]);

  const deleteVariable = useCallback(async (id: string) => {
    const previous = useVariableStore.getState().variables[id];
    if (!previous) return;

    try {
      await VariableService.deleteVariable(id);
      useVariableStore.getState().deleteVariable(id);
    } catch (e) {
      logger.data.error('Failed to delete variable in backend: ' + String(e), 'VariableManagement');
      uiStore.showToast(`变量删除失败: ${e}`, 'error');
    }
  }, []);

  const promoteVariable = useCallback((_id: string) => {
    // No-op in new architecture - all variables are in project store
    logger.data.debug('promoteVariable is no-op in new architecture', 'VariableManagement');
  }, []);

  const demoteVariable = useCallback((_id: string) => {
    // No-op in new architecture - all variables are in project store
    logger.data.debug('demoteVariable is no-op in new architecture', 'VariableManagement');
  }, []);

  return {
    addVariable,
    updateVariable,
    deleteVariable,
    promoteVariable,
    demoteVariable,
  };
}
