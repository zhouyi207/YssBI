import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { splitComponentForTab } from '@/features/core/layout/layoutTabModel';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { createUntitledGraphResource } from '@/features/application/resource/resourceActions';
import { openGraphInEditor } from '@/features/application/editor/openGraphInEditor';
import { uiStore } from '@/features/core/ui/UIStore';
import { parseUntitledGraphPath } from '@/shared/types/domain/graphResourcePath';
import type { LayoutTab } from '@/shared/types/ui';

/** Move a tab onto another editor group's TabBar (removes from source). */
export function moveTabBetweenGroups(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  useLayoutStore.getState().moveTab(sourceGroupId, tabId, targetGroupId, targetTabIndex);
  useLayoutStore.getState().setActiveGroup(targetGroupId);
}

/**
 * Drag tab to editor canvas edge — VS Code split: copy tab into a new group, keep source.
 */
export function splitEditorWithTab(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): string | null {
  const tab = useLayoutStore.getState().nodes[sourceGroupId]?.data?.tabs?.find((t) => t.id === tabId);
  if (!tab) return null;

  return useLayoutStore.getState().splitEditorGroupAtEdge(targetGroupId, edge, {
    component: tab.component || 'GraphEditor',
    tabs: [{ ...tab }],
    activeTabId: tabId,
  });
}

/** Button / command split — copies active tab to right or bottom. */
export function splitEditorAtEdge(groupId: string, edge: 'right' | 'bottom'): void {
  const nodes = useLayoutStore.getState().nodes;
  const activeTab = nodes[groupId]?.data?.tabs?.find(
    (tab) => tab.id === nodes[groupId]?.data?.activeTabId,
  ) ?? nodes[groupId]?.data?.tabs?.[0];

  useLayoutStore.getState().splitEditorGroupAtEdge(groupId, edge, {
    component: splitComponentForTab(activeTab),
    tabs: activeTab ? [{ ...activeTab, pinned: true as const }] : [],
    activeTabId: activeTab?.id,
    pinSourceActiveTab: true,
  });
}

/** Double-click TabBar empty area — create Untitled-N event in the target editor group. */
export async function createUntitledEventInGroup(groupId: string): Promise<void> {
  try {
    useLayoutStore.getState().setActiveGroup(groupId);
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
