import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '../stores/useEditorStore';
import type { DetailFocus } from './types';

export function focusDetail(focus: DetailFocus): void {
  useEditorStore.getState().setDetailFocus(focus);
}

export function focusDetailOnActiveGraph(groupId?: string | null): void {
  const layout = useLayoutStore.getState();
  const gid = groupId ?? layout.activeEditorGroupId;
  if (!gid) return;

  const activeTabId = layout.nodes[gid]?.data?.activeTabId;
  if (!activeTabId) return;

  const tab = layout.nodes[gid]?.data?.tabs?.find((item) => item.id === activeTabId);
  if (tab?.type === 'event' || tab?.type === 'function') {
    focusDetail({ kind: tab.type, id: activeTabId });
  }
}

export function focusDetailOnNode(nodeId: string, groupId?: string | null): void {
  const layout = useLayoutStore.getState();
  const gid = groupId ?? layout.activeEditorGroupId;
  if (!gid) return;

  const graphId = layout.nodes[gid]?.data?.activeTabId;
  if (!graphId) return;

  focusDetail({ kind: 'node', id: nodeId, graphId });
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
