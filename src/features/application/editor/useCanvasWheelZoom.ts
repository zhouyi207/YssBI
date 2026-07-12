import { useEffect, type RefObject } from 'react';
import { attachCanvasWheelZoom } from '@/features/core/viewport';

export function useCanvasWheelZoom(
  canvasElementRef: RefObject<HTMLDivElement | null>,
  graphPath: string | null,
) {
  useEffect(() => {
    const canvasEl = canvasElementRef.current;
    if (!canvasEl || !graphPath) return;
    return attachCanvasWheelZoom(canvasEl, graphPath);
  }, [canvasElementRef, graphPath]);
}
