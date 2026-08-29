import { getPaneSelection, useEditorPaneStateStore } from '@/features/core/dockview/editorPaneStateStore';
import { layoutTabFromEditorMetadata } from '@/features/core/dockview/workbenchPanelModel';
import {
  workbenchDockviewRead,
  type WorkbenchPanelInfo,
} from '@/features/core/dockview/workbenchRead';
import type { LayoutTab } from '@/shared/types';

export type LocatedLayoutTab = { nodeId: string; tab: LayoutTab };
export interface LayoutGroupContext { activeEditorGroupId: string | null }

function panelTab(panel: WorkbenchPanelInfo | undefined): LayoutTab | null {
  return panel?.metadata.role === 'editor'
    ? layoutTabFromEditorMetadata(panel.metadata)
    : null;
}

function activeEditorPanelInGroup(groupId: string): WorkbenchPanelInfo | undefined {
  const activePanelInstanceId = workbenchDockviewRead
    .listGroups()
    .find((group) => group.groupId === groupId)
    ?.activePanelInstanceId;
  if (!activePanelInstanceId) return undefined;
  return workbenchDockviewRead
    .listGroupPanels(groupId)
    .find((panel) => panel.panelInstanceId === activePanelInstanceId
      && panel.metadata.role === 'editor');
}

export function resolveEditorTargetGroupId(explicitGroupId?: string | null): string {
  const groupIds = new Set(
    workbenchDockviewRead.listGroups().map((group) => group.groupId),
  );
  if (explicitGroupId && groupIds.has(explicitGroupId)) return explicitGroupId;
  const activeEditorGroupId = workbenchDockviewRead.getActiveEditorPanel()?.groupId;
  return activeEditorGroupId && groupIds.has(activeEditorGroupId)
    ? activeEditorGroupId
    : '';
}

export function locateLayoutTab(tabId: string, nodeId?: string): LocatedLayoutTab | null {
  const panel = workbenchDockviewRead
    .findEditorPanelsByResource(tabId)
    .find((candidate) => nodeId === undefined || candidate.groupId === nodeId);
  const tab = panelTab(panel);
  return panel && tab ? { nodeId: panel.groupId, tab } : null;
}

export function getActiveLayoutTab(groupId: string): { activeTabId: string; tab: LayoutTab } | null {
  const panel = activeEditorPanelInGroup(groupId);
  const tab = panelTab(panel);
  return panel && tab ? { activeTabId: tab.id, tab } : null;
}

export function resolveEditorGroupId(groupId?: string | null, context?: LayoutGroupContext): string | null {
  return groupId
    ?? context?.activeEditorGroupId
    ?? workbenchDockviewRead.getActiveEditorPanel()?.groupId
    ?? null;
}

export interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}

export function createGraphSelection(nodeIds: readonly string[], connectionIds: readonly string[]): GraphSelection {
  return { nodeIds: new Set(nodeIds), connectionIds: new Set(connectionIds) };
}

function activePanelInstanceId(groupId: string): string | undefined {
  return activeEditorPanelInGroup(groupId)?.panelInstanceId;
}

export function getEditorGroupGraphSelection(groupId: string): GraphSelection {
  const selection = getPaneSelection(activePanelInstanceId(groupId));
  return createGraphSelection(selection.selectedNodeIds, selection.selectedConnectionIds);
}

export interface UpdatedGraphNodeSelection {
  groupId: string;
  nodeIds: string[];
}

export function updateEditorGroupSelectedNodeIds(
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string | null,
): UpdatedGraphNodeSelection | null {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (!groupId || !panelId) return null;
  const current = getPaneSelection(panelId).selectedNodeIds;
  const nodeIds = typeof updater === 'function' ? updater(current) : updater;
  useEditorPaneStateStore.getState().setSelectedNodeIds(panelId, nodeIds);
  return { groupId, nodeIds: [...new Set(nodeIds)] };
}

export interface UpdatedGraphConnectionSelection {
  groupId: string;
  connectionIds: string[];
}

export function updateEditorGroupSelectedConnectionIds(
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string | null,
): UpdatedGraphConnectionSelection | null {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (!groupId || !panelId) return null;
  const current = getPaneSelection(panelId).selectedConnectionIds;
  const connectionIds = typeof updater === 'function' ? updater(current) : updater;
  useEditorPaneStateStore.getState().setSelectedConnectionIds(panelId, connectionIds);
  return { groupId, connectionIds: [...new Set(connectionIds)] };
}

export function clearEditorGroupGraphSelection(targetGroupId?: string | null): void {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (panelId) useEditorPaneStateStore.getState().clearSelection(panelId);
}
