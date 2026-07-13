/**
 * 编辑器组工作区数据：groupId、tabs、activeTabId、selectedNodeIds
 * 订阅 editorTabStore 的 per-group placement；groupId 来自 GroupContext 或 layout focus。
 */

import { useContext, useMemo } from 'react';
import { GroupContext } from '../context/GroupContext';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const contextGroupId = useContext(GroupContext);
  const scopedGroupId = overrideGroupId ?? contextGroupId;

  const focusedGroupId = useLayoutStore((s) => s.activeEditorGroupId);
  const groupId = scopedGroupId ?? focusedGroupId ?? DEFAULT_EDITOR_GROUP_ID;

  const placement = useEditorGroupPlacement(groupId);

  return useMemo(
    () => ({
      groupId,
      tabs: placement.tabs,
      activeTabId: placement.activeTabId,
      selectedNodeIds: placement.selectedNodeIds,
    }),
    [groupId, placement.tabs, placement.activeTabId, placement.selectedNodeIds],
  );
}
