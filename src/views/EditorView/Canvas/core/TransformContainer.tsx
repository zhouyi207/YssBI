import { useRef, useEffect } from 'react';
import {
  applyViewportTransform,
  getViewport,
  subscribeToViewport,
  viewportTransformStyle,
} from '@/features/core/viewport';

export const TransformContainer = ({ graphId, children }: { graphId: string; children: React.ReactNode }) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!graphId) return;
    return subscribeToViewport(graphId, (viewport) => {
      const el = containerRef.current;
      if (el) applyViewportTransform(el, viewport);
    });
  }, [graphId]);

  const initial = graphId ? getViewport(graphId) : { x: 0, y: 0, scale: 1 };

  return (
    <div ref={containerRef} style={viewportTransformStyle(initial)}>
      {children}
    </div>
  );
};
