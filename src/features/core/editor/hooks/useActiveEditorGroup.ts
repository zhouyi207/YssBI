/**
 * 获取当前活动的编辑器组和 Tab 信息
 * 仅依赖 layout store
 */

import { useMemo } from 'react';
import type { LayoutTab } from '@/shared/types';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { normalizeLayoutTabs } from '@/features/core/layout/layoutTabModel';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const groupId = overrideGroupId ?? activeEditorGroupId ?? 'default_editor';

  const node = useLayoutStore((s: LayoutState) => s.nodes[groupId]);

  const tabs: LayoutTab[] = useMemo(
    () => normalizeLayoutTabs(node?.data?.tabs ?? []),
    [node?.data?.tabs],
  );
  const activeTabId = node?.data?.activeTabId || null;
  const selectedNodeIds = node?.data?.params?.selectedNodeIds || [];

  return {
    groupId,
    activeEditorGroupId: groupId,
    activeTabId,
    tabs,
    selectedNodeIds,
    node,
  };
}
