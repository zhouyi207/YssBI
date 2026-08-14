import type { LayoutTab, LayoutTree } from '@/shared/types';
import { useLayoutStore } from './layoutStore';
import { useEditorTabStore } from './editorTabStore';
import { DEFAULT_EDITOR_GROUP_ID } from './workbenchLayoutDefaults';
import { isEditorGroupNode } from './layoutEditorGroupNode';

export type LocatedLayoutTab = { nodeId: string; tab: LayoutTab };

export interface LayoutGroupContext {
  activeEditorGroupId: string | null;
}

function readNodes(nodes?: LayoutTree): LayoutTree {
  return nodes ?? useLayoutStore.getState().nodes;
}

export { isEditorGroupNode } from './layoutEditorGroupNode';

/**
 * Resolve the editor group that should receive a new or activated tab.
 * Never returns fixed chrome nodes (sidebar / detail / panel).
 */
export function resolveEditorTargetGroupId(
  explicitGroupId?: string | null,
  nodes?: LayoutTree,
  context?: LayoutGroupContext,
): string {
  const tree = readNodes(nodes);
  const ctx = context ?? useLayoutStore.getState();

  const candidates = [
    explicitGroupId,
    ctx.activeEditorGroupId,
  ].filter((id): id is string => Boolean(id));

  for (const id of candidates) {
    if (isEditorGroupNode(tree[id])) return id;
  }

  const fallback = Object.values(tree).find(isEditorGroupNode);
  if (fallback) return fallback.id;

  return DEFAULT_EDITOR_GROUP_ID;
}

export function getLayoutTabById(tabId: string): LocatedLayoutTab | null {
  const located = useEditorTabStore.getState().locateTab(tabId);
  if (located) return { nodeId: located.groupId, tab: located.tab };
  return null;
}

export function locateLayoutTab(
  tabId: string,
  nodeId?: string,
  _nodes?: LayoutTree,
): LocatedLayoutTab | null {
  const located = useEditorTabStore.getState().locateTab(tabId, nodeId);
  return located ? { nodeId: located.groupId, tab: located.tab } : null;
}

export function getActiveLayoutTab(
  groupId: string,
  _nodes?: LayoutTree,
): { activeTabId: string; tab: LayoutTab } | null {
  const placement = useEditorTabStore.getState().getPlacement(groupId);
  const activeTabId = placement.activeTabId;
  if (!activeTabId) return null;
  const tab = useEditorTabStore.getState().resolveTab(activeTabId);
  if (!tab) return null;
  return { activeTabId, tab };
}

export function resolveEditorGroupId(
  groupId?: string | null,
  context?: LayoutGroupContext,
): string | null {
  const ctx = context ?? useLayoutStore.getState();
  return groupId ?? ctx.activeEditorGroupId ?? null;
}

function normalizeIds(ids: readonly string[]): string[] {
  return [...new Set(ids)];
}

function areStringArraysEqual(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

export interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}

export function createGraphSelection(
  nodeIds: readonly string[],
  connectionIds: readonly string[],
): GraphSelection {
  return {
    nodeIds: new Set(nodeIds),
    connectionIds: new Set(connectionIds),
  };
}

export function getEditorGroupGraphSelection(groupId: string): GraphSelection {
  const placement = useEditorTabStore.getState().getPlacement(groupId);
  return createGraphSelection(placement.selectedNodeIds, placement.selectedConnectionIds);
}

/** 更新编辑器组内画布选中节点（目标组由 `resolveEditorGroupId` 解析） */
export function updateEditorGroupSelectedNodeIds(
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string | null,
): void {
  const gid = resolveEditorGroupId(targetGroupId);
  if (!gid) return;

  const current = useEditorTabStore.getState().getPlacement(gid).selectedNodeIds;
  const next = normalizeIds(typeof updater === 'function' ? updater(current) : updater);
  const placement = useEditorTabStore.getState().getPlacement(gid);
  if (areStringArraysEqual(current, next) && placement.selectedConnectionIds.length === 0) return;
  useEditorTabStore.getState().setSelectedNodeIds(gid, next);
}

export function updateEditorGroupSelectedConnectionIds(
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string | null,
): void {
  const gid = resolveEditorGroupId(targetGroupId);
  if (!gid) return;

  const placement = useEditorTabStore.getState().getPlacement(gid);
  const current = placement.selectedConnectionIds;
  const next = normalizeIds(typeof updater === 'function' ? updater(current) : updater);
  if (areStringArraysEqual(current, next) && placement.selectedNodeIds.length === 0) return;
  useEditorTabStore.getState().setSelectedConnectionIds(gid, next);
}

export function clearEditorGroupGraphSelection(targetGroupId?: string | null): void {
  const gid = resolveEditorGroupId(targetGroupId);
  if (!gid) return;
  useEditorTabStore.getState().clearGraphSelection(gid);
}
