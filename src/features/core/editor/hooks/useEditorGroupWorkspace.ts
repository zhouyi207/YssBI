/**
 * 编辑器组工作区数据：groupId、tabs、activeTabId、selectedNodeIds
 */
import { useMemo } from 'react';
import { useActiveEditorGroup } from './useActiveEditorGroup';

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const active = useActiveEditorGroup(overrideGroupId);

  return useMemo(
    () => ({
      groupId: active.groupId,
      activeGroupId: active.activeEditorGroupId,
      tabs: active.tabs,
      activeTabId: active.activeTabId,
      selectedNodeIds: active.selectedNodeIds,
    }),
    [active]
  );
}
