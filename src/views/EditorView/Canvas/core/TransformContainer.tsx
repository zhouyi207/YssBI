import { useRef, useEffect } from 'react';
import { applyViewportTransform, viewportTransformStyle } from '@/features/core/viewport/viewportTransform';
import { getViewport, subscribeToViewport } from '@/features/core/viewport/viewportSession';
import type { ViewportScope } from '@/features/core/viewport';

export const TransformContainer = ({
  viewportScope,
  children,
}: {
  viewportScope: ViewportScope | null;
  children: React.ReactNode;
}) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!viewportScope) return;
    return subscribeToViewport(viewportScope, (viewport) => {
      const el = containerRef.current;
      if (el) applyViewportTransform(el, viewport);
    });
  }, [viewportScope?.groupId, viewportScope?.graphPath]);

  const initial = viewportScope ? getViewport(viewportScope) : { x: 0, y: 0, scale: 1 };

  return (
    <div ref={containerRef} style={viewportTransformStyle(initial)}>
      {children}
    </div>
  );
};
