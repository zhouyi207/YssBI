import { LOG_ITEM_GAP, LOG_ITEM_HEIGHT } from '@/app/appConfig/default';
import { ScrollArea } from '@/components/ui/scroll-area';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { LogItemRow } from './LogItemRow';
import { useLogPanelVirtualList } from './useLogPanelVirtualList';
import type { LogPanelVariant } from './useLogPanelController';

export interface LogPanelVirtualListProps {
  logs: DiagnosticRecordDto[];
  autoScroll: boolean;
  refreshScrollToken: number;
  variant: LogPanelVariant;
  selectedIndex: number | null;
  onSelectLog: (index: number) => void;
}

export function LogPanelVirtualList({
  logs,
  autoScroll,
  refreshScrollToken,
  variant,
  selectedIndex,
  onSelectLog,
}: LogPanelVirtualListProps) {
  const { viewportRef, virtualizer, handleScroll } = useLogPanelVirtualList({
    logs,
    autoScroll,
    variant,
    refreshScrollToken,
  });

  return (
    <ScrollArea
      viewportRef={viewportRef}
      onViewportScroll={handleScroll}
      orientation="vertical"
      className="relative min-h-0 flex-1 bg-[var(--workbench-bg)]"
    >
      <div className="py-0.5">
        <div style={{ height: virtualizer.getTotalSize(), width: '100%', position: 'relative' }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const log = logs[virtualRow.index];
            if (!log) return null;
            return (
              <div
                key={`${log.streamId}:${log.sequence}`}
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
    </ScrollArea>
  );
}
