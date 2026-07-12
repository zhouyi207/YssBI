/**
 * 获取当前活动的编辑器组和 Tab 信息
 * 仅依赖 layout store
 */

import { useMemo } from 'react';
import type { LayoutTab } from '@/shared/types';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { normalizeLayoutTabs } from '@/features/core/layout/layoutTabModel';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  /** Globally focused editor group in layout store (nullable before hydrate). */
  const focusedEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  /** Group identity for this hook consumer (explicit override, else focused, else default). */
  const groupId = overrideGroupId ?? focusedEditorGroupId ?? DEFAULT_EDITOR_GROUP_ID;

  const node = useLayoutStore((s: LayoutState) => s.nodes[groupId]);

  const tabs: LayoutTab[] = useMemo(
    () => normalizeLayoutTabs(node?.data?.tabs ?? []),
    [node?.data?.tabs],
  );
  const activeTabId = node?.data?.activeTabId || null;
  const selectedNodeIds = node?.data?.params?.selectedNodeIds || [];

  return {
    groupId,
    focusedEditorGroupId,
    activeTabId,
    tabs,
    selectedNodeIds,
    node,
  };
}
