import { useCallback } from 'react';
import { Variable } from '@/shared/types/domain';
import { useNodeStore } from '@/features/core/node-registry/stores';
import { useProjectStore } from '@/features/core/project';
import { useLayoutStore, LayoutState } from '@/features/application/editor/core/stores/layoutStore';
import { VariableService } from '@/services/variable/variableService';

/**
 * Variable Management Hook
 * Handles creation, update, deletion, and promotion/demotion of variables
 */
export function useVariableManagement(switchSidebarTab: (tab: 'events' | 'functions' | 'macros' | 'variables') => void) {
  // const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const activeEditorNode = useLayoutStore((s: LayoutState) => 
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  const addVariable = useCallback(async (name?: string, type: string = "Int32", isGlobal: boolean = false) => {
    try {
      // 构建变量对象
      const variable: Variable = {
        id: `var_${Date.now()}`, // 临时 ID，后端会生成新的
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
      if (isGlobal || !activeTabId) {
        useProjectStore.getState().addVariable(newVarId, newVar);
      } else {
        useNodeStore.getState().addVariable(activeTabId, newVarId, newVar);
      }

      switchSidebarTab('variables');
    } catch (e) {
      console.error("Failed to create variable:", e);
    }
  }, [activeTabId, switchSidebarTab]);

  const updateVariable = useCallback((id: string, data: Partial<Variable>) => {
    const st = useProjectStore.getState();
    const isGlobal = !!st.variables[id];

    // Update variable definition
    if (isGlobal) {
      st.updateVariable(id, data);
    } else {
      const nodeStore = useNodeStore.getState();
      let found = false;
      for (const [tid, tabState] of Object.entries(nodeStore.tabs)) {
        if (tabState.variables[id]) {
          nodeStore.updateVariable(tid, id, data);
          found = true;
          break;
        }
      }
      if (!found && !isGlobal) {
        console.warn(`[useVariableManagement] Variable ${id} not found in any scope.`);
      }
    }

    // Update all nodes referencing this variable
    const nodeStore = useNodeStore.getState();
    Object.keys(nodeStore.tabs).forEach(tid => {
      const nodes = nodeStore.getNodes(tid);
      const needsUpdate = nodes.some((n: any) => n.variableId === id);
      if (!needsUpdate) return;

      const newNodes = nodes.map((n: any) => {
        if (n.variableId !== id) return n;

        // 深拷贝节点
        const clone = JSON.parse(JSON.stringify(n));

        if (data.name) clone.variableName = data.name;
        if (data.data_type) {
          clone.variableType = data.data_type;

          // Update pin types
          if (clone.type === "get_variable") {
            clone.outputs.forEach((p: any) => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
          } else if (clone.type === "set_variable") {
            clone.inputs.forEach((p: any) => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
            clone.outputs.forEach((p: any) => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
          }
        }
        return clone;
      });

      nodeStore.setNodes(tid, newNodes);
    });
  }, []);

  const deleteVariable = useCallback((id: string) => {
    if (useProjectStore.getState().variables[id]) {
      useProjectStore.getState().deleteVariable(id);
    } else {
      if (activeTabId) {
        useNodeStore.getState().removeVariable(activeTabId, id);
      }
    }
  }, [activeTabId]);

  const promoteVariable = useCallback((id: string) => {
    if (!activeTabId) return;
    const v = useNodeStore.getState().tabs[activeTabId]?.variables[id];
    if (!v) return;
    useNodeStore.getState().removeVariable(activeTabId, id);
    useProjectStore.getState().addVariable(id, v);
  }, [activeTabId]);

  const demoteVariable = useCallback((id: string) => {
    const v = useProjectStore.getState().variables[id];
    if (!v) return;
    useProjectStore.getState().deleteVariable(id);
    if (activeTabId) {
      useNodeStore.getState().addVariable(activeTabId, id, v);
    }
  }, [activeTabId]);

  return {
    addVariable,
    updateVariable,
    deleteVariable,
    promoteVariable,
    demoteVariable,
  };
}
