import { getEditorGroupActiveTabId } from '@/features/core/layout/editorTabStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import {
  getActiveLayoutTab,
  resolveEditorGroupId,
} from '@/features/core/layout/layoutTabQueries';
import { useEditorStore } from '../stores/useEditorStore';
import { setVariablesGraphScopeFromResource } from './variablesGraphScope';
import type { DetailFocus } from './types';

export function focusDetail(focus: DetailFocus): void {
  useEditorStore.getState().setDetailFocus(focus);
  if (focus.kind === 'event' || focus.kind === 'function') {
    setVariablesGraphScopeFromResource(focus.path);
  }
}

export function focusDetailOnActiveGraph(groupId?: string | null): void {
  const layout = useLayoutStore.getState();
  const gid = resolveEditorGroupId(groupId, layout);
  if (!gid) return;

  const active = getActiveLayoutTab(gid, layout.nodes);
  if (active?.tab.type === 'event' || active?.tab.type === 'function') {
    focusDetail({ kind: active.tab.type, path: active.activeTabId });
  }
}

export function focusDetailOnNode(nodeId: string, groupId?: string | null): void {
  const layout = useLayoutStore.getState();
  const gid = resolveEditorGroupId(groupId, layout);
  if (!gid) return;

  const graphPath = getEditorGroupActiveTabId(gid);
  if (!graphPath) return;

  focusDetail({ kind: 'node', id: nodeId, graphPath });
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
