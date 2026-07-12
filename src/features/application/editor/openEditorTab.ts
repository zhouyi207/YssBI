import type { LayoutTab } from '@/shared/types';
import type { DetailFocus } from '@/features/core/editor/detail/types';
import { useEditorStore } from '@/features/core/editor';
import {
  getLayoutTabById,
  isEditorGroupNode,
  resolveEditorTargetGroupId,
  useLayoutStore,
} from '@/features/core/layout';
import {
  applyTabPinState,
  findPreviewTabInTabs,
} from '@/features/core/layout/layoutTabModel';
import { applyEditorTabSelection } from './editorTabSelection';
import { ensureDetailVisible } from './ensureDetailVisible';

export interface OpenEditorTabOptions {
  targetGroupId?: string;
  focusDetail?: DetailFocus;
  /** `false` opens in the preview slot (VS Code single-click). Default: pinned. */
  pinned?: boolean;
}

/**
 * Open or activate a tab in the main editor area.
 * Recovers tabs that were previously attached to fixed chrome nodes (sidebar/detail).
 */
export function openEditorTab(tab: LayoutTab, options?: OpenEditorTabOptions): void {
  const pinned = options?.pinned !== false;
  const tabToOpen = applyTabPinState(tab, pinned);

  const layoutStore = useLayoutStore.getState();
  const editorGroupId = resolveEditorTargetGroupId(options?.targetGroupId);
  const existing = getLayoutTabById(tab.id);

  if (existing) {
    const existingNode = layoutStore.nodes[existing.nodeId];
    if (isEditorGroupNode(existingNode)) {
      if (pinned && existing.tab.pinned === false) {
        layoutStore.setTabPinned(existing.nodeId, tab.id, true);
      }
      if (existingNode.data?.activeTabId !== tab.id) {
        applyEditorTabSelection(existing.nodeId, tab.id);
      }
      layoutStore.setActiveGroup(existing.nodeId);
    } else {
      layoutStore.moveTab(existing.nodeId, tab.id, editorGroupId);
      if (pinned) {
        layoutStore.setTabPinned(editorGroupId, tab.id, true);
      }
    }
  } else if (!pinned) {
    openPreviewTabInGroup(editorGroupId, tabToOpen);
  } else {
    layoutStore.addTab(editorGroupId, tabToOpen);
    layoutStore.setActiveGroup(editorGroupId);
  }

  if (options?.focusDetail) {
    useEditorStore.getState().setDetailFocus(options.focusDetail);
  }
  ensureDetailVisible();
}

/** At most one preview tab per editor group; replaces the previous preview when opening another. */
function openPreviewTabInGroup(groupId: string, tab: LayoutTab): void {
  const layoutStore = useLayoutStore.getState();
  const groupTabs = layoutStore.nodes[groupId]?.data?.tabs;
  const previewTab = findPreviewTabInTabs(groupTabs);

  if (previewTab && previewTab.id !== tab.id) {
    layoutStore.removeTab(groupId, previewTab.id);
  }

  const tabsAfterReplace = useLayoutStore.getState().nodes[groupId]?.data?.tabs;
  if (tabsAfterReplace?.some((item) => item.id === tab.id)) {
    applyEditorTabSelection(groupId, tab.id);
  } else {
    layoutStore.addTab(groupId, tab);
  }
  layoutStore.setActiveGroup(groupId);
}
