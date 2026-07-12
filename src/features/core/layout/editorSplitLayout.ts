import type { LayoutDirection } from '@/shared/types/ui';

/** Drop edge on an editor group canvas — `center` maps to VS Code default (split right). */
export type EditorSplitEdge = 'left' | 'right' | 'top' | 'bottom' | 'center';

export interface EditorSplitPlacement {
  direction: LayoutDirection;
  /** Insert the new group after the target along `direction` (right / bottom). */
  isAfter: boolean;
}

/** Map a dock edge to row/col branch direction and insert order. */
export function resolveEditorSplitPlacement(edge: EditorSplitEdge): EditorSplitPlacement {
  const resolved = edge === 'center' ? 'right' : edge;
  return {
    direction: resolved === 'left' || resolved === 'right' ? 'row' : 'col',
    isAfter: resolved === 'right' || resolved === 'bottom',
  };
}

export function createEditorGroupId(): string {
  return Math.random().toString(36).slice(2, 11);
}
