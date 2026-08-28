import { useRef, useEffect } from 'react';
import {
  applyViewportGrid,
  getViewport,
  subscribeToViewport,
  viewportGridStyle,
  type ViewportScope,
} from '@/features/application/viewCapabilities';
import { GRID } from '@/app/appConfig/default';

export const ViewportGrid = ({ viewportScope }: { viewportScope: ViewportScope | null }) => {
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!viewportScope) return;
    return subscribeToViewport(viewportScope, (viewport) => {
      const el = gridRef.current;
      if (el) applyViewportGrid(el, viewport, GRID);
    });
  }, [viewportScope?.groupId, viewportScope?.graphPath]);

  const initial = viewportScope ? getViewport(viewportScope) : { x: 0, y: 0, scale: 1 };

  return (
    <div
      ref={gridRef}
      className="absolute inset-0 pointer-events-none"
      style={{
        backgroundImage: `radial-gradient(circle at 1px 1px, var(--grid-lines) 1px, transparent 1.25px)`,
        backgroundRepeat: 'repeat',
        ...viewportGridStyle(initial, GRID),
      }}
    />
  );
};
