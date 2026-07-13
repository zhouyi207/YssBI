import type { LayoutNode, LayoutTree } from '@/shared/types/ui';
import { commitSplitPairSizes } from './editorGridSizing';
import { EDITOR_AREA_ID, PANEL_PART_ID } from './workbenchLayoutDefaults';
import { isEditorGroupNode } from './layoutEditorGroupNode';
import { isEditorGroupPlacementEmpty } from './editorTabStore';
import {
  createEditorGroupId,
  resolveEditorSplitPlacement,
  type EditorSplitEdge,
} from './editorSplitLayout';

export const EDITOR_GROUP_MIN_WIDTH = 200;
export const EDITOR_GROUP_MIN_HEIGHT = 120;

export function resolveEditorGroupMinSize(
  node: LayoutNode | undefined,
  orientation: 'row' | 'col',
): number {
  return node?.minSize ?? (orientation === 'row' ? EDITOR_GROUP_MIN_WIDTH : EDITOR_GROUP_MIN_HEIGHT);
}

/**
 * Editor Grid domain — VS Code `GridWidget` + `SerializableGrid` equivalent.
 *
 * YssBI models the editor part as a nested row/col tree under `editor_area` (not imperative DOM).
 * - Tree mutations: this module
 * - Persistence: `editorGridMemento`
 * - Facade API: `EditorGroupsService`
 * - Render + sash: `LayoutNodeRenderer` + `sashResizeLogic`
 *
 * A separate imperative GridWidget class is intentionally not introduced.
 */

export function isDescendantOf(nodes: LayoutTree, nodeId: string, ancestorId: string): boolean {
  let current: string | null = nodeId;
  while (current) {
    if (current === ancestorId) return true;
    current = nodes[current]?.parentId ?? null;
  }
  return false;
}

/** True for sashes between views inside the editor part (not workbench chrome). */
export function isEditorGridSash(
  beforeNodeId: string,
  afterNodeId: string,
  nodes: LayoutTree,
): boolean {
  if (beforeNodeId === PANEL_PART_ID || afterNodeId === PANEL_PART_ID) return false;
  return isDescendantOf(nodes, beforeNodeId, EDITOR_AREA_ID)
    && isDescendantOf(nodes, afterNodeId, EDITOR_AREA_ID);
}

export function listEditorGroupIds(nodes: LayoutTree): string[] {
  return Object.values(nodes)
    .filter(isEditorGroupNode)
    .map((node) => node.id);
}

export function snapshotEditorGridPixelSizes(nodes: LayoutTree): Record<string, number> {
  const sizes: Record<string, number> = {};
  if (!nodes[EDITOR_AREA_ID]) return sizes;

  const visit = (id: string) => {
    const node = nodes[id];
    if (!node) return;
    if (node.pixelSize != null) sizes[id] = node.pixelSize;
    node.children?.forEach(visit);
  };
  visit(EDITOR_AREA_ID);
  return sizes;
}

export function applyEditorGridPixelSizes(
  nodes: LayoutTree,
  sizes: Record<string, number>,
): void {
  for (const [id, pixelSize] of Object.entries(sizes)) {
    const node = nodes[id];
    if (node) node.pixelSize = pixelSize;
  }
}

export function setEditorGroupMaximizedHidden(nodes: LayoutTree, hiddenGroupId: string, hidden: boolean): void {
  const node = nodes[hiddenGroupId];
  if (!node?.data) return;
  node.data = { ...node.data, groupMaximizedHidden: hidden };
}

export function clearEditorGroupMaximizedHidden(nodes: LayoutTree): void {
  for (const groupId of listEditorGroupIds(nodes)) {
    const node = nodes[groupId];
    if (node?.data?.groupMaximizedHidden) {
      node.data = { ...node.data, groupMaximizedHidden: false };
    }
  }
}

export function readEditorAreaMaximizedGroupId(nodes: LayoutTree): string | null {
  const value = nodes[EDITOR_AREA_ID]?.data?.maximizedGroupId;
  return typeof value === 'string' ? value : null;
}

export function readEditorAreaRestoredGridSizes(nodes: LayoutTree): Record<string, number> | null {
  const raw = nodes[EDITOR_AREA_ID]?.data?.restoredGridSizes;
  if (!raw || typeof raw !== 'object') return null;
  return raw as Record<string, number>;
}

export function writeEditorAreaMaximizeState(
  nodes: LayoutTree,
  maximizedGroupId: string | null,
  restoredGridSizes: Record<string, number> | null,
): void {
  const editorArea = nodes[EDITOR_AREA_ID];
  if (!editorArea) return;
  editorArea.data = {
    ...editorArea.data,
    maximizedGroupId: maximizedGroupId ?? undefined,
    restoredGridSizes: restoredGridSizes ?? undefined,
  };
}

/** Drop stale exit snapshot while a group stays maximized (chrome/viewport changed). */
export function invalidateEditorAreaMaximizeSnapshot(nodes: LayoutTree): void {
  const maximizedId = readEditorAreaMaximizedGroupId(nodes);
  if (!maximizedId) return;

  const maximized = nodes[maximizedId];
  if (maximized?.pixelSize != null) {
    maximized.pixelSize = undefined;
  }

  writeEditorAreaMaximizeState(nodes, maximizedId, null);
}

export function equalSplitPairSizes(beforeSize: number, afterSize: number): { beforeSize: number; afterSize: number } {
  const total = beforeSize + afterSize;
  const half = Math.floor(total / 2);
  return { beforeSize: half, afterSize: total - half };
}

function mergeRemovedGroupPixelSize(
  sibling: LayoutNode,
  removed: LayoutNode,
): void {
  if (sibling.pixelSize != null && removed.pixelSize != null) {
    sibling.pixelSize += removed.pixelSize;
    return;
  }
  if (sibling.pixelSize == null && removed.pixelSize != null) {
    sibling.pixelSize = removed.pixelSize;
    return;
  }
  if (sibling.pixelSize != null && removed.pixelSize == null) {
    return;
  }
  sibling.pixelSize = undefined;
}

/** Apply equal split sizes to two grid siblings (sash double-click). */
export function applyEqualGridSplit(
  nodes: LayoutTree,
  beforeId: string,
  afterId: string,
  beforeSize: number,
  afterSize: number,
): void {
  const { beforeSize: nextBefore, afterSize: nextAfter } = equalSplitPairSizes(beforeSize, afterSize);
  commitSplitPairSizes(nodes, beforeId, afterId, nextBefore, nextAfter);
}

export function firstEditorGroupId(nodes: LayoutTree): string | null {
  return listEditorGroupIds(nodes)[0] ?? null;
}

export function isActiveEditorGroupValid(nodes: LayoutTree, groupId: string | null | undefined): boolean {
  return groupId != null && isEditorGroupNode(nodes[groupId]);
}

/**
 * GridWidget.removeView equivalent — drop an empty editor group and collapse redundant branches.
 * Returns whether the group was removed and the next group to focus.
 */
export function removeEditorGroupFromTree(
  nodes: LayoutTree,
  groupId: string,
): { removed: boolean; nextActiveGroupId: string | null } {
  const group = nodes[groupId];
  if (!isEditorGroupNode(group)) {
    return { removed: false, nextActiveGroupId: groupId };
  }
  if (!isEditorGroupPlacementEmpty(groupId)) {
    return { removed: false, nextActiveGroupId: groupId };
  }
  if (listEditorGroupIds(nodes).length <= 1) {
    return { removed: false, nextActiveGroupId: groupId };
  }

  const parent = group.parentId ? nodes[group.parentId] : undefined;
  if (!parent?.children) {
    return { removed: false, nextActiveGroupId: groupId };
  }

  const removedIndex = parent.children.indexOf(groupId);
  const siblingId = parent.children[removedIndex - 1] ?? parent.children[removedIndex + 1] ?? null;
  const sibling = siblingId ? nodes[siblingId] : undefined;
  parent.children.splice(removedIndex, 1);

  if (parent.id === EDITOR_AREA_ID) {
    if (sibling) {
      mergeRemovedGroupPixelSize(sibling, group);
    }
  } else if (parent.children.length === 1 && parent.parentId) {
    const grandParent = nodes[parent.parentId];
    if (grandParent?.children) {
      const singleChildId = parent.children[0];
      const singleChild = nodes[singleChildId];
      if (singleChild) {
        const parentIndex = grandParent.children.indexOf(parent.id);
        grandParent.children[parentIndex] = singleChildId;
        singleChild.parentId = grandParent.id;
        singleChild.size = parent.size ?? 1;
        mergeRemovedGroupPixelSize(singleChild, group);
        delete nodes[parent.id];
      }
    }
  } else if (sibling) {
    mergeRemovedGroupPixelSize(sibling, group);
  } else if (parent.children.length === 0 && parent.parentId) {
    const grandParent = nodes[parent.parentId];
    if (grandParent?.children) {
      grandParent.children = grandParent.children.filter((cid) => cid !== parent.id);
      delete nodes[parent.id];
    }
  }

  delete nodes[groupId];

  reconcileEditorGridAfterGroupRemoved(nodes, groupId);

  return {
    removed: true,
    nextActiveGroupId: sibling && isEditorGroupNode(sibling) ? sibling.id : firstEditorGroupId(nodes),
  };
}

/**
 * After removing an editor group, clear stale maximize/hidden flags so the remaining
 * grid fills the editor area and leaf groups render content (watermark / canvas).
 */
export function reconcileEditorGridAfterGroupRemoved(
  nodes: LayoutTree,
  removedGroupId: string,
): void {
  const maximizedId = readEditorAreaMaximizedGroupId(nodes);
  const maximizedStale =
    maximizedId === removedGroupId || (maximizedId != null && !nodes[maximizedId]);
  const remaining = listEditorGroupIds(nodes);

  if (maximizedStale) {
    const restored = readEditorAreaRestoredGridSizes(nodes);
    clearEditorGroupMaximizedHidden(nodes);
    if (restored) applyEditorGridPixelSizes(nodes, restored);
    writeEditorAreaMaximizeState(nodes, null, null);
    return;
  }

  const orphanedHidden = remaining.some((id) => nodes[id]?.data?.groupMaximizedHidden);
  if (orphanedHidden) {
    clearEditorGroupMaximizedHidden(nodes);
    writeEditorAreaMaximizeState(nodes, null, null);
  }
}

export interface SplitEditorGroupPayload {
  component: string;
}

/** GridWidget.addView equivalent — fork a new editor group at a dock edge. */
export function splitEditorGroupInTree(
  nodes: LayoutTree,
  targetGroupId: string,
  edge: EditorSplitEdge,
  payload: SplitEditorGroupPayload,
): string | null {
  const { direction, isAfter } = resolveEditorSplitPlacement(edge);
  const targetNode = nodes[targetGroupId];
  if (!targetNode?.parentId) return null;

  const parentNode = nodes[targetNode.parentId];
  if (!parentNode) return null;

  const newNodeId = createEditorGroupId();
  const newNode: LayoutNode = {
    id: newNodeId,
    type: 'component',
    parentId: parentNode.id,
    children: [],
    size: 1,
    data: {
      component: payload.component,
    },
  };

  if (parentNode.type === direction) {
    const targetIndex = parentNode.children?.indexOf(targetGroupId) ?? 0;
    const insertIndex = isAfter ? targetIndex + 1 : targetIndex;
    parentNode.children?.splice(insertIndex, 0, newNodeId);
    nodes[newNodeId] = newNode;
    return newNodeId;
  }

  const branchId = createEditorGroupId();
  const branch: LayoutNode = {
    id: branchId,
    type: direction,
    parentId: parentNode.id,
    children: isAfter ? [targetGroupId, newNodeId] : [newNodeId, targetGroupId],
    size: targetNode.size ?? 1,
  };

  const targetIndex = parentNode.children?.indexOf(targetGroupId) ?? 0;
  parentNode.children![targetIndex] = branchId;

  targetNode.parentId = branchId;
  targetNode.size = 1;
  targetNode.pixelSize = undefined;

  newNode.parentId = branchId;
  nodes[newNodeId] = newNode;
  nodes[branchId] = branch;
  return newNodeId;
}

export function panelStartSizeFromNode(node: LayoutNode | undefined, domSize: number): number {
  if (!node || node.data?.visible === false) return 0;
  if (node.pixelSize != null) return node.pixelSize;
  return domSize;
}
