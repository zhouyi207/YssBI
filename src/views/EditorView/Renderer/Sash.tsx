import React, { useRef, useEffect } from 'react';
import type { LayoutDirection } from '@/shared/types/ui';
import { attachSashDrag } from './sashResizeLogic';

interface SashProps {
  orientation: LayoutDirection;
  beforeRef: React.RefObject<HTMLDivElement | null>;
  afterRef: React.RefObject<HTMLDivElement | null>;
  beforeNodeId: string;
  afterNodeId: string;
}

export const Sash: React.FC<SashProps> = ({
  orientation,
  beforeRef,
  afterRef,
  beforeNodeId,
  afterNodeId,
}) => {
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
    });
  }, [orientation, beforeRef, afterRef, beforeNodeId, afterNodeId]);

  const isRow = orientation === 'row';

  return (
    <div
      ref={sashRef}
      className={`
        group relative z-30 transition-colors duration-150
        ${isRow ? '-mx-1 h-full w-2 cursor-col-resize' : '-my-1 h-2 w-full cursor-row-resize'}
        hover:bg-primary/10 [&.active]:bg-primary/20
      `}
    >
      <div
        className={`
          absolute bg-border/60 transition-colors group-hover:bg-primary group-[.active]:bg-primary
          ${isRow
            ? 'left-1/2 h-full w-px -translate-x-1/2'
            : 'top-1/2 h-px w-full -translate-y-1/2'}
        `}
      />
    </div>
  );
};
