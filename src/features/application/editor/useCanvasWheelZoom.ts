import { useEffect, type RefObject } from "react";
import { attachCanvasWheelZoom } from "@/features/core/viewport";
import type { ViewportScope } from "@/features/core/viewport";

export function useCanvasWheelZoom(
  canvasElementRef: RefObject<HTMLDivElement | null>,
  viewportScope: ViewportScope | null,
  enabled: boolean,
) {
  useEffect(() => {
    const canvasEl = canvasElementRef.current;
    if (!enabled || !canvasEl || !viewportScope) return;
    return attachCanvasWheelZoom(canvasEl, viewportScope);
  }, [canvasElementRef, enabled, viewportScope?.groupId, viewportScope?.graphPath]);
}
