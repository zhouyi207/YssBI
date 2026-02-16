import { useCallback } from 'react';
import { Variable } from '@/shared/types/domain';
import { useVariableStore } from '@/features/core/dataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { VariableService } from '@/services/variable/variableService';
import { useSidebarTab } from './useSidebarTab';

/**
 * Variable Management Hook
 * Handles creation, update, deletion, and promotion/demotion of variables
 */
export function useVariableManagement() {
  const switchSidebarTab = useSidebarTab();
  // const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const activeEditorNode = useLayoutStore((s: LayoutState) => 
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  const addVariable = useCallback(async (name?: string, type: string = "Int32", isGlobal: boolean = false) => {
    try {
      // 构建变量对象（id 由后端 create_variable 分配）
      const variable: Variable = {
        id: "", // 占位符，后端分配真实 ID
        name: name || `variable_${Date.now()}`,
        data_type: type as any,
        description: '',
        scope: isGlobal 
          ? { type: 'global' }
          : activeTabId 
            ? { type: 'function', function_id: activeTabId } // 假设是 function，也可能是 macro
            : { type: 'global' }, // 如果没有 activeTabId，默认为全局
        static_value: undefined,
        is_array: false,
        is_constant: false,
        default_value: undefined,
        is_exposed: false,
        tags: [],
      };

      // 调用后端创建变量
      const newVarId = await VariableService.createVariable(variable);
      
      // 获取创建后的变量
      const newVar = await VariableService.getVariable(newVarId);

      // 更新前端状态
      useVariableStore.getState().addVariable(newVarId, newVar);

      switchSidebarTab('variables');
    } catch (e) {
      console.error("Failed to create variable:", e);
    }
  }, [activeTabId, switchSidebarTab]);

  const updateVariable = useCallback((id: string, data: Partial<Variable>) => {
    useVariableStore.getState().updateVariable(id, data);
  }, []);

  const deleteVariable = useCallback((id: string) => {
    useVariableStore.getState().deleteVariable(id);
  }, []);

  const promoteVariable = useCallback((id: string) => {
    // No-op in new architecture - all variables are in project store
    console.log('[useVariableManagement] promoteVariable is no-op in new architecture');
  }, []);

  const demoteVariable = useCallback((id: string) => {
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
