/**
 * 获取当前活动的编辑器组和 Tab 信息
 * 仅依赖 layout store
 */

import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  const activeGroupIdFromStore = useLayoutStore((s: LayoutState) => s.activeGroupId);
  const groupId = overrideGroupId ?? activeGroupIdFromStore ?? 'default_editor';

  const node = useLayoutStore((s: LayoutState) => s.nodes[groupId]);
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);

  const isEditor = node?.type === 'component' && !!node.data?.tabs;
  const functionalNode = isEditor ? node : useLayoutStore.getState().nodes[activeEditorGroupId || ''] || node;

  const tabs = functionalNode?.data?.tabs || [];
  const activeTabId = functionalNode?.data?.activeTabId || null;
  const selectedNodeIds = functionalNode?.data?.params?.selectedNodeIds || [];

  return {
    groupId,
    activeGroupId: activeGroupIdFromStore ?? 'default_editor',
    activeEditorGroupId: activeEditorGroupId ?? 'default_editor',
    activeTabId,
    tabs,
    selectedNodeIds,
    node,
  };
}
