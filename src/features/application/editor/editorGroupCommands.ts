import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { EditorGroupsService } from '@/features/core/layout/editorGroupsService';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { createUntitledGraphResource } from '@/features/application/resource/resourceActions';
import { openGraphInEditor } from '@/features/application/editor/openGraphInEditor';
import { uiStore } from '@/features/core/ui/UIStore';
import { parseUntitledGraphPath } from '@/shared/types/domain/graphResourcePath';
import type { LayoutTab } from '@/shared/types/ui';
import { activateEditorGroup, switchEditorTab } from './switchEditorTab';

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
 * Drag tab to editor canvas edge — VS Code split: copy tab into a new group, keep source.
 */
export async function splitEditorWithTab(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  const tab = useLayoutStore.getState().nodes[sourceGroupId]?.data?.tabs?.find((t) => t.id === tabId);
  if (!tab) return null;

  const created = EditorGroupsService.splitGroupAtEdge(targetGroupId, edge, {
    component: tab.component || 'GraphEditor',
    tabs: [{ ...tab }],
    activeTabId: tabId,
  });
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

/** Double-click TabBar empty area — create Untitled-N event in the target editor group. */
export async function createUntitledEventInGroup(groupId: string): Promise<void> {
  try {
    await activateEditorGroup(groupId);
    const graphPath = await createUntitledGraphResource('event');
    const parsed = parseUntitledGraphPath(graphPath);
    const name = parsed?.label ?? graphPath;
    await openGraphInEditor(graphPath, name, 'event', groupId);
  } catch (error) {
    uiStore.showToast(formatErrorMessage(error), 'error');
    throw error;
  }
}

export function findTabInEditorGroup(groupId: string, tabId: string): LayoutTab | undefined {
  return useLayoutStore.getState().nodes[groupId]?.data?.tabs?.find((tab) => tab.id === tabId);
}
