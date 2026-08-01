import { useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { SIDEBAR_FLAT_ROW_HEIGHT } from '@/features/core/sidebar';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { SidebarFlatRowItem } from './SidebarFlatRowItem';
import type { SidebarRenderRow } from './sidebarRenderRows';

export function SidebarFlatRowList({ rows }: { rows: SidebarRenderRow[] }) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => SIDEBAR_FLAT_ROW_HEIGHT,
    overscan: 12,
  });

  if (rows.length === 0) {
    return <div className="min-h-0 min-w-0 flex-1" />;
  }

  return (
    <OverlayScrollbar ref={scrollRef} className="min-h-0 min-w-0 flex-1 basis-0">
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const row = rows[virtualRow.index];
          return (
            <div
              key={row.rowKey}
              className="absolute left-0 top-0 h-7 w-full overflow-x-hidden"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              <SidebarFlatRowItem row={row} />
            </div>
          );
        })}
      </div>
    </OverlayScrollbar>
  );
}
