/**
 * Focused editor group workspace — tabs/activeTab from editorTabStore placements.
 */

import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  const focusedEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const groupId = overrideGroupId ?? focusedEditorGroupId ?? DEFAULT_EDITOR_GROUP_ID;

  const node = useLayoutStore((s: LayoutState) => s.nodes[groupId]);
  const placement = useEditorGroupPlacement(groupId);

  return {
    groupId,
    focusedEditorGroupId,
    activeTabId: placement.activeTabId,
    tabs: placement.tabs,
    selectedNodeIds: placement.selectedNodeIds,
    node,
  };
}
