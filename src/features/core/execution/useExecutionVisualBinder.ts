import { useEffect } from 'react';
import {
  getExecutionVisual,
  subscribeExecutionVisual,
} from './executionVisualSession';
import { syncExecutionVisualDom, clearExecutionVisualDom } from './executionVisualDom';

/** Imperative node execution highlights — no per-event React re-render. */
export function useExecutionVisualBinder(
  canvasRef: React.RefObject<HTMLDivElement | null>,
  graphId: string | undefined,
): void {
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !graphId) return;

    const sync = () => {
      const snap = getExecutionVisual();
      if (!snap.active || snap.graphId !== graphId) {
        clearExecutionVisualDom(canvas);
        return;
      }
      syncExecutionVisualDom(canvas, snap);
    };

    sync();
    return subscribeExecutionVisual(sync);
  }, [canvasRef, graphId]);
}
