import { applyLogFilter, type LogDomainId } from "@/features/application/log";
import { LogPanelStatus } from "./LogPanelStatus";
import { LogPanelToolbar } from "./LogPanelToolbar";
import { useLogWorkspaceContext } from "./logWorkspaceContext";

export function LogWorkspaceActions({ domain }: { readonly domain?: LogDomainId }) {
  const { logs, filter } = useLogWorkspaceContext();
  const filteredLogCount = domain ? applyLogFilter(logs, filter, domain).length : 0;
  return (
    <div
      data-yssbi-logs-header-actions
      className="flex h-full shrink-0 items-center gap-1 px-1"
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
    >
      {domain ? <LogPanelStatus filteredLogCount={filteredLogCount} /> : null}
      {domain ? <LogPanelToolbar /> : null}
    </div>
  );
}
