import { LOG_ITEM_HEIGHT, LOG_ITEM_GAP } from '@/app/appConfig/default';
import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import type { LogMessage } from '@/shared/types/ui';
import { LogItemRow } from './LogItemRow';
import { useLogPanelVirtualList } from './useLogPanelVirtualList';
import type { LogPanelVariant } from './useLogPanelController';

export interface LogPanelVirtualListProps {
  logs: LogMessage[];
  loading: boolean;
  hasMore: boolean;
  loadMoreLogs: () => Promise<void>;
  autoScroll: boolean;
  refreshScrollToken: number;
  variant: LogPanelVariant;
  selectedIndex: number | null;
  onSelectLog: (index: number) => void;
}

export function LogPanelVirtualList({
  logs,
  loading,
  hasMore,
  loadMoreLogs,
  autoScroll,
  refreshScrollToken,
  variant,
  selectedIndex,
  onSelectLog,
}: LogPanelVirtualListProps) {
  const { viewportRef, virtualizer, handleScroll } = useLogPanelVirtualList({
    logs,
    autoScroll,
    hasMore,
    loading,
    loadMoreLogs,
    variant,
    refreshScrollToken,
  });

  return (
    <OverlayScrollbar
      ref={viewportRef}
      onScroll={handleScroll}
      direction="vertical"
      className="relative min-h-0 flex-1 bg-[var(--workbench-bg)]"
    >
      {loading ? (
        <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-center justify-center gap-2 border-b border-border/40 bg-[var(--workbench-bg)]/90 py-1.5 text-[10px] text-[var(--accent-color)]">
          <div className="h-2.5 w-2.5 animate-spin rounded-full border border-[var(--accent-color)] border-t-transparent" />
        </div>
      ) : null}
      <div className="py-0.5">
        <div style={{ height: virtualizer.getTotalSize(), width: '100%', position: 'relative' }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const log = logs[virtualRow.index];
            if (!log) return null;
            return (
              <div
                key={virtualRow.key}
                data-index={virtualRow.index}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: LOG_ITEM_HEIGHT + LOG_ITEM_GAP,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <LogItemRow
                  log={log}
                  isSelected={selectedIndex === virtualRow.index}
                  onClick={() => onSelectLog(virtualRow.index)}
                />
              </div>
            );
          })}
        </div>
      </div>
    </OverlayScrollbar>
  );
}
