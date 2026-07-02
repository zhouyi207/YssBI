import { useCallback } from 'react';
import { useResourceStore } from '@/features/core/resource';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useSidebarStore } from '@/features/core/sidebar';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import {
  createVariableAction,
  deleteVariableAction,
  updateVariableAction,
} from '@/features/application/dataManagement/variableActions';

/**
 * Variable Management Hook
 * Binds UI state and delegates CRUD to variable application actions.
 */
export function useVariableManagement() {
  const switchSidebarTab = useSidebarTab();
  const activeEditorNode = useLayoutStore((s: LayoutState) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;
  const graphTypeFromTab = activeTabId && activeEditorNode?.data?.tabs
    ? activeEditorNode.data.tabs.find((t: { id: string; type?: string }) => t.id === activeTabId)?.type
    : undefined;
  const graphTypeFromResource = useResourceStore((s) => {
    if (!activeTabId) return undefined;
    if (s.resources[`graph:event:${activeTabId}`]?.exists) return 'event';
    if (s.resources[`graph:function:${activeTabId}`]?.exists) return 'function';
    return undefined;
  });
  const rawType = graphTypeFromTab || graphTypeFromResource;
  const graphType = (rawType === 'event' || rawType === 'function' ? rawType : undefined) as 'event' | 'function' | undefined;

  const addVariable = useCallback(async (
    name?: string,
    type: string = 'Int64',
    isGlobal: boolean = false,
  ) => {
    const created = await createVariableAction({
      name,
      type,
      isGlobal,
      activeGraphId: isGlobal ? null : activeTabId,
      graphType: isGlobal ? undefined : graphType,
    });
    if (created) {
      switchSidebarTab('variables');
      const sidebar = useSidebarStore.getState();
      if (isGlobal) {
        sidebar.setSectionExpanded('variablesGlobal', true);
      } else {
        sidebar.setSectionExpanded('variablesLocal', true);
      }
    }
  }, [activeTabId, graphType, switchSidebarTab]);

  const updateVariable = useCallback(async (id: string, data: Parameters<typeof updateVariableAction>[1]) => {
    await updateVariableAction(id, data);
  }, []);

  const deleteVariable = useCallback(async (id: string) => {
    await deleteVariableAction(id);
  }, []);

  const promoteVariable = useCallback((_id: string) => {
    // No-op in document-owned variable architecture
  }, []);

  const demoteVariable = useCallback((_id: string) => {
    // No-op in document-owned variable architecture
  }, []);

  return {
    addVariable,
    updateVariable,
    deleteVariable,
    promoteVariable,
    demoteVariable,
  };
}
