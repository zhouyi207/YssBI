/**
 * 获取当前活动的编辑器组和 Tab 信息
 * 仅依赖 layout store
 */

import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  const activeGroupIdFromStore = useLayoutStore((s: LayoutState) => s.activeGroupId);
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const groupId = overrideGroupId ?? activeGroupIdFromStore ?? 'default_editor';
  const editorGroupId = activeEditorGroupId || 'default_editor';

  const node = useLayoutStore((s: LayoutState) => s.nodes[groupId]);
  const editorNode = useLayoutStore((s: LayoutState) => s.nodes[editorGroupId]);

  const isEditor = node?.type === 'component' && !!node.data?.tabs;
  const functionalNode = isEditor ? node : editorNode ?? node;

  const tabs = functionalNode?.data?.tabs || [];
  const activeTabId = functionalNode?.data?.activeTabId || null;
  const selectedNodeIds = functionalNode?.data?.params?.selectedNodeIds || [];

  return {
    groupId,
    activeGroupId: activeGroupIdFromStore ?? 'default_editor',
    activeEditorGroupId: editorGroupId,
    activeTabId,
    tabs,
    selectedNodeIds,
    node,
  };
}
