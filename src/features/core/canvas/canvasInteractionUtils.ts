import type { RefObject } from 'react';

import { getEditorGroupActiveTabId } from '@/features/core/layout/editorTabStore';
import { getViewport, editorViewportScope } from '@/features/core/viewport';

import { queryCanvasElement } from './selectionHitTargets';

export function resolveTabId(groupId: string, activeTabIdRef: RefObject<string | null>): string | null {
  return getEditorGroupActiveTabId(groupId) ?? activeTabIdRef.current ?? null;
}

export function getCanvasWorldPoint(
  groupId: string,
  graphPath: string | null,
  clientX: number,
  clientY: number,
) {
  const canvasEl = queryCanvasElement(groupId);
  if (!canvasEl) {
    return { x: clientX, y: clientY };
  }

  const rect = canvasEl.getBoundingClientRect();
  const viewport = graphPath
    ? getViewport(editorViewportScope(groupId, graphPath))
    : { x: 0, y: 0, scale: 1 };
  return {
    x: (clientX - rect.left - viewport.x) / viewport.scale,
    y: (clientY - rect.top - viewport.y) / viewport.scale,
  };
}
