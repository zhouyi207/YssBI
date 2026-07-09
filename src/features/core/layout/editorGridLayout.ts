import type { LayoutNode, LayoutTree } from '@/shared/types/ui';
import { EDITOR_AREA_ID, PANEL_PART_ID } from './workbenchLayoutDefaults';
import { isEditorGroupNode } from './layoutTabQueries';

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

export function equalSplitPairSizes(beforeSize: number, afterSize: number): { beforeSize: number; afterSize: number } {
  const total = beforeSize + afterSize;
  const half = Math.floor(total / 2);
  return { beforeSize: half, afterSize: total - half };
}

export function panelStartSizeFromNode(node: LayoutNode | undefined, domSize: number): number {
  if (!node || node.data?.visible === false) return 0;
  if (node.pixelSize != null) return node.pixelSize;
  return domSize;
}
