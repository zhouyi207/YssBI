import { useCallback } from 'react';
import type { Variable, VariableScope } from '@/shared/types/domain';
import { dataTypeFromKey, getDefaultValue } from '@/shared/types/domain/dataType';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { useVariableStore, useGraphMetaStore } from '@/features/core/dataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { VariableService } from '@/services/variable/variableService';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';

/** 根据 activeTabId 和 graph 类型构建 scope */
function buildScope(
  isGlobal: boolean,
  activeTabId: string | null,
  graphType: 'event' | 'function' | 'macro' | undefined
): VariableScope {
  if (isGlobal || !activeTabId) return { type: 'global' };
  switch (graphType) {
    case 'event':
      return { type: 'event', eventId: activeTabId };
    case 'function':
      return { type: 'function', functionId: activeTabId };
    case 'macro':
      return { type: 'macro', macroId: activeTabId };
    default:
      return { type: 'global' };
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
  const graphType = useGraphMetaStore((s) =>
    activeTabId ? s.graphs[activeTabId]?.type : undefined
  );

  const addVariable = useCallback(async (name?: string, type: string = 'Int32', isGlobal: boolean = false) => {
    try {
      const dataType = dataTypeFromKey(type);
      const variable: Omit<Variable, 'id'> = {
        name: name || `variable_${Date.now()}`,
        dataType,
        dataValue: dataValueFromRaw(getDefaultValue(dataType), dataType),
        description: '',
        scope: buildScope(isGlobal, activeTabId, graphType),
        tags: [],
      };

      const newVarId = await VariableService.createVariable(variable);
      const newVar = await VariableService.getVariable(newVarId);
      useVariableStore.getState().addVariable(newVarId, newVar);

      switchSidebarTab('variables');
    } catch (e) {
      console.error('Failed to create variable:', e);
    }
  }, [activeTabId, graphType, switchSidebarTab]);

  const updateVariable = useCallback((id: string, data: Partial<Variable>) => {
    useVariableStore.getState().updateVariable(id, data);
  }, []);

  const deleteVariable = useCallback((id: string) => {
    useVariableStore.getState().deleteVariable(id);
  }, []);

  const promoteVariable = useCallback((_id: string) => {
    // No-op in new architecture - all variables are in project store
    console.log('[useVariableManagement] promoteVariable is no-op in new architecture');
  }, []);

  const demoteVariable = useCallback((_id: string) => {
    // No-op in new architecture - all variables are in project store
    console.log('[useVariableManagement] demoteVariable is no-op in new architecture');
  }, []);

  return {
    addVariable,
    updateVariable,
    deleteVariable,
    promoteVariable,
    demoteVariable,
  };
}
