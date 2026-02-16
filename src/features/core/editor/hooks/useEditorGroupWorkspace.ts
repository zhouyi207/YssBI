import { useMemo } from 'react';
import { useEditorGroup } from '@/features/application/editor/core/hooks/useEditorGroup';

export function useEditorGroupWorkspace() {
  const {
    groupId,
    activeGroupId,
    tabs,
    activeTabId,
    nodes,
    variables,
    selectedNodeIds,
  } = useEditorGroup();

  return useMemo(() => ({
    groupId,
    activeGroupId,
    tabs,
    activeTabId,
    nodes,
    variables,
    selectedNodeIds,
  }), [
    groupId,
    activeGroupId,
    tabs,
    activeTabId,
    nodes,
    variables,
    selectedNodeIds,
  ]);
}
