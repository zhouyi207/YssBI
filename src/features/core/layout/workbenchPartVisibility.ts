import type { LayoutNode } from '@/shared/types/ui';
import type { WorkbenchPartId } from './workbenchLayoutDefaults';

const USER_HIDDEN_PART_IDS = new Set<WorkbenchPartId>(['sidebar', 'panel', 'detail']);

export function isWorkbenchChromePart(nodeId: string): nodeId is WorkbenchPartId {
  return USER_HIDDEN_PART_IDS.has(nodeId as WorkbenchPartId);
}

/** User explicitly hid a workbench chrome part; sash auto-restore should not override. */
export function isWorkbenchPartUserHidden(node: LayoutNode | undefined): boolean {
  if (!node || !isWorkbenchChromePart(node.id)) return false;
  return node.data?.userHidden === true;
}

export function shouldRestoreWorkbenchPartOnSashDrag(node: LayoutNode | undefined): boolean {
  if (!node || node.data?.visible !== false) return false;
  return !isWorkbenchPartUserHidden(node);
}
