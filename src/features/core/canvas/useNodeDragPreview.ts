import { useEffect, useRef } from 'react';
import { useGraphDataStore } from '@/features/core/dataStore';
import { getDragPreview, subscribeDragPreview } from './dragPreview';

/**
 * Applies node drag offset imperatively during pointer drag so React nodes
 * do not re-render every frame.
 */
export function useNodeDragPreview(
  canvasElementRef: React.RefObject<HTMLDivElement | null>,
  graphPath: string | null,
): void {
  const lastDraggedRef = useRef<Set<string>>(new Set());
  const rafRef = useRef(0);

  useEffect(() => {
    const root = canvasElementRef.current;
    if (!root || !graphPath) return;

    const apply = () => {
      const preview = getDragPreview();
      const store = useGraphDataStore.getState();

      if (!preview.active) {
        for (const nodeId of lastDraggedRef.current) {
          const el = root.querySelector(`[data-node-id="${nodeId}"]`) as HTMLElement | null;
          const pos = store.getGraphNode(graphPath, nodeId)?.position;
          if (!el || !pos) continue;
          el.style.transform = `translate3d(${pos.x}px, ${pos.y}px, 0)`;
        }
        lastDraggedRef.current = new Set();
        return;
      }

      lastDraggedRef.current = new Set(preview.dragNodeIds);
      for (const nodeId of preview.dragNodeIds) {
        const el = root.querySelector(`[data-node-id="${nodeId}"]`) as HTMLElement | null;
        const pos = store.getGraphNode(graphPath, nodeId)?.position;
        if (!el || !pos) continue;
        el.style.transform = `translate3d(${pos.x + preview.dragDelta.x}px, ${pos.y + preview.dragDelta.y}px, 0)`;
      }
    };

    const tick = () => {
      apply();
      if (getDragPreview().active) {
        rafRef.current = requestAnimationFrame(tick);
      }
    };

    const unsub = subscribeDragPreview(() => {
      cancelAnimationFrame(rafRef.current);
      apply();
      if (getDragPreview().active) {
        rafRef.current = requestAnimationFrame(tick);
      }
    });

    return () => {
      unsub();
      cancelAnimationFrame(rafRef.current);
    };
  }, [canvasElementRef, graphPath]);
}
