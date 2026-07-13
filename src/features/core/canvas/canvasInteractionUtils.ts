import type { RefObject } from 'react';
import { CONTEXT_MENU_MOVE_THRESHOLD_PX } from '@/app/appConfig/default';
import { getEditorGroupActiveTabId } from '@/features/core/layout/editorTabStore';
import { getViewport, editorViewportScope } from '@/features/core/viewport';
import type { EditorGesture } from '@/shared/types/ui';
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

export function getGestureScreenMovement(gesture: EditorGesture, scale = 1): boolean {
  if (!gesture) return false;

  if (gesture.type === 'pan') {
    const dx = gesture.lastX - gesture.startX;
    const dy = gesture.lastY - gesture.startY;
    return Math.sqrt(dx * dx + dy * dy) > CONTEXT_MENU_MOVE_THRESHOLD_PX;
  }

  if (gesture.type === 'drag' && gesture.dragDelta) {
    const screenDx = Math.abs(gesture.dragDelta.x * scale);
    const screenDy = Math.abs(gesture.dragDelta.y * scale);
    return screenDx > CONTEXT_MENU_MOVE_THRESHOLD_PX || screenDy > CONTEXT_MENU_MOVE_THRESHOLD_PX;
  }

  return false;
}
