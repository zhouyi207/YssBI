import type { EditorSplitEdge } from './editorSplitLayout';
import { resolveEditorSplitHit, type EditorSplitHit } from './editorSplitHitTest';
import { readEditorGroupDropBounds } from './editorDropTarget';

export interface EditorDropPreviewRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

/** Half-editor highlight region for split drop preview (viewport coordinates). */
export function computeEditorSplitPreviewRect(
  nodeRect: DOMRect,
  edge: EditorSplitEdge,
): EditorDropPreviewRect {
  if (edge === 'left') {
    return {
      top: nodeRect.top,
      left: nodeRect.left,
      width: nodeRect.width / 2,
      height: nodeRect.height,
    };
  }

  if (edge === 'right') {
    return {
      top: nodeRect.top,
      left: nodeRect.left + nodeRect.width / 2,
      width: nodeRect.width / 2,
      height: nodeRect.height,
    };
  }

  if (edge === 'top') {
    return {
      top: nodeRect.top,
      left: nodeRect.left,
      width: nodeRect.width,
      height: nodeRect.height / 2,
    };
  }

  if (edge === 'bottom') {
    return {
      top: nodeRect.top + nodeRect.height / 2,
      left: nodeRect.left,
      width: nodeRect.width,
      height: nodeRect.height / 2,
    };
  }

  return {
    top: nodeRect.top,
    left: nodeRect.left,
    width: nodeRect.width,
    height: nodeRect.height,
  };
}

export function readEditorGroupContentRect(targetGroupId: string): DOMRect | null {
  return readEditorGroupDropBounds(targetGroupId);
}

export function resolveEditorDropHitAtClientPoint(
  targetGroupId: string,
  clientX: number,
  clientY: number,
  options?: Parameters<typeof resolveEditorSplitHit>[3],
): { hit: EditorSplitHit; rect: EditorDropPreviewRect } | null {
  const bounds = readEditorGroupDropBounds(targetGroupId);
  if (!bounds) return null;

  const offsetX = clientX - bounds.left;
  const offsetY = clientY - bounds.top;
  if (offsetX < 0 || offsetY < 0 || offsetX > bounds.width || offsetY > bounds.height) {
    return null;
  }

  const hit = resolveEditorSplitHit(
    { width: bounds.width, height: bounds.height },
    offsetX,
    offsetY,
    options,
  );

  const edge = hit.mode === 'split' ? hit.edge : 'center';
  return {
    hit,
    rect: computeEditorSplitPreviewRect(bounds, edge),
  };
}

export function readEditorSplitPreviewRect(
  targetGroupId: string,
  edge: EditorSplitEdge,
): EditorDropPreviewRect | null {
  const bounds = readEditorGroupDropBounds(targetGroupId);
  if (!bounds) return null;
  return computeEditorSplitPreviewRect(bounds, edge);
}
