import type { GraphResourceDragData } from '@/features/core/dnd';
import { canDropFunctionIntoEventGraph } from '@/features/application/editor/canvasDrop';
import type { EditorDropPreviewRect } from '@/features/core/layout/editorDropPreview';
import type { EditorSplitHit } from '@/features/core/layout/editorSplitHitTest';
import type { EditorDropPreview } from './editorDropPreviewStore';

export function resolveSidebarGraphResourceDropPreview(
  resource: GraphResourceDragData,
  targetGroupId: string,
  resolved: { hit: EditorSplitHit; rect: EditorDropPreviewRect },
  shiftKey: boolean,
): EditorDropPreview {
  if (resolved.hit.mode === 'split') {
    return {
      kind: 'split',
      targetGroupId,
      edge: resolved.hit.edge,
      rect: resolved.rect,
    };
  }

  if (
    resource.type === 'function'
    && canDropFunctionIntoEventGraph(targetGroupId, resource, shiftKey)
  ) {
    return {
      kind: 'function-into-event',
      targetGroupId,
      rect: resolved.rect,
      shiftHeld: shiftKey,
    };
  }

  return {
    kind: 'merge',
    targetGroupId,
    rect: resolved.rect,
    resourceName: resource.name,
  };
}
