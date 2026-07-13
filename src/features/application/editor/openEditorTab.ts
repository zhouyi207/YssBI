import type { LayoutTab } from '@/shared/types';
import type { DetailFocus } from '@/features/core/editor/detail/types';
import { useEditorStore } from '@/features/core/editor';
import {
  getLayoutTabById,
  isEditorGroupNode,
  resolveEditorTargetGroupId,
  useLayoutStore,
} from '@/features/core/layout';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import {
  applyTabPinState,
  findPreviewTabInTabs,
} from '@/features/core/layout/layoutTabModel';
import { applyEditorTabSelection } from './editorTabSelection';
import { ensureDetailVisible } from './ensureDetailVisible';

export interface OpenEditorTabOptions {
  targetGroupId?: string;
  /** Insert or move to this index on the target editor group TabBar. */
  insertIndex?: number;
  focusDetail?: DetailFocus;
  /** `false` opens in the preview slot (VS Code single-click). Default: pinned. */
  pinned?: boolean;
}

/**
 * Open or activate a tab in the main editor area.
 */
export function openEditorTab(tab: LayoutTab, options?: OpenEditorTabOptions): void {
  const pinned = options?.pinned !== false;
  const tabToOpen = applyTabPinState(tab, pinned);
  const editorGroupId = resolveEditorTargetGroupId(options?.targetGroupId);
  const insertIndex = options?.insertIndex;
  const layoutStore = useLayoutStore.getState();
  const tabStore = useEditorTabStore.getState();
  const existing = getLayoutTabById(tab.id);

  if (existing) {
    const fromNodeId = existing.nodeId;

    if (pinned && existing.tab.pinned === false) {
      layoutStore.setTabPinned(fromNodeId, tab.id, true);
    }

    const needsMove =
      insertIndex !== undefined
      || !isEditorGroupNode(layoutStore.nodes[fromNodeId])
      || fromNodeId !== editorGroupId;

    if (needsMove) {
      layoutStore.moveTab(fromNodeId, tab.id, editorGroupId, insertIndex);
    } else if (tabStore.getPlacement(fromNodeId).activeTabId !== tab.id) {
      applyEditorTabSelection(fromNodeId, tab.id);
    }

    layoutStore.setActiveGroup(editorGroupId);
  } else if (!pinned) {
    openPreviewTabInGroup(editorGroupId, tabToOpen);
  } else {
    layoutStore.addTab(editorGroupId, tabToOpen, insertIndex);
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
  const tabStore = useEditorTabStore.getState();
  const groupTabs = tabStore.resolveGroupTabs(groupId);
  const previewTab = findPreviewTabInTabs(groupTabs);

  if (previewTab && previewTab.id !== tab.id) {
    layoutStore.removeTab(groupId, previewTab.id);
  }

  const tabsAfterReplace = useEditorTabStore.getState().resolveGroupTabs(groupId);
  if (tabsAfterReplace.some((item) => item.id === tab.id)) {
    applyEditorTabSelection(groupId, tab.id);
  } else {
    layoutStore.addTab(groupId, tab);
  }
  layoutStore.setActiveGroup(groupId);
}
