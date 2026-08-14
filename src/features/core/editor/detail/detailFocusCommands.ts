import { editorDockviewPort, type DockviewPanelInfo } from '@/features/core/dockview';
import type { LayoutTab } from '@/shared/types';
import { useEditorStore } from '../stores/useEditorStore';
import { setVariablesGraphScopeFromResource } from './variablesGraphScope';
import type { DetailFocus } from './types';

export function focusDetail(focus: DetailFocus): void {
  useEditorStore.getState().setDetailFocus(focus);
  if (focus.kind === 'event' || focus.kind === 'function') {
    setVariablesGraphScopeFromResource(focus.path);
  }
}

function activePanelForGroup(groupId?: string | null): DockviewPanelInfo | undefined {
  if (!groupId) return editorDockviewPort.getActivePanel();
  const activePanelInstanceId = editorDockviewPort
    .listGroups()
    .find((group: { groupId: string }) => group.groupId === groupId)
    ?.activePanelInstanceId;
  return editorDockviewPort
    .listPanels()
    .find((panel: DockviewPanelInfo) => panel.panelInstanceId === activePanelInstanceId);
}

function readLayoutTab(panel: DockviewPanelInfo | undefined): LayoutTab | null {
  const value = panel?.tab?.data?.layoutTab;
  return value && typeof value === 'object' ? value as LayoutTab : null;
}

export function focusDetailOnActiveGraph(groupId?: string | null): void {
  const tab = readLayoutTab(activePanelForGroup(groupId));
  if (tab?.type === 'event' || tab?.type === 'function') {
    focusDetail({ kind: tab.type, path: tab.id });
  }
}

export function focusDetailOnNode(nodeId: string, groupId?: string | null): void {
  const tab = readLayoutTab(activePanelForGroup(groupId));
  if (!tab) return;
  focusDetail({ kind: 'node', id: nodeId, graphPath: tab.id });
}

export function focusDetailOnNodeDefinition(nodeType: string): void {
  focusDetail({ kind: 'nodeDefinition', nodeType });
}

export type CanvasDetailGesture =
  | { type: 'blank-click'; groupId: string }
  | { type: 'box-select'; groupId: string; selectedIds: string[] }
  | { type: 'node-click'; groupId: string; nodeId: string };

/** Apply detail focus from a completed canvas gesture. Selection is handled elsewhere. */
export function applyCanvasDetailFocus(gesture: CanvasDetailGesture): void {
  switch (gesture.type) {
    case 'blank-click':
      focusDetailOnActiveGraph(gesture.groupId);
      break;
    case 'box-select':
      if (gesture.selectedIds.length === 1) {
        focusDetailOnNode(gesture.selectedIds[0], gesture.groupId);
      }
      break;
    case 'node-click':
      focusDetailOnNode(gesture.nodeId, gesture.groupId);
      break;
  }
}
