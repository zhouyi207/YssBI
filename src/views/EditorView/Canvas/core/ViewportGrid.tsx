import { useRef, useEffect } from 'react';
import {
  applyViewportGrid,
  getViewport,
  subscribeToViewport,
  viewportGridStyle,
} from '@/features/core/viewport';
import { GRID } from '@/app/appConfig/default';

export const ViewportGrid = ({ graphPath }: { graphPath: string }) => {
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!graphPath) return;
    return subscribeToViewport(graphPath, (viewport) => {
      const el = gridRef.current;
      if (el) applyViewportGrid(el, viewport, GRID);
    });
  }, [graphPath]);

  const initial = graphPath ? getViewport(graphPath) : { x: 0, y: 0, scale: 1 };

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
