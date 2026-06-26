/**
 * 编辑器组工作区数据：groupId、tabs、activeTabId、nodes、variables、selectedNodeIds
 * 直接使用 core hooks，无 application 依赖
 */
import { useMemo } from 'react';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorGraphData } from './useEditorGraphData';

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const active = useActiveEditorGroup(overrideGroupId);
  const { variables } = useEditorGraphData();

  return useMemo(
    () => ({
      groupId: active.groupId,
      activeGroupId: active.activeGroupId,
      tabs: active.tabs,
      activeTabId: active.activeTabId,
      variables,
      selectedNodeIds: active.selectedNodeIds,
    }),
    [active, variables]
  );
}
