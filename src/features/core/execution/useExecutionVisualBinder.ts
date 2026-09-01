import { useEffect } from "react";
import { getExecutionVisual, subscribeExecutionVisual } from "./executionVisualSession";
import { syncExecutionVisualDom, clearExecutionVisualDom } from "./executionVisualDom";

/** Imperative node execution highlights — no per-event React re-render. */
export function useExecutionVisualBinder(
  canvasElementRef: React.RefObject<HTMLDivElement | null>,
  graphPath: string | undefined,
): void {
  useEffect(() => {
    const canvas = canvasElementRef.current;
    if (!canvas || !graphPath) return;

    const sync = () => {
      const snap = getExecutionVisual();
      if (!snap.active || snap.graphPath !== graphPath) {
        clearExecutionVisualDom(canvas);
        return;
      }
      syncExecutionVisualDom(canvas, snap);
    };

    sync();
    return subscribeExecutionVisual(sync);
  }, [canvasElementRef, graphPath]);
}
