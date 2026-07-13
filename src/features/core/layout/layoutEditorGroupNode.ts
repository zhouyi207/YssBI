import type { LayoutNode } from '@/shared/types';

/** Non-fixed component nodes that host editor tabs (not sidebar / detail / panel). */
export function isEditorGroupNode(node: LayoutNode | undefined): boolean {
  return node?.type === 'component' && !node.data?.isFixed;
}
