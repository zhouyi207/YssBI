import { useMemo } from "react";
import { applyLogFilter, type LogDomainId } from "@/features/application/log";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { LogPanelList } from "./LogPanelList";
import { useLogWorkspaceContext } from "./logWorkspaceContext";
function isSameLog(left: LogRecordDto, right: LogRecordDto): boolean {
  return left.streamId === right.streamId && left.sequence === right.sequence;
}
export function LogDomainPanel({ domain }: { readonly domain: LogDomainId }) {
  const {
    logs,
    filter,
    selectedLog,
    autoScroll,
    isInitialLoad,
    refreshScrollToken,
    presentation,
    selectLog,
  } = useLogWorkspaceContext();
  const filteredLogs = useMemo(() => applyLogFilter(logs, filter, domain), [domain, filter, logs]);
  const selectedIndex = useMemo(() => {
    if (!selectedLog) return null;
    const index = filteredLogs.findIndex((log) => isSameLog(log, selectedLog));
    return index >= 0 ? index : null;
  }, [filteredLogs, selectedLog]);

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background text-foreground">
      <LogPanelList
        filteredLogs={filteredLogs}
        totalLogCount={logs.length}
        isInitialLoad={isInitialLoad}
        autoScroll={autoScroll}
        refreshScrollToken={refreshScrollToken}
        presentation={presentation}
        selectedIndex={selectedIndex}
        onSelectLog={selectLog}
      />
    </div>
  );
}
