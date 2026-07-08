import { useLayoutStore } from '@/features/core/layout/layoutStore';
import type { LayoutTab } from '@/shared/types/ui';
import { switchEditorTab, activateCurrentEditorTab } from './switchEditorTab';

type GraphTabType = Extract<LayoutTab['type'], 'event' | 'function'>;

function resolveTab(
  groupId: string,
  tabId: string,
  tab?: Pick<LayoutTab, 'type' | 'id'> | null,
): LayoutTab | undefined {
  if (tab && tab.id === tabId) {
    return tab as LayoutTab;
  }
  return useLayoutStore.getState().nodes[groupId]?.data?.tabs?.find((item) => item.id === tabId);
}

/** @deprecated Prefer `switchEditorTab` with a resolved `LayoutTab`. */
export async function switchEditorGraphTab(
  groupId: string,
  tabId: string,
  tab?: Pick<LayoutTab, 'type' | 'id'> | null,
): Promise<boolean> {
  const resolvedTab = resolveTab(groupId, tabId, tab);
  if (!resolvedTab) return false;
  return switchEditorTab(groupId, resolvedTab);
}

export type { GraphTabType };

/** @deprecated Prefer `activateCurrentEditorTab`. */
export async function activateCurrentGraphTab(groupId: string): Promise<boolean> {
  return activateCurrentEditorTab(groupId);
}
