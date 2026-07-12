import type { LayoutNode, LayoutTree } from '@/shared/types/ui';
import { EDITOR_AREA_ID } from './workbenchLayoutDefaults';
import { isDescendantOf, readEditorAreaMaximizedGroupId } from './editorGridLayout';

function isSplitContainer(node: LayoutNode | undefined): node is LayoutNode & { type: 'row' | 'col' } {
  return node?.type === 'row' || node?.type === 'col';
}

function splitChildWeight(node: LayoutNode): number {
  if (node.data?.groupMaximizedHidden) return 0;
  if (node.data?.visible === false) return 0;
  if (node.pixelSize != null && node.pixelSize > 0) return node.pixelSize;
  if (node.size != null && node.size > 0) return node.size;
  return 1;
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

export function isEditorGridNode(nodes: LayoutTree, nodeId: string): boolean {
  return nodeId === EDITOR_AREA_ID || isDescendantOf(nodes, nodeId, EDITOR_AREA_ID);
}
