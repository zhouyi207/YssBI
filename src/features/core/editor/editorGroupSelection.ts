import { getPaneSelection, useEditorPaneStateStore } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";

export interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}

export function createGraphSelection(
  nodeIds: readonly string[],
  connectionIds: readonly string[],
): GraphSelection {
  return { nodeIds: new Set(nodeIds), connectionIds: new Set(connectionIds) };
}

function resolveEditorGroupId(groupId?: string | null): string | null {
  return groupId ?? workbenchDockviewRead.getActiveEditorPanel()?.groupId ?? null;
}

function activePanelInstanceId(groupId: string): string | undefined {
  return workbenchDockviewRead.getActiveEditorPanelInGroup(groupId)?.panelInstanceId;
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
  const nodeIds = typeof updater === "function" ? updater(current) : updater;
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
  const connectionIds = typeof updater === "function" ? updater(current) : updater;
  useEditorPaneStateStore.getState().setSelectedConnectionIds(panelId, connectionIds);
  return { groupId, connectionIds: [...new Set(connectionIds)] };
}

export function clearEditorGroupGraphSelection(targetGroupId?: string | null): void {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (panelId) useEditorPaneStateStore.getState().clearSelection(panelId);
}
