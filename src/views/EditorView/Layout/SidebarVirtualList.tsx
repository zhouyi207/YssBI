import { useRef, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';

const ROW_HEIGHT = 28;

export interface SidebarVirtualListProps<T> {
  items: T[];
  renderItem: (item: T, index: number) => ReactNode;
  empty?: ReactNode;
  estimateSize?: number;
}

/** Flat sidebar list with virtual scroll for long resource lists. */
export function SidebarVirtualList<T>({
  items,
  renderItem,
  empty,
  estimateSize = ROW_HEIGHT,
}: SidebarVirtualListProps<T>) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => estimateSize,
    overscan: 8,
  });

  if (items.length === 0) {
    return <>{empty ?? null}</>;
  }

  return (
    <OverlayScrollbar ref={scrollRef} className="min-h-0 flex-1">
      <div
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const item = items[virtualRow.index];
          return (
            <div
              key={virtualRow.key}
              className="absolute left-0 top-0 w-full"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              {renderItem(item, virtualRow.index)}
            </div>
          );
        })}
      </div>
    </OverlayScrollbar>
  );
}
