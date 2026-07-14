import type { EditorSplitEdge } from './editorSplitLayout';

export type EditorSplitDirection = Exclude<EditorSplitEdge, 'center'>;

export type EditorSplitHit =
  | { mode: 'merge' }
  | { mode: 'split'; edge: EditorSplitDirection };

export interface EditorSplitHitTestOptions {
  /** VS Code `openSideBySideDirection === 'right'` — prefer vertical splits. */
  preferSplitVertically?: boolean;
  enableSplitting?: boolean;
  isDraggingGroup?: boolean;
}

/**
 * VS Code `DropOverlay.positionOverlay` — dead-zone merge in the center,
 * directional split on edges using 10% inset + 33% tertiary zones.
 */
export function resolveEditorSplitHit(
  size: { width: number; height: number },
  offsetX: number,
  offsetY: number,
  options?: EditorSplitHitTestOptions,
): EditorSplitHit {
  const preferSplitVertically = options?.preferSplitVertically ?? true;
  const enableSplitting = options?.enableSplitting ?? true;
  const isDraggingGroup = options?.isDraggingGroup ?? false;

  const { width, height } = size;
  if (width <= 0 || height <= 0) return { mode: 'merge' };
  if (!enableSplitting) return { mode: 'merge' };

  let edgeWidthFactor: number;
  let edgeHeightFactor: number;
  if (isDraggingGroup) {
    edgeWidthFactor = preferSplitVertically ? 0.3 : 0.1;
    edgeHeightFactor = preferSplitVertically ? 0.1 : 0.3;
  } else {
    edgeWidthFactor = 0.1;
    edgeHeightFactor = 0.1;
  }

  const edgeWidthThreshold = width * edgeWidthFactor;
  const edgeHeightThreshold = height * edgeHeightFactor;
  const splitWidthThreshold = width / 3;
  const splitHeightThreshold = height / 3;

  if (
    offsetX > edgeWidthThreshold
    && offsetX < width - edgeWidthThreshold
    && offsetY > edgeHeightThreshold
    && offsetY < height - edgeHeightThreshold
  ) {
    return { mode: 'merge' };
  }

  if (preferSplitVertically) {
    if (offsetX < splitWidthThreshold) return { mode: 'split', edge: 'left' };
    if (offsetX > splitWidthThreshold * 2) return { mode: 'split', edge: 'right' };
    if (offsetY < height / 2) return { mode: 'split', edge: 'top' };
    return { mode: 'split', edge: 'bottom' };
  }

  if (offsetY < splitHeightThreshold) return { mode: 'split', edge: 'top' };
  if (offsetY > splitHeightThreshold * 2) return { mode: 'split', edge: 'bottom' };
  if (offsetX < width / 2) return { mode: 'split', edge: 'left' };
  return { mode: 'split', edge: 'right' };
}

export function resolveEditorSplitHitFromClientPoint(
  element: Element,
  clientX: number,
  clientY: number,
  options?: EditorSplitHitTestOptions,
): EditorSplitHit | null {
  const rect = element.getBoundingClientRect();
  const offsetX = clientX - rect.left;
  const offsetY = clientY - rect.top;
  if (offsetX < 0 || offsetY < 0 || offsetX > rect.width || offsetY > rect.height) {
    return null;
  }
  return resolveEditorSplitHit(
    { width: rect.width, height: rect.height },
    offsetX,
    offsetY,
    options,
  );
}
