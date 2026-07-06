import { useEffect, type RefObject } from 'react';
import { attachCanvasWheelZoom } from '@/features/core/viewport';

export function useCanvasWheelZoom(
  canvasElementRef: RefObject<HTMLDivElement | null>,
  graphId: string | null,
) {
  useEffect(() => {
    const canvasEl = canvasElementRef.current;
    if (!canvasEl || !graphId) return;
    return attachCanvasWheelZoom(canvasEl, graphId);
  }, [canvasElementRef, graphId]);
}
