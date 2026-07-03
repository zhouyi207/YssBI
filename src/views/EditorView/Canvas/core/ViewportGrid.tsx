import { useRef, useEffect } from 'react';
import {
  applyViewportGrid,
  getViewport,
  subscribeToViewport,
  viewportGridStyle,
} from '@/features/core/viewport';
import { GRID } from '@/app/appConfig/default';

export const ViewportGrid = ({ graphId }: { graphId: string }) => {
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!graphId) return;
    return subscribeToViewport(graphId, (viewport) => {
      const el = gridRef.current;
      if (el) applyViewportGrid(el, viewport, GRID);
    });
  }, [graphId]);

  const initial = graphId ? getViewport(graphId) : { x: 0, y: 0, scale: 1 };

  return (
    <div
      ref={gridRef}
      className="absolute inset-0 pointer-events-none"
      style={{
        backgroundImage: `
          linear-gradient(var(--grid-lines) 1px, transparent 1px),
          linear-gradient(90deg, var(--grid-lines) 1px, transparent 1px)
        `,
        ...viewportGridStyle(initial, GRID),
      }}
    />
  );
};
