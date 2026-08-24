import { LOG_ITEM_GAP, LOG_ITEM_HEIGHT } from '@/app/appConfig/default';
import { ScrollArea } from '@/components/ui/scroll-area';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { LogItemRow } from './LogItemRow';
import {
  useLogPanelVirtualList,
  type LogPanelPresentation,
} from './useLogPanelVirtualList';

export interface LogPanelVirtualListProps {
  readonly filteredLogs: readonly DiagnosticRecordDto[];
  readonly autoScroll: boolean;
  readonly refreshScrollToken: number;
  readonly presentation: LogPanelPresentation;
  readonly selectedIndex: number | null;
  readonly onSelectLog: (log: DiagnosticRecordDto) => void;
}

export function LogPanelVirtualList({
  filteredLogs,
  autoScroll,
  refreshScrollToken,
  presentation,
  selectedIndex,
  onSelectLog,
}: LogPanelVirtualListProps) {
  const { viewportRef, virtualizer, handleScroll } = useLogPanelVirtualList({
    filteredLogs,
    autoScroll,
    presentation,
    refreshScrollToken,
  });

  return (
    <ScrollArea
      viewportRef={viewportRef}
      onViewportScroll={handleScroll}
      orientation="vertical"
      className="relative min-h-0 flex-1 bg-background"
    >
      <div className="py-0.5">
        <div style={{ height: virtualizer.getTotalSize(), width: '100%', position: 'relative' }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const log = filteredLogs[virtualRow.index];
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
                  onClick={() => onSelectLog(log)}
                />
              </div>
            );
          })}
        </div>
      </div>
    </ScrollArea>
  );
}
