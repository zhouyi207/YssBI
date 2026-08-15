import type { EditorViewport, WorldBounds } from '@/features/core/viewport';

export interface CollectCanvasNodeWorldBoundsInput {
  canvasElement: HTMLElement;
  viewport: EditorViewport;
  nodeIds?: readonly string[];
}

export function collectCanvasNodeWorldBounds({
  canvasElement,
  viewport,
  nodeIds,
}: CollectCanvasNodeWorldBoundsInput): WorldBounds | null {
  if (!Number.isFinite(viewport.scale) || viewport.scale <= 0) return null;

  const requestedIds = nodeIds ? new Set(nodeIds) : null;
  const canvasRect = canvasElement.getBoundingClientRect();
  let bounds: WorldBounds | null = null;

  for (const element of canvasElement.querySelectorAll<HTMLElement>('[data-node-id]')) {
    const nodeId = element.dataset.nodeId;
    if (!nodeId || (requestedIds && !requestedIds.has(nodeId))) continue;

    const rect = element.getBoundingClientRect();
    const nodeBounds = {
      left: (rect.left - canvasRect.left - viewport.x) / viewport.scale,
      top: (rect.top - canvasRect.top - viewport.y) / viewport.scale,
      right: (rect.right - canvasRect.left - viewport.x) / viewport.scale,
      bottom: (rect.bottom - canvasRect.top - viewport.y) / viewport.scale,
    };
    if (!Object.values(nodeBounds).every(Number.isFinite)) continue;

    bounds = bounds
      ? {
          left: Math.min(bounds.left, nodeBounds.left),
          top: Math.min(bounds.top, nodeBounds.top),
          right: Math.max(bounds.right, nodeBounds.right),
          bottom: Math.max(bounds.bottom, nodeBounds.bottom),
        }
      : nodeBounds;
  }

  return bounds;
}
