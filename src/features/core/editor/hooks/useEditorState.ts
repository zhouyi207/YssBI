/**
 * 编辑器状态（组合 hook）
 * 组合 useActiveEditorGroup、useEditorGraphData、useEditorCollections、useEditorGroups、useEditorUIState
 */

import { useMemo } from 'react';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorGraphData } from './useEditorGraphData';
import { useEditorCollections } from './useEditorCollections';
import { useEditorGroups } from './useEditorGroups';
import { useEditorUIState } from './useEditorUIState';

export function useEditorState(overrideGroupId?: string | null) {
  const active = useActiveEditorGroup(overrideGroupId);
  const { nodes, variables } = useEditorGraphData(active.activeTabId);
  const collections = useEditorCollections();
  const groups = useEditorGroups();
  const uiState = useEditorUIState();

  return useMemo(
    () => ({
      activeGroupId: active.activeGroupId,
      activeEditorGroupId: active.activeEditorGroupId,
      activeTabId: active.activeTabId,
      groupId: active.groupId,
      tabs: active.tabs,
      nodes,
      variables,
      selectedNodeIds: active.selectedNodeIds,
      ...collections,
      groups,
      ...uiState,
    }),
    [active, nodes, variables, collections, groups, uiState]
  );
}
