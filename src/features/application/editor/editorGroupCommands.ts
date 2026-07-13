import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { EditorGroupsService } from '@/features/core/layout/editorGroupsService';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { switchEditorTab } from './switchEditorTab';

async function activateCreatedEditorGroup(groupId: string | null): Promise<string | null> {
  if (!groupId) return null;
  const node = useLayoutStore.getState().nodes[groupId];
  const activeTab = node?.data?.tabs?.find((tab) => tab.id === node.data?.activeTabId);
  if (activeTab) await switchEditorTab(groupId, activeTab);
  return groupId;
}

/** Move a tab onto another editor group's TabBar (removes from source). */
export function moveTabBetweenGroups(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  const wasInactive = useLayoutStore.getState().activeEditorGroupId !== targetGroupId;
  EditorGroupsService.moveTab(sourceGroupId, tabId, targetGroupId, targetTabIndex);
  if (!wasInactive && sourceGroupId === targetGroupId) return;
  const targetNode = useLayoutStore.getState().nodes[targetGroupId];
  const activeTab = targetNode?.data?.tabs?.find((tab) => tab.id === targetNode.data?.activeTabId);
  if (activeTab) void switchEditorTab(targetGroupId, activeTab);
}

/**
 * Drag tab to editor edge split:
 * - Multiple tabs in source → move dragged tab to the new group.
 * - Single tab in source → copy (keep source group alive; moving empties it and breaks the grid).
 */
export async function splitEditorWithTab(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  const layoutStore = useLayoutStore.getState();
  const sourceTabs = layoutStore.nodes[sourceGroupId]?.data?.tabs ?? [];
  const tab = sourceTabs.find((t) => t.id === tabId);
  if (!tab) return null;

  const moveFromSource = sourceTabs.length > 1;

  const created = EditorGroupsService.splitGroupAtEdge(targetGroupId, edge, {
    component: tab.component || 'GraphEditor',
    tabs: [{ ...tab, pinned: true }],
    activeTabId: tabId,
  });
  if (!created) return null;

  if (moveFromSource) {
    const sourceStillHasTab = useLayoutStore
      .getState()
      .nodes[sourceGroupId]?.data?.tabs?.some((item) => item.id === tabId);
    if (sourceStillHasTab) {
      useLayoutStore.getState().removeTab(sourceGroupId, tabId);
    }
  }

  return activateCreatedEditorGroup(created);
}

/** Button / command split — copies active tab to right or bottom. */
export async function splitEditorAtEdge(
  groupId: string,
  edge: 'right' | 'bottom',
): Promise<string | null> {
  const created = edge === 'right'
    ? EditorGroupsService.splitActiveTabRight(groupId)
    : EditorGroupsService.splitActiveTabDown(groupId);
  return activateCreatedEditorGroup(created);
}
