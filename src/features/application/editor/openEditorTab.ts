import type { LayoutTab } from '@/shared/types';
import type { DetailFocus } from '@/features/core/editor/detail/types';
import { useEditorStore } from '@/features/core/editor';
import {
  getLayoutTabById,
  isEditorGroupNode,
  resolveEditorTargetGroupId,
  useLayoutStore,
} from '@/features/core/layout';
import { applyEditorTabSelection } from './editorTabSelection';
import { ensureDetailVisible } from './ensureDetailVisible';

export interface OpenEditorTabOptions {
  targetGroupId?: string;
  focusDetail?: DetailFocus;
}

/**
 * Open or activate a tab in the main editor area.
 * Recovers tabs that were previously attached to fixed chrome nodes (sidebar/detail).
 */
export function openEditorTab(tab: LayoutTab, options?: OpenEditorTabOptions): void {
  const layoutStore = useLayoutStore.getState();
  const editorGroupId = resolveEditorTargetGroupId(options?.targetGroupId);
  const existing = getLayoutTabById(tab.id);

  if (existing) {
    const existingNode = layoutStore.nodes[existing.nodeId];
    if (isEditorGroupNode(existingNode)) {
      if (existingNode.data?.activeTabId !== tab.id) {
        applyEditorTabSelection(existing.nodeId, tab.id);
      }
      layoutStore.setActiveGroup(existing.nodeId);
    } else {
      layoutStore.moveTab(existing.nodeId, tab.id, editorGroupId);
    }
  } else {
    layoutStore.addTab(editorGroupId, tab);
    layoutStore.setActiveGroup(editorGroupId);
  }

  if (options?.focusDetail) {
    useEditorStore.getState().setDetailFocus(options.focusDetail);
  }
  ensureDetailVisible();
}
