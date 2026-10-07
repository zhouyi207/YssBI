import { useMemo } from "react";
import {
  useFilteredLogs,
  useLiveLogs,
  useLogStore,
  type LogDomainId,
} from "@/features/application/log";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { LogPanelList } from "./LogPanelList";
import { useLogWorkspaceContext } from "./logWorkspaceContext";
function isSameLog(left: LogRecordDto, right: LogRecordDto): boolean {
  return left.streamId === right.streamId && left.sequence === right.sequence;
}
export function LogDomainPanel({ domain }: { readonly domain: LogDomainId }) {
  const { subscriptionStatus, refreshScrollToken, selectLog } = useLogWorkspaceContext();
  const filteredLogs = useFilteredLogs(domain, (entries) => entries);
  const hasLogs = useLiveLogs((snapshot) => snapshot.entries.length > 0);
  const awaitingSnapshot = useLiveLogs(
    (snapshot) => snapshot.streamId === null && snapshot.entries.length === 0,
  );
  const selectedLog = useLogStore((state) =>
    domain === "all" || state.selectedLog?.domain === domain ? state.selectedLog : null,
  );
  const autoScroll = useLogStore((state) => state.autoScroll);
  const selectedIndex = useMemo(() => {
    if (!selectedLog) return null;
    const index = filteredLogs.findIndex((log) => isSameLog(log, selectedLog));
    return index >= 0 ? index : null;
  }, [filteredLogs, selectedLog]);

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background text-foreground">
      <LogPanelList
        filteredLogs={filteredLogs}
        hasLogs={hasLogs}
        isInitialLoad={subscriptionStatus === "connecting" && awaitingSnapshot}
        autoScroll={autoScroll}
        refreshScrollToken={refreshScrollToken}
        selectedIndex={selectedIndex}
        onSelectLog={selectLog}
      />
    </div>
  );
}
