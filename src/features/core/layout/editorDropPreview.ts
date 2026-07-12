import type { EditorSplitEdge } from './editorSplitLayout';
import { resolveEditorSplitPlacement } from './editorSplitLayout';

export interface EditorDropPreviewRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

function editorContentElement(targetGroupId: string): Element | null {
  return document.querySelector(`[data-editor-content="${targetGroupId}"]`)
    ?? document.getElementById(`layout-node-${targetGroupId}`);
}

/** Half-editor highlight region for split drop preview (viewport coordinates). */
export function computeEditorSplitPreviewRect(
  nodeRect: DOMRect,
  edge: EditorSplitEdge,
): EditorDropPreviewRect {
  const resolved = edge === 'center' ? 'right' : edge;

  if (resolved === 'left') {
    return {
      top: nodeRect.top,
      left: nodeRect.left,
      width: nodeRect.width / 2,
      height: nodeRect.height,
    };
  }

  if (resolved === 'right') {
    return {
      top: nodeRect.top,
      left: nodeRect.left + nodeRect.width / 2,
      width: nodeRect.width / 2,
      height: nodeRect.height,
    };
  }

  if (resolved === 'top') {
    return {
      top: nodeRect.top,
      left: nodeRect.left,
      width: nodeRect.width,
      height: nodeRect.height / 2,
    };
  }

  return {
    top: nodeRect.top + nodeRect.height / 2,
    left: nodeRect.left,
    width: nodeRect.width,
    height: nodeRect.height / 2,
  };
}

export function readEditorSplitPreviewRect(
  targetGroupId: string,
  edge: EditorSplitEdge,
): EditorDropPreviewRect | null {
  const element = editorContentElement(targetGroupId);
  if (!element) return null;
  return computeEditorSplitPreviewRect(element.getBoundingClientRect(), edge);
}

/** Full canvas / watermark drop target for sidebar graph open. */
export function readEditorCanvasDropRect(targetGroupId: string): EditorDropPreviewRect | null {
  const element = editorContentElement(targetGroupId);
  if (!element) return null;
  const rect = element.getBoundingClientRect();
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}

/** For tests / docs — edge resolves to placement used by layout store. */
export function editorSplitPreviewPlacement(edge: EditorSplitEdge) {
  return resolveEditorSplitPlacement(edge);
}
