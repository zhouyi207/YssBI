import { useRef, useEffect } from 'react';
import type { LayoutDirection } from '@/shared/types/ui';
import { attachSashDrag } from './sashResizeLogic';
import { cn } from '@/lib/utils';

interface SashProps {
  orientation: LayoutDirection;
  beforeRef: React.RefObject<HTMLDivElement | null>;
  afterRef: React.RefObject<HTMLDivElement | null>;
  beforeNodeId: string;
  afterNodeId: string;
}

export function Sash({
  orientation,
  beforeRef,
  afterRef,
  beforeNodeId,
  afterNodeId,
}: SashProps) {
  const sashRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const sash = sashRef.current;
    if (!sash) return;

    return attachSashDrag(sash, {
      orientation,
      beforeNodeId,
      afterNodeId,
      getBeforeEl: () => beforeRef.current,
      getAfterEl: () => afterRef.current,
      onActiveChange: (active) => sash.classList.toggle('active', active),
      onLimitChange: (atLimit) => sash.classList.toggle('at-limit', atLimit),
    });
  }, [orientation, beforeRef, afterRef, beforeNodeId, afterNodeId]);

  const isVertical = orientation === 'row';

  return (
    <div
      ref={sashRef}
      className={cn(
        'workbench-sash shrink-0 touch-none',
        isVertical ? 'workbench-sash-vertical' : 'workbench-sash-horizontal',
      )}
      aria-hidden
    />
  );
}
