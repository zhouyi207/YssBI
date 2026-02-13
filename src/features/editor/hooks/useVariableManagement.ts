import { useCallback } from 'react';
import { VariableDefinition } from '@/shared/types/editor';
import { useNodeStore } from '@/features/node-registry/stores';
import { useProjectStore } from '@/features/project';
import { useLayoutStore, LayoutState } from '@/features/layoutStore/layoutStore';
import { ProjectService } from '@/services/project/projectService';

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

  const addVariable = useCallback(async (name?: string, type: string = "int", isGlobal: boolean = false) => {
    let scopeId: string | null = null;

    if (!isGlobal && activeTabId) {
      scopeId = activeTabId;
    }

    try {
      const newVar = await ProjectService.createVariable(scopeId, name, type);

      if (scopeId) {
        useNodeStore.getState().addVariable(scopeId, newVar.id, newVar);
      } else {
        useProjectStore.getState().addGlobalVariable(newVar.id, newVar);
      }

      switchSidebarTab('variables');
    } catch (e) {
      console.error("Failed to create variable:", e);
    }
  }, [activeTabId, switchSidebarTab]);

  const updateVariable = useCallback((id: string, data: Partial<VariableDefinition>) => {
    const st = useProjectStore.getState();
    const isGlobal = !!st.globalVariables[id];

    // Update variable definition
    if (isGlobal) {
      st.updateGlobalVariable(id, data);
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
      const needsUpdate = nodes.some(n => n.variableId === id);
      if (!needsUpdate) return;

      const newNodes = nodes.map(n => {
        if (n.variableId !== id) return n;

        const clone = n.clone();

        if (data.name) clone.variableName = data.name;
        if (data.data_type) {
          clone.variableType = data.data_type;

          // Update pin types
          if (clone.type === "get_variable") {
            clone.outputs.forEach(p => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
          } else if (clone.type === "set_variable") {
            clone.inputs.forEach(p => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
            clone.outputs.forEach(p => {
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
    if (useProjectStore.getState().globalVariables[id]) {
      useProjectStore.getState().deleteGlobalVariable(id);
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
    useProjectStore.getState().addGlobalVariable(id, v);
  }, [activeTabId]);

  const demoteVariable = useCallback((id: string) => {
    const v = useProjectStore.getState().globalVariables[id];
    if (!v) return;
    useProjectStore.getState().deleteGlobalVariable(id);
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
