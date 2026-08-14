import type { LayoutTab } from '@/shared/types';
import {
  editorDockviewPort,
  getPaneSelection,
  useEditorPaneStateStore,
} from '@/features/core/dockview';
import { DEFAULT_EDITOR_GROUP_ID } from './workbenchLayoutDefaults';

export type LocatedLayoutTab = { nodeId: string; tab: LayoutTab };
export interface LayoutGroupContext { activeEditorGroupId: string | null }


function readTab(value: unknown): LayoutTab | null {
  return value && typeof value === 'object' ? value as LayoutTab : null;
}

function panelTab(panel: ReturnType<typeof editorDockviewPort.listPanels>[number]): LayoutTab | null {
  return readTab(panel.tab?.data?.layoutTab);
}

export function resolveEditorTargetGroupId(explicitGroupId?: string | null): string {
  const groups = editorDockviewPort.listGroups();
  const candidates = [explicitGroupId, editorDockviewPort.getActiveGroupId()];
  for (const id of candidates) {
    if (id && groups.some((group) => group.groupId === id)) return id;
  }
  return groups[0]?.groupId ?? DEFAULT_EDITOR_GROUP_ID;
}

export function getLayoutTabById(tabId: string): LocatedLayoutTab | null {
  return locateLayoutTab(tabId);
}

export function locateLayoutTab(tabId: string, nodeId?: string): LocatedLayoutTab | null {
  const panel = editorDockviewPort
    .findPanelsByResource(tabId)
    .find((candidate) => !nodeId || candidate.groupId === nodeId);
  const tab = panel ? panelTab(panel) : null;
  return panel && tab ? { nodeId: panel.groupId, tab } : null;
}

export function getActiveLayoutTab(groupId: string): { activeTabId: string; tab: LayoutTab } | null {
  const group = editorDockviewPort.listGroups().find((candidate) => candidate.groupId === groupId);
  const panel = editorDockviewPort
    .listPanels()
    .find((candidate) => candidate.panelInstanceId === group?.activePanelInstanceId);
  const tab = panel ? panelTab(panel) : null;
  return panel && tab ? { activeTabId: tab.id, tab } : null;
}

export function resolveEditorGroupId(groupId?: string | null, context?: LayoutGroupContext): string | null {
  return groupId ?? context?.activeEditorGroupId ?? editorDockviewPort.getActiveGroupId() ?? null;
}

export interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}

export function createGraphSelection(nodeIds: readonly string[], connectionIds: readonly string[]): GraphSelection {
  return { nodeIds: new Set(nodeIds), connectionIds: new Set(connectionIds) };
}

function activePanelInstanceId(groupId: string): string | undefined {
  return editorDockviewPort.listGroups().find((group) => group.groupId === groupId)?.activePanelInstanceId;
}

export function getEditorGroupGraphSelection(groupId: string): GraphSelection {
  const selection = getPaneSelection(activePanelInstanceId(groupId));
  return createGraphSelection(selection.selectedNodeIds, selection.selectedConnectionIds);
}

export function updateEditorGroupSelectedNodeIds(
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string | null,
): void {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (!panelId) return;
  const current = getPaneSelection(panelId).selectedNodeIds;
  useEditorPaneStateStore.getState().setSelectedNodeIds(
    panelId,
    typeof updater === 'function' ? updater(current) : updater,
  );
}

export function updateEditorGroupSelectedConnectionIds(
  updater: string[] | ((prev: string[]) => string[]),
  targetGroupId?: string | null,
): void {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (!panelId) return;
  const current = getPaneSelection(panelId).selectedConnectionIds;
  useEditorPaneStateStore.getState().setSelectedConnectionIds(
    panelId,
    typeof updater === 'function' ? updater(current) : updater,
  );
}

export function clearEditorGroupGraphSelection(targetGroupId?: string | null): void {
  const groupId = resolveEditorGroupId(targetGroupId);
  const panelId = groupId ? activePanelInstanceId(groupId) : undefined;
  if (panelId) useEditorPaneStateStore.getState().clearSelection(panelId);
}
