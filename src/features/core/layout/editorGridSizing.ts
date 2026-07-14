import type { LayoutNode, LayoutTree } from '@/shared/types/ui';
import { EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import type { EditorSplitSizingMode } from './editorPartOptions';
import {
  isDescendantOf,
  invalidateEditorAreaMaximizeSnapshot,
  readEditorAreaMaximizedGroupId,
} from './editorGridLayout';

function isSplitContainer(node: LayoutNode | undefined): node is LayoutNode & { type: 'row' | 'col' } {
  return node?.type === 'row' || node?.type === 'col';
}

function isSplitChildVisible(node: LayoutNode): boolean {
  if (node.data?.groupMaximizedHidden) return false;
  if (node.data?.visible === false) return false;
  return true;
}

function splitChildWeight(node: LayoutNode): number {
  if (!isSplitChildVisible(node)) return 0;
  if (node.pixelSize != null && node.pixelSize > 0) return node.pixelSize;
  if (node.size != null && node.size > 0) return node.size;
  return 1;
}

/** VS Code `SplitView.areViewsDistributed` pixel tolerance. */
const DISTRIBUTED_SIZE_TOLERANCE_PX = 2;

/**
 * Layout sizes along the split axis for siblings in one row/col parent.
 * Uses committed `pixelSize` when present; otherwise scales flex ratios to synthetic
 * pixels so `max - min <= 2` matches VS Code for ratio-only groups.
 */
function resolveSplitChildLayoutSizes(
  nodes: LayoutTree,
  parent: LayoutNode,
  excludeChildIds?: ReadonlySet<string>,
): number[] {
  const children = (parent.children ?? [])
    .map((id) => nodes[id])
    .filter(
      (node): node is LayoutNode =>
        Boolean(node && isSplitChildVisible(node) && !excludeChildIds?.has(node.id)),
    );
  if (children.length === 0) return [];

  const hasPixelSize = children.some((child) => child.pixelSize != null && child.pixelSize > 0);
  if (hasPixelSize) {
    return children.map(splitChildWeight);
  }

  const ratioScale = 10_000;
  return children.map((child) => (child.size ?? 1) * ratioScale);
}

/** VS Code `areViewsDistributed` — same-axis siblings within 2px of each other. */
function areSplitParentChildrenDistributed(
  nodes: LayoutTree,
  parent: LayoutNode,
  options?: { excludeChildIds?: Iterable<string> },
): boolean {
  const excludeChildIds = options?.excludeChildIds
    ? new Set(options.excludeChildIds)
    : undefined;
  const sizes = resolveSplitChildLayoutSizes(nodes, parent, excludeChildIds);
  if (sizes.length < 2) return true;

  const min = Math.min(...sizes);
  const max = Math.max(...sizes);
  return max - min <= DISTRIBUTED_SIZE_TOLERANCE_PX;
}

/** Row/col containers under `editor_area` that host 2+ split children. */
export function listEditorGridSplitParentIds(nodes: LayoutTree): string[] {
  const ids: string[] = [];
  const visit = (nodeId: string) => {
    const node = nodes[nodeId];
    if (!node) return;
    if (isSplitContainer(node) && (node.children?.length ?? 0) >= 2) {
      ids.push(nodeId);
    }
    node.children?.forEach(visit);
  };
  if (nodes[EDITOR_AREA_ID]) visit(EDITOR_AREA_ID);
  return ids;
}

/** Commit sash drag sizes: runtime pixels + viewport-independent flex weights. */
export function commitSplitPairSizes(
  nodes: LayoutTree,
  beforeId: string,
  afterId: string,
  beforePx: number,
  afterPx: number,
): void {
  const total = beforePx + afterPx;
  if (total <= 0) return;

  const before = nodes[beforeId];
  const after = nodes[afterId];
  if (before) {
    before.pixelSize = beforePx;
    before.size = beforePx / total;
  }
  if (after) {
    after.pixelSize = afterPx;
    after.size = afterPx / total;
  }
}

function setSplitChildWeight(node: LayoutNode, weight: number): void {
  node.size = weight;
  node.pixelSize = undefined;
}

function distributeSplitParentChildren(nodes: LayoutTree, parent: LayoutNode): void {
  const childIds = parent.children ?? [];
  if (childIds.length < 2) return;
  const weight = 1 / childIds.length;
  for (const childId of childIds) {
    const child = nodes[childId];
    if (child) setSplitChildWeight(child, weight);
  }
}

function applyHalveTargetSizing(
  nodes: LayoutTree,
  targetId: string,
  newId: string,
  mode: 'same-axis' | 'perpendicular',
): void {
  const target = nodes[targetId];
  const newNode = nodes[newId];
  if (!target || !newNode) return;

  if (mode === 'same-axis') {
    const half = splitChildWeight(target) / 2;
    setSplitChildWeight(target, half);
    setSplitChildWeight(newNode, half);
  } else {
    setSplitChildWeight(target, 0.5);
    setSplitChildWeight(newNode, 0.5);
  }
}

/**
 * VS Code GridWidget.addView sizing — halve the target group's allocation for the new view.
 * Same-axis: target weight is split in half; siblings keep their weights.
 * Perpendicular: the new branch splits 50/50 on the perpendicular axis.
 */
export function applyEditorGridAddViewSizing(
  nodes: LayoutTree,
  splitParentId: string,
  targetId: string,
  newId: string,
  mode: 'same-axis' | 'perpendicular',
  splitSizing: EditorSplitSizingMode = 'auto',
): void {
  if (readEditorAreaMaximizedGroupId(nodes)) return;

  const target = nodes[targetId];
  const newNode = nodes[newId];
  const parent = nodes[splitParentId];
  if (!target || !newNode || !parent?.children?.includes(targetId) || !parent.children.includes(newId)) {
    return;
  }

  if (splitSizing === 'distribute') {
    distributeSplitParentChildren(nodes, parent);
  } else if (splitSizing === 'split') {
    applyHalveTargetSizing(nodes, targetId, newId, mode);
  } else if (
    areSplitParentChildrenDistributed(nodes, parent, { excludeChildIds: [newId] })
  ) {
    distributeSplitParentChildren(nodes, parent);
  } else {
    applyHalveTargetSizing(nodes, targetId, newId, mode);
  }

  reflowEditorGridLayout(nodes);
}

/**
 * VS Code GridWidget.removeView sizing — merge removed allocation into a reference sibling,
 * or redistribute remaining children when views were already evenly distributed.
 */
export function applyEditorGridRemoveViewSizing(
  nodes: LayoutTree,
  parentId: string,
  removedId: string,
  splitSizing: EditorSplitSizingMode = 'auto',
): void {
  if (readEditorAreaMaximizedGroupId(nodes)) return;

  const parent = nodes[parentId];
  const removed = nodes[removedId];
  const childIds = parent?.children ?? [];
  const removedIndex = childIds.indexOf(removedId);
  if (!parent || !removed || removedIndex < 0 || childIds.length < 2) return;

  const shouldDistribute =
    splitSizing === 'distribute' ||
    (splitSizing === 'auto' && areSplitParentChildrenDistributed(nodes, parent));

  if (shouldDistribute) {
    return;
  }

  const referenceIndex = removedIndex === 0 ? removedIndex + 1 : removedIndex - 1;
  const referenceId = childIds[referenceIndex];
  const reference = referenceId ? nodes[referenceId] : undefined;
  if (!reference) return;

  const removedWeight = splitChildWeight(removed);
  const referenceWeight = splitChildWeight(reference);
  setSplitChildWeight(reference, referenceWeight + removedWeight);
}

/** Equal weights for every child under a split parent (after a group was removed). */
export function distributeSplitParentRemainingChildren(nodes: LayoutTree, parentId: string): void {
  const parent = nodes[parentId];
  if (!parent) return;
  distributeSplitParentChildren(nodes, parent);
}

/** VS Code `SplitView.areViewsDistributed` for one editor-grid row/col parent. */
export function areEditorGridSplitChildrenDistributed(nodes: LayoutTree, parentId: string): boolean {
  const parent = nodes[parentId];
  if (!parent) return true;
  return areSplitParentChildrenDistributed(nodes, parent);
}

export function shouldDistributeAfterEditorGridRemove(
  nodes: LayoutTree,
  parent: LayoutNode,
  splitSizing: EditorSplitSizingMode,
): boolean {
  return (
    splitSizing === 'distribute' ||
    (splitSizing === 'auto' && areSplitParentChildrenDistributed(nodes, parent))
  );
}

/**
 * Drop fixed `pixelSize` on lone children so they can grow after a sibling group closes.
 * Walks the editor grid from `editor_area`; only applies when a parent has exactly one child.
 */
export function clearEditorGridSingletonPixelLocks(nodes: LayoutTree): void {
  if (readEditorAreaMaximizedGroupId(nodes)) return;

  const relaxLoneChild = (parentId: string): void => {
    const parent = nodes[parentId];
    const childIds = parent?.children ?? [];
    if (childIds.length !== 1) {
      childIds.forEach(relaxLoneChild);
      return;
    }

    const child = nodes[childIds[0]];
    if (!child) return;

    child.pixelSize = undefined;
    child.size = 1;
    relaxLoneChild(childIds[0]);
  };

  if (nodes[EDITOR_AREA_ID]) relaxLoneChild(EDITOR_AREA_ID);
}

/**
 * Normalize every editor-grid split to ratio-only `size` weights.
 * Skipped while a group is maximized (pixel snapshot lives on editor_area.data).
 */
export function normalizeEditorGridSplitWeights(nodes: LayoutTree): void {
  if (readEditorAreaMaximizedGroupId(nodes)) return;

  for (const parentId of listEditorGridSplitParentIds(nodes)) {
    const parent = nodes[parentId];
    if (!parent?.children?.length) continue;

    const children = parent.children
      .map((id) => nodes[id])
      .filter((node): node is LayoutNode => Boolean(node));
    if (children.length < 2) continue;

    const weights = children.map(splitChildWeight);
    const total = weights.reduce((sum, weight) => sum + weight, 0);
    if (total <= 0) continue;

    for (let i = 0; i < children.length; i++) {
      children[i].size = weights[i] / total;
      children[i].pixelSize = undefined;
    }
  }
}

/** Viewport-independent weights for memento snapshot (does not mutate live nodes). */
export function computeEditorGridMementoSizes(nodes: LayoutTree): Record<string, number> {
  const sizes: Record<string, number> = {};

  for (const parentId of listEditorGridSplitParentIds(nodes)) {
    const parent = nodes[parentId];
    if (!parent?.children?.length) continue;

    const children = parent.children
      .map((id) => nodes[id])
      .filter((node): node is LayoutNode => Boolean(node));
    if (children.length < 2) continue;

    const weights = children.map(splitChildWeight);
    const total = weights.reduce((sum, weight) => sum + weight, 0);
    if (total <= 0) continue;

    for (let i = 0; i < children.length; i++) {
      sizes[children[i].id] = weights[i] / total;
    }
  }

  return sizes;
}

/** Reclaim editor grid space after chrome, viewport, or group-count changes. */
export function reflowEditorGridLayout(nodes: LayoutTree): void {
  if (readEditorAreaMaximizedGroupId(nodes)) {
    invalidateEditorAreaMaximizeSnapshot(nodes);
    return;
  }
  clearEditorGridSingletonPixelLocks(nodes);
  normalizeEditorGridSplitWeights(nodes);
}
export function isEditorGridNode(nodes: LayoutTree, nodeId: string): boolean {
  return nodeId === EDITOR_AREA_ID || isDescendantOf(nodes, nodeId, EDITOR_AREA_ID);
}
