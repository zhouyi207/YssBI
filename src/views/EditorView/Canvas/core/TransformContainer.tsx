import { useRef, useEffect } from 'react';
import {
  applyViewportTransform,
  getViewport,
  subscribeToViewport,
  viewportTransformStyle,
} from '@/features/core/viewport';

export const TransformContainer = ({ graphPath, children }: { graphPath: string; children: React.ReactNode }) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!graphPath) return;
    return subscribeToViewport(graphPath, (viewport) => {
      const el = containerRef.current;
      if (el) applyViewportTransform(el, viewport);
    });
  }, [graphPath]);

  const initial = graphPath ? getViewport(graphPath) : { x: 0, y: 0, scale: 1 };

  return (
    <div ref={containerRef} style={viewportTransformStyle(initial)}>
      {children}
    </div>
  );
};
