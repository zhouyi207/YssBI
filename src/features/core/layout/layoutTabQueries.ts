import type { LayoutTab, LayoutTree } from '@/shared/types';
import { useLayoutStore } from './layoutStore';

export type LocatedLayoutTab = { nodeId: string; tab: LayoutTab };

export interface LayoutGroupContext {
  activeEditorGroupId: string | null;
  activeGroupId: string | null;
}

function readNodes(nodes?: LayoutTree): LayoutTree {
  return nodes ?? useLayoutStore.getState().nodes;
}

export function getLayoutTabById(tabId: string, nodes?: LayoutTree): LocatedLayoutTab | null {
  const tree = readNodes(nodes);
  for (const node of Object.values(tree)) {
    const tab = node.data?.tabs?.find((item) => item.id === tabId);
    if (tab) return { nodeId: node.id, tab };
  }
  return null;
}

export function locateLayoutTab(
  tabId: string,
  nodeId?: string,
  nodes?: LayoutTree,
): LocatedLayoutTab | null {
  const tree = readNodes(nodes);
  if (nodeId) {
    const tab = tree[nodeId]?.data?.tabs?.find((t) => t.id === tabId);
    return tab ? { nodeId, tab } : null;
  }
  return getLayoutTabById(tabId, tree);
}

export function getActiveLayoutTab(
  groupId: string,
  nodes?: LayoutTree,
): { activeTabId: string; tab: LayoutTab } | null {
  const tree = readNodes(nodes);
  const activeTabId = tree[groupId]?.data?.activeTabId;
  if (!activeTabId) return null;
  const tab = tree[groupId]?.data?.tabs?.find((item) => item.id === activeTabId);
  if (!tab) return null;
  return { activeTabId, tab };
}

export function getActiveLayoutTabAmongGroups(
  groupIds: string[],
  nodes?: LayoutTree,
): LayoutTab | null {
  const tree = readNodes(nodes);
  const uniqueGroupIds = Array.from(new Set(groupIds.filter(Boolean)));
  for (const groupId of uniqueGroupIds) {
    const active = getActiveLayoutTab(groupId, tree);
    if (active) return active.tab;
  }
  return null;
}

export function resolveEditorGroupId(
  groupId?: string | null,
  context?: LayoutGroupContext,
): string | null {
  const ctx = context ?? useLayoutStore.getState();
  return groupId ?? ctx.activeEditorGroupId ?? ctx.activeGroupId ?? null;
}
