/**
 * 编辑器状态（组合 hook）
 * 组合 useActiveEditorGroup、useEditorCollections、useEditorGroups、useEditorUIState
 */

import { useMemo } from 'react';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorCollections } from './useEditorCollections';
import { useEditorGroups } from './useEditorGroups';
import { useEditorUIState } from './useEditorUIState';

type ActiveEditorGroup = ReturnType<typeof useActiveEditorGroup>;

function buildEditorState(
  active: ActiveEditorGroup,
  collections: ReturnType<typeof useEditorCollections>,
  groups: ReturnType<typeof useEditorGroups>,
  uiState: ReturnType<typeof useEditorUIState>,
) {
  return {
    activeGroupId: active.activeEditorGroupId,
    activeEditorGroupId: active.activeEditorGroupId,
    activeTabId: active.activeTabId,
    groupId: active.groupId,
    tabs: active.tabs,
    selectedNodeIds: active.selectedNodeIds,
    ...collections,
    groups,
    ...uiState,
  };
}

export function useEditorState(overrideGroupId?: string | null) {
  const active = useActiveEditorGroup(overrideGroupId);
  const collections = useEditorCollections();
  const groups = useEditorGroups();
  const uiState = useEditorUIState();

  return useMemo(
    () => buildEditorState(active, collections, groups, uiState),
    [active, collections, groups, uiState],
  );
}

export { buildEditorState };
