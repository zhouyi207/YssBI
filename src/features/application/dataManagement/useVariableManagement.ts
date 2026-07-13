import { useCallback } from 'react';
import { lookupGraphResourceKind, useResourceStore } from '@/features/core/resource';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
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
  const { activeTabId, tabs } = useActiveEditorGroup();
  const variablesGraphScopePath = useEditorStore((s) => s.variablesGraphScopePath);
  const localGraphPath = variablesGraphScopePath ?? activeTabId;
  const graphTypeFromTab = localGraphPath
    ? tabs.find((t) => t.id === localGraphPath)?.type
    : undefined;
  const graphTypeFromResource = useResourceStore((s) =>
    localGraphPath ? lookupGraphResourceKind(s.resources, localGraphPath) : undefined,
  );
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
      activeGraphPath: isGlobal ? null : localGraphPath,
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
  }, [localGraphPath, graphType, switchSidebarTab]);

  const updateVariable = useCallback(async (id: string, data: Parameters<typeof updateVariableAction>[1]) => {
    await updateVariableAction(id, data);
  }, []);

  const deleteVariable = useCallback(async (id: string) => {
    await deleteVariableAction(id);
  }, []);

  return {
    addVariable,
    updateVariable,
    deleteVariable,
  };
}
