/**
 * 编辑器状态（组合 hook）
 * 组合 useActiveEditorGroup、useEditorCollections、useEditorGroups、useEditorUIState
 */

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
    activeEditorGroupId: active.focusedEditorGroupId ?? active.groupId,
    activeTabId: active.activeTabId,
    groupId: active.groupId,
    tabs: active.tabs,
    selectedNodeIds: active.selectedNodeIds,
    ...collections,
    groups,
    ...uiState,
  };
}

export { buildEditorState };
